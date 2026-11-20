use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_SENSITIVITY: f32 = 1.22;
pub const MIN_SENSITIVITY: f32 = 1.05;
pub const MAX_SENSITIVITY: f32 = 2.0;
pub const DEFAULT_DEMO_BPM: f32 = 112.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AudioMode {
    #[default]
    Idle,
    System,
    File,
    Demo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct AudioTelemetry {
    pub mode: AudioMode,
    pub running: bool,
    pub busy: bool,
    pub beat: bool,
    pub onset: bool,
    pub strength: f32,
    pub level: f32,
    pub bass: f32,
    pub bpm: u16,
    pub beats: u64,
    pub locked: bool,
    pub next_beat_at_ms: Option<f64>,
    pub period_ms: Option<f64>,
    pub sensitivity: f32,
    pub file_name: Option<String>,
    /// True when `running && elapsed > 3500ms && level < 0.012`.
    pub silence_hint: bool,
    pub error: Option<String>,
}

impl Default for AudioTelemetry {
    fn default() -> Self {
        Self {
            mode: AudioMode::Idle,
            running: false,
            busy: false,
            beat: false,
            onset: false,
            strength: 0.0,
            level: 0.0,
            bass: 0.0,
            bpm: 0,
            beats: 0,
            locked: false,
            next_beat_at_ms: None,
            period_ms: None,
            sensitivity: DEFAULT_SENSITIVITY,
            file_name: None,
            silence_hint: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Start Windows WASAPI render loopback capture (`None` = system default output).
    StartSystem { device_id: Option<String> },
    /// Decode and loop a local audio file while feeding both speakers and beat detector.
    StartFile { path: PathBuf },
    /// Start synthesized demo beat generator (default 112 BPM, used by `--self-test` / diagnostics).
    StartDemo { bpm: f32 },
    /// Stop active audio capture / playback and reset beat detector.
    Stop,
    /// Update onset/flux sensitivity (`1.05..=2.0`, default `1.22`).
    SetSensitivity(f32),
    /// Play 3-tone countdown completion chime (`[660, 880, 1046]` Hz @ 0/220/440ms).
    PlayCompletionChime,
    /// Re-enumerate active Windows audio render endpoints.
    RefreshDevices,
    /// Gracefully stop background audio threads on application exit.
    Shutdown,
}

pub fn clamp_sensitivity(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY)
    } else {
        DEFAULT_SENSITIVITY
    }
}

pub fn is_supported_audio_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" | "opus"
    )
}
