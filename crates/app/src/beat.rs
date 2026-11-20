//! 节拍线程：固定 60 Hz 从采样缓冲取最近 2048 点 → Analyser → BeatDetector → MotionSlot。
//! 每 120 ms 把遥测发给主线程（docs/架构.md 第 4、5 节）。

use crate::ToMain;
use crate::clock::Mono;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use yume_audio::SampleRing;
use yume_beat::{Analyser, BeatDetector, MotionFrame, TempoShift};
use yume_core::Command;
use yume_core::command::AudioCommand;
use yume_core::model::audio::Telemetry;

const FRAME_MS: f64 = 1000.0 / 60.0;
const TELEMETRY_MS: f64 = 120.0;
const DIAGNOSTIC_MS: f64 = 10_000.0;

/// 最新的节拍帧。写方覆盖，读方只取最新。
#[derive(Default)]
pub struct MotionSlot(Mutex<MotionFrame>);

impl MotionSlot {
    pub fn set(&self, frame: MotionFrame) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = frame;
        }
    }

    #[must_use]
    pub fn get(&self) -> MotionFrame {
        self.0.lock().map(|f| *f).unwrap_or_default()
    }
}

pub enum BeatControl {
    /// 有音源时运行，没有时停下并清空节拍状态。
    Active(bool),
    Shift(TempoShift),
    Quit,
}

pub fn spawn(
    ring: Arc<SampleRing>,
    motion: Arc<MotionSlot>,
    control: Receiver<BeatControl>,
    clock: Mono,
    to_main: ToMain,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("beat".into())
        .spawn(move || run(&ring, &motion, &control, clock, &to_main))
}

fn run(
    ring: &SampleRing,
    motion: &MotionSlot,
    control: &Receiver<BeatControl>,
    clock: Mono,
    to_main: &ToMain,
) {
    let mut analyser = Analyser::default();
    let mut samples = vec![0.0_f32; analyser.fft_size()];
    let mut bytes = vec![0_u8; analyser.frequency_bin_count()];
    let mut detector: Option<(u32, BeatDetector)> = None;
    let mut active = false;
    let mut next_frame = clock.now_ms();
    let mut next_telemetry = 0.0;
    let mut diagnostic = (0.0, 0_usize);

    loop {
        let wait = if active {
            (next_frame - clock.now_ms()).max(0.0)
        } else {
            1000.0
        };
        match control.recv_timeout(Duration::from_secs_f64(wait / 1000.0)) {
            Ok(BeatControl::Active(on)) => {
                if on != active {
                    active = on;
                    detector = None;
                    analyser.reset();
                    motion.set(MotionFrame::default());
                    next_frame = clock.now_ms();
                    diagnostic = (next_frame, 0);
                }
                continue;
            }
            Ok(BeatControl::Shift(shift)) => {
                if let Some((_, d)) = detector.as_mut() {
                    d.shift_tempo(shift);
                }
                continue;
            }
            Ok(BeatControl::Quit) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if !active {
            continue;
        }

        let now = clock.now_ms();
        // 落后太多（休眠、卡顿）就重新对齐，不补帧。
        next_frame = if now - next_frame > 100.0 {
            now + FRAME_MS
        } else {
            next_frame + FRAME_MS
        };

        // 音源换了（采样率可能变）就重建检测器。
        let generation = ring.generation();
        if detector.as_ref().is_none_or(|(g, _)| *g != generation) {
            let rate = f64::from(ring.sample_rate().max(1));
            detector = Some((generation, BeatDetector::new(rate, bytes.len())));
            analyser.reset();
        }
        let Some((_, d)) = detector.as_mut() else {
            continue;
        };
        ring.latest(&mut samples);
        analyser.get_byte_frequency_data(&samples, &mut bytes);
        let frame = d.process(now, &bytes);
        motion.set(frame);

        // 每 10 秒记一次输入情况，排查"没声音""不跟拍"用。
        if now - diagnostic.0 >= DIAGNOSTIC_MS {
            let written = ring.written();
            log::info!(
                "节拍：{:.0} 秒内收到 {} 个采样（{} Hz），电平 {:.3}，BPM {:.0}，{}",
                (now - diagnostic.0) / 1000.0,
                written.saturating_sub(diagnostic.1),
                ring.sample_rate(),
                frame.level,
                frame.bpm,
                if frame.locked {
                    "已锁拍"
                } else {
                    "未锁拍"
                }
            );
            diagnostic = (now, written);
        }

        if now >= next_telemetry {
            next_telemetry = now + TELEMETRY_MS;
            #[allow(clippy::cast_possible_truncation, reason = "显示用，精度足够")]
            let telemetry = Telemetry {
                bpm: frame.bpm as f32,
                level: frame.level as f32,
                bass: frame.bass as f32,
                beats: frame.beats,
                beat_locked: frame.locked,
            };
            to_main(Command::Audio(AudioCommand::Telemetry(telemetry)));
        }
    }
}
