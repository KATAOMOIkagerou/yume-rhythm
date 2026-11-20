//! 回归测试的公共部分：读取基准轨迹、重建信号、驱动 Rust 实现。
#![allow(dead_code, reason = "每个测试二进制只用到其中一部分")]

pub mod compare;
pub mod fixture;
pub mod synth;
pub mod wav;

use yume_beat::{Analyser, AnalyserConfig, BeatDetector, locked_beat_nod};

use compare::RustFrame;
use fixture::{Clock, Fixture, fnv1a, frame_times};

/// 与 `lib/trace.mjs` 相同的驱动方式：固定 60 Hz 时钟，每帧取以
/// `floor(now·sr/1000)` 结尾（不含）的 fftSize 个采样，越界部分补零。
pub fn run_end_to_end(
    samples: &[f32],
    sample_rate: f64,
    config: AnalyserConfig,
    clock: &Clock,
    skip: &[[f64; 2]],
) -> Vec<RustFrame> {
    let mut analyser = Analyser::new(config).expect("分析器参数");
    let mut detector = BeatDetector::new(sample_rate, analyser.frequency_bin_count());
    let mut bytes = vec![0_u8; analyser.frequency_bin_count()];
    let n = analyser.fft_size();
    let mut window = vec![0.0_f32; n];
    frame_times(samples.len(), sample_rate, clock.hz, skip)
        .into_iter()
        .map(|now| {
            let end = ((now * sample_rate) / 1000.0).floor() as i64;
            for (i, slot) in window.iter_mut().enumerate() {
                let idx = end - n as i64 + i as i64;
                *slot = usize::try_from(idx)
                    .ok()
                    .and_then(|k| samples.get(k))
                    .copied()
                    .unwrap_or(0.0);
            }
            analyser.get_byte_frequency_data(&window, &mut bytes);
            let byte_hash = fnv1a(bytes.iter().copied());
            let frame = detector.process(now, &bytes);
            RustFrame {
                frame,
                nod: nod_at(clock, now, &frame),
                byte_hash: Some(byte_hash),
            }
        })
        .collect()
}

/// 轨迹里的 `nod` 列：`m1(now + nodOffsetMs, nextBeatAt, periodMs, nodStrength)`。
pub fn nod_at(clock: &Clock, now: f64, frame: &yume_beat::MotionFrame) -> Option<f64> {
    locked_beat_nod(
        now + clock.nod_offset_ms,
        frame.next_beat_at,
        frame.period_ms,
        clock.nod_strength,
    )
}

pub fn analyser_config(f: &Fixture) -> AnalyserConfig {
    AnalyserConfig {
        fft_size: f.analyser.fft_size,
        smoothing: f.analyser.smoothing,
        min_db: f.analyser.min_db,
        max_db: f.analyser.max_db,
    }
}
