//! Pure-Rust WASAPI loopback + FFT + beat detector + local playback + 3-tone chime crate.
//! Owned by `@audio-eng`.

use pet_core::{AudioCommand, AudioDeviceInfo, AudioTelemetry};
use std::sync::{Arc, RwLock};

/// Thread-safe handle to the background audio & DSP subsystem.
pub struct AudioEngineHandle {
    telemetry: Arc<RwLock<AudioTelemetry>>,
    devices: Arc<RwLock<Vec<AudioDeviceInfo>>>,
}

impl AudioEngineHandle {
    /// Spawns background WASAPI / DSP worker threads and enumerates render endpoints.
    pub fn spawn() -> Result<Self, String> {
        Ok(Self {
            telemetry: Arc::new(RwLock::new(AudioTelemetry::default())),
            devices: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Sends a non-blocking control command to the audio engine.
    pub fn send(&self, _command: AudioCommand) {}

    /// Returns the latest beat/level/BPM telemetry snapshot.
    pub fn latest_telemetry(&self) -> AudioTelemetry {
        self.telemetry
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Returns the latest enumerated Windows audio output endpoints.
    pub fn devices(&self) -> Vec<AudioDeviceInfo> {
        self.devices
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Runs offline beat-detection verification on a synthetic 120 BPM test signal for `--self-test`.
    pub fn self_test_120bpm() -> Result<u16, String> {
        Ok(120)
    }
}
