//! Paths and persisted user settings.

use std::path::PathBuf;

use serde_json::{Value, json};

pub const MODELS: &[&str] = &["sonnet", "haiku", "opus"];

/// (label, Deepgram language code)
pub const LANGUAGES: &[(&str, &str)] = &[
    ("Multilingual", "multi"),
    ("English", "en"),
    ("Українська", "uk"),
    ("Русский", "ru"),
    ("Deutsch", "de"),
    ("Polski", "pl"),
    ("Español", "es"),
    ("Français", "fr"),
];

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("clueless")
}

fn settings_path() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("clueless/settings.json")
}

pub fn deepgram_key_path() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("clueless/deepgram.key")
}

/// `DEEPGRAM_API_KEY`, otherwise the contents of the key file.
pub fn deepgram_key() -> Option<String> {
    let usable = |k: String| Some(k.trim().to_string()).filter(|k| !k.is_empty());
    std::env::var("DEEPGRAM_API_KEY")
        .ok()
        .and_then(usable)
        .or_else(|| std::fs::read_to_string(deepgram_key_path()).ok().and_then(usable))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Background opacity of the window, 0.2 ..= 1.0.
    pub opacity: f64,
    pub auto_answer: bool,
    pub microphone: bool,
    pub keep_above: bool,
    pub model: u32,
    pub language: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            opacity: 0.72,
            auto_answer: true,
            microphone: false,
            keep_above: true,
            model: 0,
            language: 0,
        }
    }
}

impl Settings {
    pub fn from_json(v: &Value) -> Self {
        let d = Settings::default();
        let flag = |k: &str, d: bool| v[k].as_bool().unwrap_or(d);
        let index = |k: &str, len: usize| v[k].as_u64().filter(|&i| (i as usize) < len).unwrap_or(0) as u32;
        Settings {
            opacity: v["opacity"].as_f64().unwrap_or(d.opacity).clamp(0.2, 1.0),
            auto_answer: flag("auto_answer", d.auto_answer),
            microphone: flag("microphone", d.microphone),
            keep_above: flag("keep_above", d.keep_above),
            model: index("model", MODELS.len()),
            language: index("language", LANGUAGES.len()),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "opacity": self.opacity,
            "auto_answer": self.auto_answer,
            "microphone": self.microphone,
            "keep_above": self.keep_above,
            "model": self.model,
            "language": self.language,
        })
    }

    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .map(|v| Settings::from_json(&v))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, self.to_json().to_string()) {
            eprintln!("clueless: cannot save settings to {}: {e}", path.display());
        }
    }
}
