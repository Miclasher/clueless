//! Messages sent from background threads to the UI.

use crate::audio::Speaker;
use crate::claude::ClaudeEvent;
use crate::files::Attachment;

#[derive(Debug)]
pub enum Event {
    Claude(u64, ClaudeEvent),
    ClaudeExited(u64, String),
    /// Loudness of a capture, 0.0 ..= 1.0.
    Level(u64, Speaker, f32),
    /// Voice activity started or stopped.
    Speaking(u64, Speaker, bool),
    CaptureEnded(u64, Speaker, String),
    /// The line being spoken so far; empty once it has ended.
    Partial(u64, Speaker, String),
    /// A finished line of the transcript.
    Line(Speaker, String),
    /// Transcription is having trouble, or has recovered.
    SttStatus(u64, String),
    /// Transcription cannot work.
    SttFailed(u64, String),
    /// A screenshot of the primary screen, or why there is none.
    Screenshot(Result<Attachment, String>),
}
