//! Pure-Rust WASAPI loopback + FFT + beat detector + local playback + 3-tone chime crate.
//! Owned by `@audio-eng`.

pub mod detector;
pub mod fft;
pub mod file_player;
pub mod synth;
pub mod wasapi_io;

use crate::detector::BeatDetector;
use crate::fft::{AnalyserFft, FREQ_BINS};
use crate::synth::DemoSynth;
use crate::wasapi_io::{
    enumerate_output_devices, play_completion_chime_async, start_demo_player,
    start_loopback_capture, ProducerTask, ENGINE_SAMPLE_RATE,
};
use pet_core::{
    clamp_sensitivity, monotonic_ms, AudioCommand, AudioDeviceInfo, AudioMode, AudioTelemetry,
    DEFAULT_DEMO_BPM, DEFAULT_SENSITIVITY,
};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapRb};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DSP_TICK_MS: u64 = 10;
const RING_CAPACITY_SAMPLES: usize = ENGINE_SAMPLE_RATE as usize; // 1.0 s lock-free PCM buffer

/// Thread-safe handle to the background audio & DSP subsystem.
pub struct AudioEngineHandle {
    tx: Sender<AudioCommand>,
    telemetry: Arc<RwLock<AudioTelemetry>>,
    devices: Arc<RwLock<Vec<AudioDeviceInfo>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl AudioEngineHandle {
    /// Spawns the dedicated DSP & audio controller thread and enumerates Windows render endpoints.
    pub fn spawn() -> Result<Self, String> {
        let telemetry = Arc::new(RwLock::new(AudioTelemetry::default()));
        let devices = Arc::new(RwLock::new(Vec::new()));

        // Initial device enumeration (non-fatal if machine has no sound device).
        if let Ok(list) = enumerate_output_devices() {
            if let Ok(mut guard) = devices.write() {
                *guard = list;
            }
        }

        let (tx, rx) = mpsc::channel::<AudioCommand>();
        let t_telemetry = telemetry.clone();
        let t_devices = devices.clone();

        let worker = thread::Builder::new()
            .name("pet-audio-dsp".into())
            .spawn(move || {
                dsp_controller_loop(rx, t_telemetry, t_devices);
            })
            .map_err(|e| format!("无法启动音频 DSP 线程: {e}"))?;

        Ok(Self {
            tx,
            telemetry,
            devices,
            worker: Mutex::new(Some(worker)),
        })
    }

    /// Sends a non-blocking control command to the audio engine.
    pub fn send(&self, command: AudioCommand) {
        let _ = self.tx.send(command);
    }

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
        let bpm = run_offline_synth_test(120.0, 12.0, DEFAULT_SENSITIVITY)?;
        if (bpm as i32 - 120).abs() <= 2 {
            Ok(bpm)
        } else {
            Err(format!("120 BPM 自检未锁定到目标范围: 实测 {bpm} BPM"))
        }
    }
}

impl Drop for AudioEngineHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(AudioCommand::Shutdown);
        if let Ok(mut guard) = self.worker.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Offline helper for unit tests and `--self-test`: synthesizes `duration_secs` of drum+bass
/// audio at `target_bpm` and runs it through `AnalyserFft` + `BeatDetector` at 10 ms cadence.
pub fn run_offline_synth_test(
    target_bpm: f32,
    duration_secs: f32,
    sensitivity: f32,
) -> Result<u16, String> {
    let sample_rate = ENGINE_SAMPLE_RATE;
    let mut synth = DemoSynth::new(sample_rate, target_bpm);
    let mut fft = AnalyserFft::new(sample_rate);
    let mut detector = BeatDetector::new();
    detector.set_sensitivity(sensitivity);

    let step_samples = (sample_rate as usize) / 100; // 10 ms hop = 480 samples
    let total_steps = (duration_secs * 100.0).round() as usize;
    let mut chunk = vec![0.0_f32; step_samples];
    let mut freq = [0u8; FREQ_BINS];
    let mut last_locked_bpm = 0u16;
    let mut total_beats = 0u64;

    for step in 0..total_steps {
        synth.render_mono(&mut chunk);
        fft.push_samples(&chunk);
        fft.get_byte_frequency_data(&mut freq);
        let now_ms = (step as f64 + 1.0) * 10.0;
        let frame = detector.process(now_ms, &freq, sample_rate);
        if frame.locked && frame.bpm > 0 {
            last_locked_bpm = frame.bpm;
        }
        total_beats = frame.beats;
    }

    if last_locked_bpm == 0 || total_beats < 4 {
        return Err(format!(
            "离线检测未能稳定锁定 (bpm={last_locked_bpm}, beats={total_beats})"
        ));
    }
    Ok(last_locked_bpm)
}

struct ActiveSession {
    mode: AudioMode,
    producer: ProducerTask,
    cons: HeapCons<f32>,
    started_at_ms: f64,
    file_name: Option<String>,
    empty_ticks: u32,
    last_beat_strength: f32,
}

fn dsp_controller_loop(
    rx: Receiver<AudioCommand>,
    telemetry: Arc<RwLock<AudioTelemetry>>,
    devices: Arc<RwLock<Vec<AudioDeviceInfo>>>,
) {
    let mut fft = AnalyserFft::new(ENGINE_SAMPLE_RATE);
    let mut detector = BeatDetector::new();
    let mut freq_buf = [0u8; FREQ_BINS];
    let mut pcm_scratch = [0.0_f32; 4096];
    let mut session: Option<ActiveSession> = None;

    let runtime_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let stop_session = |session: &mut Option<ActiveSession>,
                        fft: &mut AnalyserFft,
                        detector: &mut BeatDetector,
                        telemetry: &Arc<RwLock<AudioTelemetry>>,
                        clear_error: bool| {
        if let Some(mut active) = session.take() {
            active.producer.stop_and_join();
        }
        fft.reset();
        detector.reset();
        if let Ok(mut t) = telemetry.write() {
            let sens = detector.sensitivity();
            let err = if clear_error { None } else { t.error.clone() };
            *t = AudioTelemetry {
                sensitivity: sens,
                error: err,
                ..AudioTelemetry::default()
            };
        }
    };

    let mut next_tick = Instant::now();

    loop {
        // Check for async runtime errors reported by active producer thread.
        let async_err = runtime_error
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(err_msg) = async_err {
            stop_session(&mut session, &mut fft, &mut detector, &telemetry, false);
            if let Ok(mut t) = telemetry.write() {
                t.error = Some(err_msg);
            }
        }

        // Wait for commands: block indefinitely when idle (0% CPU), or wake every 10 ms when active.
        let cmd_opt = if session.is_none() {
            match rx.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => break,
            }
        } else {
            let now = Instant::now();
            let wait = next_tick.saturating_duration_since(now);
            match rx.recv_timeout(wait) {
                Ok(cmd) => Some(cmd),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };

        if let Some(first_cmd) = cmd_opt {
            let mut should_shutdown = false;
            let mut pending = vec![first_cmd];
            while let Ok(more) = rx.try_recv() {
                pending.push(more);
            }

            for cmd in pending {
                match cmd {
                    AudioCommand::Shutdown => {
                        stop_session(&mut session, &mut fft, &mut detector, &telemetry, true);
                        should_shutdown = true;
                        break;
                    }
                    AudioCommand::Stop => {
                        stop_session(&mut session, &mut fft, &mut detector, &telemetry, true);
                    }
                    AudioCommand::SetSensitivity(val) => {
                        let clamped = clamp_sensitivity(val);
                        detector.set_sensitivity(clamped);
                        if let Ok(mut t) = telemetry.write() {
                            t.sensitivity = clamped;
                        }
                    }
                    AudioCommand::PlayCompletionChime => {
                        play_completion_chime_async();
                    }
                    AudioCommand::RefreshDevices => {
                        if let Ok(list) = enumerate_output_devices() {
                            if let Ok(mut guard) = devices.write() {
                                *guard = list;
                            }
                        }
                    }
                    AudioCommand::StartSystem { device_id } => {
                        stop_session(&mut session, &mut fft, &mut detector, &telemetry, true);
                        if let Ok(mut t) = telemetry.write() {
                            t.busy = true;
                            t.error = None;
                        }
                        if let Ok(mut guard) = runtime_error.lock() {
                            *guard = None;
                        }

                        let rb = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES);
                        let (prod, cons) = rb.split();
                        let err_sink = runtime_error.clone();
                        let on_error = Arc::new(move |msg: String| {
                            if let Ok(mut g) = err_sink.lock() {
                                *g = Some(msg);
                            }
                        });

                        match start_loopback_capture(device_id, prod, on_error) {
                            Ok(producer) => {
                                let started_at_ms = monotonic_ms();
                                session = Some(ActiveSession {
                                    mode: AudioMode::System,
                                    producer,
                                    cons,
                                    started_at_ms,
                                    file_name: None,
                                    empty_ticks: 0,
                                    last_beat_strength: 0.0,
                                });
                                next_tick = Instant::now();
                                if let Ok(mut t) = telemetry.write() {
                                    t.mode = AudioMode::System;
                                    t.running = true;
                                    t.busy = false;
                                    t.error = None;
                                    t.file_name = None;
                                }
                            }
                            Err(err_msg) => {
                                if let Ok(mut t) = telemetry.write() {
                                    t.busy = false;
                                    t.running = false;
                                    t.mode = AudioMode::Idle;
                                    t.error = Some(err_msg);
                                }
                            }
                        }
                    }
                    AudioCommand::StartFile { path } => {
                        stop_session(&mut session, &mut fft, &mut detector, &telemetry, true);
                        let file_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string());
                        if let Ok(mut t) = telemetry.write() {
                            t.busy = true;
                            t.error = None;
                        }
                        if let Ok(mut guard) = runtime_error.lock() {
                            *guard = None;
                        }

                        let rb = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES);
                        let (prod, cons) = rb.split();
                        let err_sink = runtime_error.clone();
                        let on_error = Arc::new(move |msg: String| {
                            if let Ok(mut g) = err_sink.lock() {
                                *g = Some(msg);
                            }
                        });

                        match file_player::start_file_player(path, prod, on_error) {
                            Ok(producer) => {
                                let started_at_ms = monotonic_ms();
                                session = Some(ActiveSession {
                                    mode: AudioMode::File,
                                    producer,
                                    cons,
                                    started_at_ms,
                                    file_name: file_name.clone(),
                                    empty_ticks: 0,
                                    last_beat_strength: 0.0,
                                });
                                next_tick = Instant::now();
                                if let Ok(mut t) = telemetry.write() {
                                    t.mode = AudioMode::File;
                                    t.running = true;
                                    t.busy = false;
                                    t.error = None;
                                    t.file_name = file_name;
                                }
                            }
                            Err(err_msg) => {
                                if let Ok(mut t) = telemetry.write() {
                                    t.busy = false;
                                    t.running = false;
                                    t.mode = AudioMode::Idle;
                                    t.error = Some(err_msg);
                                }
                            }
                        }
                    }
                    AudioCommand::StartDemo { bpm } => {
                        stop_session(&mut session, &mut fft, &mut detector, &telemetry, true);
                        let demo_bpm = if bpm.is_finite() && bpm > 0.0 {
                            bpm
                        } else {
                            DEFAULT_DEMO_BPM
                        };
                        let rb = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES);
                        let (prod, cons) = rb.split();
                        match start_demo_player(demo_bpm, prod) {
                            Ok(producer) => {
                                let started_at_ms = monotonic_ms();
                                session = Some(ActiveSession {
                                    mode: AudioMode::Demo,
                                    producer,
                                    cons,
                                    started_at_ms,
                                    file_name: None,
                                    empty_ticks: 0,
                                    last_beat_strength: 0.0,
                                });
                                next_tick = Instant::now();
                                if let Ok(mut t) = telemetry.write() {
                                    t.mode = AudioMode::Demo;
                                    t.running = true;
                                    t.busy = false;
                                    t.error = None;
                                    t.file_name = None;
                                }
                            }
                            Err(err_msg) => {
                                if let Ok(mut t) = telemetry.write() {
                                    t.busy = false;
                                    t.running = false;
                                    t.mode = AudioMode::Idle;
                                    t.error = Some(err_msg);
                                }
                            }
                        }
                    }
                }
            }

            if should_shutdown {
                break;
            }
        }

        // Run one 10 ms DSP frame if a session is active and tick deadline has arrived.
        if let Some(active) = session.as_mut() {
            let now_inst = Instant::now();
            if now_inst >= next_tick {
                let mut total_popped = 0usize;
                loop {
                    let n = active.cons.pop_slice(&mut pcm_scratch);
                    if n == 0 {
                        break;
                    }
                    fft.push_samples(&pcm_scratch[..n]);
                    total_popped += n;
                    if n < pcm_scratch.len() {
                        break;
                    }
                }

                // WASAPI loopback emits 0 packets during system silence: inject 10 ms zero frames
                // after 3 empty ticks so the FFT window & beat detector decay cleanly to silence.
                if total_popped == 0 {
                    active.empty_ticks = active.empty_ticks.saturating_add(1);
                    if active.empty_ticks >= 3 {
                        fft.push_silence((ENGINE_SAMPLE_RATE as usize) / 100);
                    }
                } else {
                    active.empty_ticks = 0;
                }

                fft.get_byte_frequency_data(&mut freq_buf);
                let now_ms = monotonic_ms();
                let frame = detector.process(now_ms, &freq_buf, ENGINE_SAMPLE_RATE);
                if frame.beat {
                    active.last_beat_strength = frame.strength;
                }

                let elapsed_ms = now_ms - active.started_at_ms;
                let silence_hint = elapsed_ms > 3500.0 && frame.level < 0.012;

                if let Ok(mut t) = telemetry.write() {
                    t.mode = active.mode;
                    t.running = true;
                    t.busy = false;
                    t.beat = frame.beat;
                    t.onset = frame.onset;
                    // Latch last fired beat strength so 24 fps render loop never misses strength
                    // between 100 Hz DSP ticks when detecting `beats > prev_beats`.
                    t.strength = active.last_beat_strength;
                    t.level = frame.level;
                    t.bass = frame.bass;
                    t.bpm = frame.bpm;
                    t.beats = frame.beats;
                    t.locked = frame.locked;
                    t.next_beat_at_ms = frame.next_beat_at_ms;
                    t.period_ms = frame.period_ms;
                    t.sensitivity = detector.sensitivity();
                    t.file_name = active.file_name.clone();
                    t.silence_hint = silence_hint;
                    t.error = None;
                }

                next_tick += Duration::from_millis(DSP_TICK_MS);
                if next_tick < now_inst {
                    next_tick = now_inst + Duration::from_millis(DSP_TICK_MS);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::generate_completion_chime;
    use std::f32::consts::TAU;

    #[test]
    fn offline_120_bpm_locks_within_2_bpm() {
        let bpm = run_offline_synth_test(120.0, 12.0, DEFAULT_SENSITIVITY)
            .expect("120 BPM demo signal must lock");
        assert!(
            (bpm as i32 - 120).abs() <= 2,
            "expected ~120 BPM, got {bpm}"
        );
        assert!(AudioEngineHandle::self_test_120bpm().is_ok());
    }

    #[test]
    fn offline_112_bpm_locks_within_2_bpm() {
        let bpm = run_offline_synth_test(112.0, 12.0, DEFAULT_SENSITIVITY)
            .expect("112 BPM demo signal must lock");
        assert!(
            (bpm as i32 - 112).abs() <= 2,
            "expected ~112 BPM, got {bpm}"
        );
    }

    #[test]
    fn sensitivity_scaling_genuinely_affects_onset_detection() {
        // Synthesize soft rhythmic pulses where low sensitivity misses subtle onsets
        // and high sensitivity detects noticeably more onsets.
        let sample_rate = ENGINE_SAMPLE_RATE;
        let hop = (sample_rate as usize) / 100; // 10 ms

        let count_onsets = |sensitivity: f32| -> usize {
            let mut fft = AnalyserFft::new(sample_rate);
            let mut det = BeatDetector::new();
            det.set_sensitivity(sensitivity);
            let mut freq = [0u8; FREQ_BINS];
            let mut chunk = vec![0.0_f32; hop];
            let mut phase = 0.0_f32;
            let mut onsets = 0usize;

            for step in 0..600 {
                // 6 seconds: gentle bass pulse every 500 ms (50 steps), alternating soft/strong
                // so adaptive band peaks stay high while soft pulses sit near the flux gate.
                let pos_in_beat = step % 50;
                let is_strong_beat = (step / 50) % 2 == 0;
                let base_amp = 0.05_f32;
                let pulse_amp = if pos_in_beat < 4 {
                    if is_strong_beat {
                        0.45
                    } else {
                        0.12
                    }
                } else {
                    0.0
                };
                for s in &mut chunk {
                    phase = (phase + TAU * 110.0 / sample_rate as f32) % TAU;
                    let tone = phase.sin() + 0.5 * (phase * 4.0).sin() + 0.3 * (phase * 9.0).sin();
                    *s = tone * (base_amp + pulse_amp);
                }
                fft.push_samples(&chunk);
                fft.get_byte_frequency_data(&mut freq);
                let frame = det.process((step as f64 + 1.0) * 10.0, &freq, sample_rate);
                if frame.onset {
                    onsets += 1;
                }
            }
            onsets
        };

        let low = count_onsets(1.05);
        let high = count_onsets(2.0);
        assert!(
            high > low,
            "higher sensitivity (2.0 -> {high} onsets) must detect more onsets than low sensitivity (1.05 -> {low} onsets)"
        );
    }

    #[test]
    fn completion_chime_has_expected_duration_and_three_tones() {
        let sr = 48_000u32;
        let samples = generate_completion_chime(sr);
        let dur_sec = samples.len() as f32 / sr as f32;
        assert!(
            (0.85..=0.95).contains(&dur_sec),
            "expected ~0.90s chime, got {dur_sec:.3}s"
        );
        // Verify each of the 3 note windows (0.05s, 0.27s, 0.49s) has non-trivial energy.
        for &sec in &[0.05_f32, 0.27_f32, 0.49_f32] {
            let start = (sec * sr as f32) as usize;
            let peak = samples[start..start + 480]
                .iter()
                .map(|x| x.abs())
                .fold(0.0_f32, f32::max);
            assert!(
                peak > 0.05,
                "expected audible chime energy around {sec}s, peak={peak}"
            );
        }
    }

    #[test]
    fn device_enumeration_is_safe_and_consistent() {
        // Should never panic even on headless machines; if devices exist, at most one is default.
        if let Ok(devices) = enumerate_output_devices() {
            let defaults = devices.iter().filter(|d| d.is_default).count();
            assert!(defaults <= 1, "at most one default render device expected");
            if defaults == 1 {
                assert!(
                    devices[0].is_default,
                    "default render device should be sorted first"
                );
            }
        }
    }

    #[test]
    fn dsp_frame_latency_is_sub_millisecond() {
        let mut synth = DemoSynth::new(ENGINE_SAMPLE_RATE, 120.0);
        let mut fft = AnalyserFft::new(ENGINE_SAMPLE_RATE);
        let mut det = BeatDetector::new();
        let mut chunk = vec![0.0_f32; 480];
        let mut freq = [0u8; FREQ_BINS];

        // Warm up 4 seconds so estimation window is populated.
        for step in 0..400 {
            synth.render_mono(&mut chunk);
            fft.push_samples(&chunk);
            fft.get_byte_frequency_data(&mut freq);
            let _ = det.process((step as f64 + 1.0) * 10.0, &freq, ENGINE_SAMPLE_RATE);
        }

        let start = Instant::now();
        let iters = 200usize;
        for step in 400..(400 + iters) {
            synth.render_mono(&mut chunk);
            fft.push_samples(&chunk);
            fft.get_byte_frequency_data(&mut freq);
            let _ = det.process((step as f64 + 1.0) * 10.0, &freq, ENGINE_SAMPLE_RATE);
        }
        let avg_us = start.elapsed().as_micros() as f64 / iters as f64;
        println!("DSP average frame time (FFT + BeatDetector): {avg_us:.1} us/frame");
        // Even in unoptimized debug mode, FFT + beat detection should stay comfortably fast.
        assert!(
            avg_us < 10_000.0,
            "DSP frame took unexpectedly long: {avg_us:.1} us/frame"
        );
    }

    #[test]
    fn offline_multi_tempo_locks_accurately() {
        for &target in &[105.0_f32, 128.0_f32, 136.0_f32] {
            let bpm = run_offline_synth_test(target, 12.0, DEFAULT_SENSITIVITY)
                .unwrap_or_else(|e| panic!("{target} BPM failed: {e}"));
            assert!(
                (bpm as f32 - target).abs() <= 2.0,
                "expected ~{target} BPM, got {bpm}"
            );
        }
    }

    #[test]
    fn engine_handle_command_lifecycle_and_sensitivity_update() {
        let handle = AudioEngineHandle::spawn().expect("spawn audio engine");
        handle.send(AudioCommand::SetSensitivity(1.75));
        handle.send(AudioCommand::RefreshDevices);
        // Brief wait for command processing on DSP thread.
        thread::sleep(Duration::from_millis(40));
        let t = handle.latest_telemetry();
        assert!((t.sensitivity - 1.75).abs() < 1e-4);
        assert_eq!(t.mode, AudioMode::Idle);
        assert!(!t.running);

        // Invalid file should report Chinese error cleanly without crashing.
        handle.send(AudioCommand::StartFile {
            path: std::path::PathBuf::from("nonexistent_audio_file_12345.mp3"),
        });
        thread::sleep(Duration::from_millis(50));
        let t2 = handle.latest_telemetry();
        assert!(!t2.running);
        assert!(t2.error.is_some());

        handle.send(AudioCommand::Stop);
        thread::sleep(Duration::from_millis(20));
        assert!(handle.latest_telemetry().error.is_none());
        drop(handle);
    }
}
