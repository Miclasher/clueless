//! Audio capture through PipeWire/PulseAudio (`parec`) and speech segmentation.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, Sender, sync_channel};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::event::Event;

pub const RATE: usize = 16_000;
const FRAME: usize = RATE * 30 / 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
    Them,
    You,
}

impl Speaker {
    pub fn label(self) -> &'static str {
        match self {
            Speaker::Them => "Them",
            Speaker::You => "You",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// Everything played on the default output device.
    System,
    /// A single playback stream, for example one browser tab.
    Stream(u32),
    Microphone,
}

impl Source {
    fn args(&self) -> Vec<String> {
        // Without these, PipeWire silently re-links a capture to the microphone
        // when the stream or device being recorded goes away.
        let pinned = [
            "--property=node.dont-reconnect=true",
            "--property=node.dont-fallback=true",
            "--property=node.dont-move=true",
        ];
        let target = match self {
            Source::System => "--device=@DEFAULT_MONITOR@".to_string(),
            Source::Stream(i) => format!("--monitor-stream={i}"),
            Source::Microphone => return vec!["--device=@DEFAULT_SOURCE@".into()],
        };
        std::iter::once(target).chain(pinned.map(String::from)).collect()
    }
}

/// Name of the default output device.
pub fn default_sink() -> Option<String> {
    let out = Command::new("pactl").arg("get-default-sink").output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !name.is_empty()).then_some(name)
}

/// Parses `pactl --format=json list sink-inputs`.
pub fn parse_streams(json: &str) -> Vec<(u32, String)> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|i| {
            let index = u32::try_from(i["index"].as_u64()?).ok()?;
            let p = &i["properties"];
            let app = p["application.name"]
                .as_str()
                .or(p["node.name"].as_str())
                .unwrap_or("Unknown app");
            let media = p["media.name"].as_str().unwrap_or("");
            let mut label = if media.is_empty() {
                app.to_string()
            } else {
                format!("{app}: {media}")
            };
            if label.chars().count() > 48 {
                label = label.chars().take(47).collect::<String>() + "…";
            }
            Some((index, format!("{label} (#{index})")))
        })
        .collect()
}

/// Applications that are currently playing sound.
pub fn list_streams() -> Vec<(u32, String)> {
    Command::new("pactl")
        .args(["--format=json", "list", "sink-inputs"])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .ok()
        .map(|o| parse_streams(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Energy based voice activity detector with an adaptive noise floor. It only
/// tells whether someone is talking; all audio is transcribed regardless.
pub struct Vad {
    floor: f32,
    active: bool,
    onset: usize,
    silence: usize,
}

impl Vad {
    pub fn new() -> Self {
        Vad {
            floor: 0.001,
            active: false,
            onset: 0,
            silence: 0,
        }
    }

    /// Feeds the loudness of one frame; returns the new state when it changes.
    pub fn push(&mut self, level: f32) -> Option<bool> {
        let threshold = (self.floor * 3.0).clamp(0.004, 0.05);
        let loud = level > threshold;
        // The floor drops quickly and rises slowly, so speech barely moves it.
        let k = if level < self.floor {
            0.2
        } else if self.active {
            0.001
        } else {
            0.01
        };
        self.floor += (level - self.floor) * k;

        if !self.active {
            self.onset = if loud { self.onset + 1 } else { 0 };
            if self.onset < 3 {
                return None;
            }
            self.active = true;
            self.silence = 0;
            return Some(true);
        }

        self.silence = if loud { 0 } else { self.silence + 1 };
        if self.silence < 25 {
            return None;
        }
        self.flush()
    }

    /// Declares silence without waiting for quiet frames.
    pub fn flush(&mut self) -> Option<bool> {
        if !self.active {
            return None;
        }
        self.active = false;
        self.onset = 0;
        Some(false)
    }
}

/// Decodes s16le audio.
fn samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b) as f32 / 32768.0)
        .collect()
}

pub fn rms(frame: &[f32]) -> f32 {
    (frame.iter().map(|s| s * s).sum::<f32>() / frame.len().max(1) as f32).sqrt()
}

/// A running capture; dropping it stops recording.
pub struct Capture {
    child: Child,
}

impl Capture {
    pub fn start(
        source: &Source,
        speaker: Speaker,
        generation: u64,
        events: Sender<Event>,
        audio: Sender<Vec<u8>>,
    ) -> Result<Capture> {
        let mut child = Command::new("parec")
            .args([
                "--raw",
                "--format=s16le",
                "--rate=16000",
                "--channels=1",
                "--latency-msec=30",
            ])
            .arg("--client-name=clueless")
            .args(source.args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("cannot start `parec` (install libpulse)")?;
        let mut out = child.stdout.take().context("no stdout")?;
        let mut err = child.stderr.take().context("no stderr")?;

        let (frames, incoming) = sync_channel::<Vec<u8>>(256);
        thread::spawn(move || {
            let mut bytes = [0u8; FRAME * 2];
            while out.read_exact(&mut bytes).is_ok() {
                if frames.send(bytes.to_vec()).is_err() {
                    break;
                }
            }
        });

        thread::spawn(move || {
            let mut vad = Vad::new();
            let mut count = 0u32;
            let speaking = |active: bool| {
                let _ = events.send(Event::Speaking(generation, speaker, active));
            };
            loop {
                let change = match incoming.recv_timeout(Duration::from_millis(800)) {
                    Ok(frame) => {
                        let level = rms(&samples(&frame));
                        // The transcriber outlives this thread only to finish up.
                        let _ = audio.send(frame);
                        count = count.wrapping_add(1);
                        if count.is_multiple_of(4) {
                            let _ = events.send(Event::Level(generation, speaker, (level * 6.0).min(1.0)));
                        }
                        vad.push(level)
                    }
                    // A paused stream delivers nothing, which is also a pause.
                    Err(RecvTimeoutError::Timeout) => vad.flush(),
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                if let Some(active) = change {
                    speaking(active);
                }
            }
            if let Some(active) = vad.flush() {
                speaking(active);
            }
            // Let the transcriber finish the last words.
            drop(audio);
            let mut why = String::new();
            let _ = err.read_to_string(&mut why);
            let _ = events.send(Event::CaptureEnded(generation, speaker, why.trim().to_string()));
        });

        Ok(Capture { child })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(vad: &mut Vad, level: f32, frames: usize) -> Vec<bool> {
        (0..frames).filter_map(|_| vad.push(level)).collect()
    }

    #[test]
    fn detects_speech_and_pauses() {
        let mut v = Vad::new();
        assert!(run(&mut v, 0.0, 50).is_empty());
        assert_eq!(run(&mut v, 0.1, 40), [true]);
        assert!(run(&mut v, 0.0, 20).is_empty());
        assert_eq!(run(&mut v, 0.0, 20), [false]);
    }

    #[test]
    fn decodes_samples() {
        assert_eq!(samples(&[0, 0, 0, 0x80]), [0.0, -1.0]);
    }

    #[test]
    fn reads_stream_list() {
        let j = r#"[{"index":7,"properties":{"application.name":"Firefox","media.name":"Meet"}}]"#;
        assert_eq!(parse_streams(j), vec![(7, "Firefox: Meet (#7)".to_string())]);
        assert!(parse_streams("garbage").is_empty());
    }
}
