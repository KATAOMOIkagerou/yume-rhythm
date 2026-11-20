//! 手动八度纠正（`BeatDetector::shift_tempo`，基准引擎没有的功能）。
//!
//! 每个测试让两个检测器吃同样的频谱字节：一个不纠正（即基准行为），一个在指定时刻纠正，
//! 再比较两者的输出。

// cfg(test) 让 clippy 把辅助代码视为测试代码（crate 的 clippy.toml 允许测试里 unwrap/expect/panic）。
#[cfg(test)]
mod common;

use common::fixture::{Fixture, Source, frame_times};
use common::synth::synthesize;
use yume_beat::{Analyser, BeatDetector, MotionFrame, TempoScale, TempoShift};

struct Run {
    times: Vec<f64>,
    plain: Vec<MotionFrame>,
    shifted: Vec<MotionFrame>,
    scales: Vec<TempoScale>,
    /// 每次 shift_tempo 的返回值。
    applied: Vec<bool>,
}

/// `shifts`：在第一个 `now >= at_ms` 的帧处理完之后调用 `shift_tempo`。
#[cfg(test)]
fn run(name: &str, shifts: &[(f64, TempoShift)]) -> Run {
    let fx = Fixture::load_named(name);
    let Source::Synth { spec } = &fx.source else {
        panic!("{name} 不是合成信号");
    };
    let samples = synthesize(spec);
    let mut analyser = Analyser::default();
    let bins = analyser.frequency_bin_count();
    let mut plain = BeatDetector::new(fx.sample_rate, bins);
    let mut shifted = BeatDetector::new(fx.sample_rate, bins);
    let mut bytes = vec![0_u8; bins];
    let n = analyser.fft_size();
    let times = frame_times(samples.len(), fx.sample_rate, fx.clock.hz, &fx.skip);
    let mut out = Run {
        times: times.clone(),
        plain: Vec::new(),
        shifted: Vec::new(),
        scales: Vec::new(),
        applied: Vec::new(),
    };
    let mut pending = shifts.iter().peekable();
    for now in times {
        let end = ((now * fx.sample_rate) / 1000.0).floor() as usize;
        let start = end.saturating_sub(n);
        analyser.get_byte_frequency_data(&samples[start..end.min(samples.len())], &mut bytes);
        out.plain.push(plain.process(now, &bytes));
        out.shifted.push(shifted.process(now, &bytes));
        while let Some(&&(at, shift)) = pending.peek() {
            if now < at {
                break;
            }
            out.applied.push(shifted.shift_tempo(shift));
            pending.next();
        }
        out.scales.push(shifted.tempo_scale());
    }
    out
}

/// `[from_ms, to_ms)` 内的帧下标。
fn span(r: &Run, from_ms: f64, to_ms: f64) -> std::ops::Range<usize> {
    let a = r
        .times
        .iter()
        .position(|&t| t >= from_ms)
        .unwrap_or(r.times.len());
    let b = r
        .times
        .iter()
        .position(|&t| t >= to_ms)
        .unwrap_or(r.times.len());
    a..b
}

fn beat_times(frames: &[MotionFrame], times: &[f64]) -> Vec<f64> {
    frames
        .iter()
        .zip(times)
        .filter(|(f, _)| f.beat)
        .map(|(_, &t)| t)
        .collect()
}

#[test]
fn double_restores_a_half_tempo_lock() {
    // 175 BPM 的鼓打贝斯：基准 3 秒后自己改锁到 88 BPM。6 秒时手动加倍。
    let r = run("bpm175_dnb", &[(6000.0, TempoShift::Double)]);
    assert_eq!(r.applied, [true]);
    let after = span(&r, 6100.0, 24000.0);
    for i in after.clone() {
        let (p, s) = (&r.plain[i], &r.shifted[i]);
        assert_eq!(r.scales[i], TempoScale::Double, "frame {i}");
        assert_eq!(p.bpm, 88.0);
        assert_eq!(s.bpm, 176.0);
        let period = p.period_ms.expect("locked");
        assert!((s.period_ms.expect("locked") - period / 2.0).abs() < 1e-9);
        let next = s.next_beat_at.expect("locked");
        assert!(next > r.times[i] && next - r.times[i] <= period / 2.0 + 1e-6);
    }
    let plain_beats = beat_times(&r.plain[after.clone()], &r.times[after.clone()]);
    let beats = beat_times(&r.shifted[after.clone()], &r.times[after]);
    assert!(
        beats.len().abs_diff(plain_beats.len() * 2) <= 2,
        "{} vs 2×{}",
        beats.len(),
        plain_beats.len()
    );
    // 基准的每一拍都保留，中间插入的拍落在两拍正中（±1 帧）。
    for t in &plain_beats {
        assert!(beats.contains(t));
    }
    // 基准自己的拍间隔也有 ±2 帧抖动（帧量化 + 每次估计 ±8 ms 的相位微调），插入的拍同理。
    let half = r.plain[r.plain.len() - 1].period_ms.expect("locked") / 2.0;
    let jitter = 2.0 * 1000.0 / 60.0;
    for w in beats.windows(2) {
        let gap = w[1] - w[0];
        assert!(
            (gap - half).abs() <= jitter,
            "beat gap {gap} ms, expected ≈{half}"
        );
    }
    let mean = (beats[beats.len() - 1] - beats[0]) / (beats.len() - 1) as f64;
    assert!(
        (mean - half).abs() < 2.0,
        "mean gap {mean} ms, expected ≈{half}"
    );
}

#[test]
fn half_drops_every_other_beat() {
    let r = run("bpm140_rock", &[(6000.0, TempoShift::Half)]);
    assert_eq!(r.applied, [true]);
    let after = span(&r, 6100.0, 24000.0);
    for i in after.clone() {
        assert_eq!(r.shifted[i].bpm, 70.0);
        let period = r.plain[i].period_ms.expect("locked");
        assert!((r.shifted[i].period_ms.expect("locked") - 2.0 * period).abs() < 1e-9);
    }
    let plain_beats = beat_times(&r.plain[after.clone()], &r.times[after.clone()]);
    let beats = beat_times(&r.shifted[after.clone()], &r.times[after]);
    assert!(beats.len().abs_diff(plain_beats.len() / 2) <= 1);
    let double = 2.0 * r.plain[r.plain.len() - 1].period_ms.expect("locked");
    for w in beats.windows(2) {
        let gap = w[1] - w[0];
        assert!(
            (gap - double).abs() <= 2.0 * 1000.0 / 60.0,
            "beat gap {gap} ms"
        );
    }
    // 节拍计数随输出走。
    let last = r.shifted.last().expect("frames");
    let plain_last = r.plain.last().expect("frames");
    assert!(last.beats < plain_last.beats);
}

#[test]
fn shifting_back_restores_the_original_output() {
    let r = run(
        "bpm120_four_floor",
        &[(5000.0, TempoShift::Double), (9000.0, TempoShift::Half)],
    );
    assert_eq!(r.applied, [true, true]);
    let restored = span(&r, 9100.0, 24000.0);
    let offset = r.shifted[restored.start].beats - r.plain[restored.start].beats;
    assert!(offset > 0, "倍速期间多出的拍保留在计数里");
    for i in restored {
        let mut expected = r.plain[i];
        expected.beats += offset;
        assert_eq!(r.shifted[i], expected, "frame {i}");
        assert_eq!(r.scales[i], TempoScale::Original);
    }
}

#[test]
fn relock_cancels_the_correction() {
    // 100 BPM 锁定后加倍（200 BPM）；12 秒处换成 128 BPM，基准在约 16 秒改锁到 64 BPM，
    // 纠正应随之失效；之后再次加倍得到 128 BPM。
    let r = run(
        "tempo_change_100_128",
        &[(5000.0, TempoShift::Double), (20000.0, TempoShift::Double)],
    );
    assert_eq!(r.applied, [true, true]);
    let i = span(&r, 6000.0, 6100.0).start;
    assert_eq!(r.shifted[i].bpm, 200.0);
    let relock = r
        .plain
        .iter()
        .position(|f| f.bpm == 64.0)
        .expect("未纠正时应改锁到 64 BPM");
    assert_eq!(r.scales[relock], TempoScale::Original);
    for i in span(&r, r.times[relock], 20000.0) {
        assert_eq!(r.shifted[i].bpm, r.plain[i].bpm);
    }
    for i in span(&r, 20100.0, 26000.0) {
        assert_eq!(r.shifted[i].bpm, 128.0);
    }
}

#[test]
fn rejected_shifts_change_nothing() {
    let mut d = BeatDetector::new(48_000.0, 1024);
    assert!(!d.shift_tempo(TempoShift::Double), "未锁拍时不生效");
    assert_eq!(d.tempo_scale(), TempoScale::Original);

    let r = run(
        "bpm140_rock",
        &[(6000.0, TempoShift::Double), (6000.0, TempoShift::Double)],
    );
    // 140 BPM（428 ms）加倍得 214 ms，低于 250 ms，拒绝；第二次同样拒绝。
    assert_eq!(r.applied, [false, false]);
    assert!(r.shifted.iter().zip(&r.plain).all(|(s, p)| s == p));
}
