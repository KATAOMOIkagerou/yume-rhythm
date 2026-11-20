//! 分析器的规范性质与公开接口的边界情况（不依赖基准轨迹）。

use yume_beat::{Analyser, AnalyserConfig, AnalyserError, BeatDetector, MotionFrame};

/// Blackman 窗下，频率正好落在第 k0 个频点上的余弦，只在 k0-2..=k0+2 有能量：
/// 归一化幅度依次为 A·a2/4、A·a1/4、A·a0/2、A·a1/4、A·a2/4（a0=0.42, a1=0.5, a2=0.08）。
#[test]
fn bin_centred_cosine_has_the_blackman_signature() {
    let mut analyser = Analyser::default();
    let n = analyser.fft_size();
    let k0 = 100;
    let amp = 0.5;
    let samples: Vec<f32> = (0..n)
        .map(|i| (amp * (2.0 * std::f64::consts::PI * (k0 * i) as f64 / n as f64).cos()) as f32)
        .collect();
    let mut bytes = vec![0_u8; analyser.frequency_bin_count()];
    // 平滑 0.08：重复输入同一窗口足够多次后收敛到未平滑的值。
    for _ in 0..40 {
        analyser.get_byte_frequency_data(&samples, &mut bytes);
    }
    let byte_of = |mag: f64| ((255.0 / 87.0) * (20.0 * mag.log10() + 90.0)).floor();
    for (offset, coeff) in [
        (-2_i64, 0.02),
        (-1, 0.125),
        (0, 0.21),
        (1, 0.125),
        (2, 0.02),
    ] {
        let k = (k0 as i64 + offset) as usize;
        let expected = byte_of(amp * coeff);
        assert!(
            (f64::from(bytes[k]) - expected).abs() <= 1.0,
            "bin {k}: {} vs {expected}",
            bytes[k]
        );
    }
    for (k, &b) in bytes.iter().enumerate() {
        if k.abs_diff(k0) > 2 {
            assert_eq!(b, 0, "bin {k} 应无能量");
        }
    }
}

#[test]
fn smoothing_carries_over_between_calls() {
    let tone: Vec<f32> = (0..2048).map(|i| ((i as f32) * 0.3).sin() * 0.5).collect();
    let silence = vec![0.0_f32; 2048];
    let mut smoothed = Analyser::default();
    let mut raw = Analyser::new(AnalyserConfig {
        smoothing: 0.0,
        ..AnalyserConfig::default()
    })
    .expect("合法参数");
    let mut a = vec![0_u8; 1024];
    let mut b = vec![0_u8; 1024];
    smoothed.get_byte_frequency_data(&tone, &mut a);
    smoothed.get_byte_frequency_data(&silence, &mut a);
    raw.get_byte_frequency_data(&tone, &mut b);
    raw.get_byte_frequency_data(&silence, &mut b);
    assert!(b.iter().all(|&x| x == 0), "不平滑时静音立即归零");
    assert!(a.iter().any(|&x| x > 0), "平滑时上一帧的 8% 仍在");
    smoothed.reset();
    smoothed.get_byte_frequency_data(&silence, &mut a);
    assert!(a.iter().all(|&x| x == 0));
}

#[test]
fn input_shorter_or_longer_than_fft_size() {
    let mut x = Analyser::default();
    let mut y = Analyser::default();
    let tail: Vec<f32> = (0..1500).map(|i| ((i as f32) * 0.05).sin()).collect();
    let mut padded = vec![0.0_f32; 548];
    padded.extend_from_slice(&tail);
    let mut longer = vec![0.9_f32; 3000];
    longer.extend_from_slice(&padded);
    let mut a = vec![0_u8; 1024];
    let mut b = vec![0_u8; 1024];
    x.get_byte_frequency_data(&tail, &mut a);
    y.get_byte_frequency_data(&longer, &mut b);
    assert_eq!(a, b, "不足补零、超出只取最后 fftSize 个");

    let mut short_out = vec![7_u8; 10];
    x.get_byte_frequency_data(&tail, &mut short_out);
    let mut long_out = vec![7_u8; 1100];
    y.get_byte_frequency_data(&longer, &mut long_out);
    assert_eq!(&long_out[..10], &short_out[..]);
    assert!(
        long_out[1024..].iter().all(|&v| v == 7),
        "超出频点数的元素保持不变"
    );
}

#[test]
fn invalid_configs_are_rejected() {
    let base = AnalyserConfig::default();
    let bad = |c: AnalyserConfig| Analyser::new(c).err();
    assert_eq!(
        bad(AnalyserConfig {
            fft_size: 1000,
            ..base
        }),
        Some(AnalyserError::FftSize(1000))
    );
    assert_eq!(
        bad(AnalyserConfig {
            fft_size: 16,
            ..base
        }),
        Some(AnalyserError::FftSize(16))
    );
    assert!(matches!(
        bad(AnalyserConfig {
            smoothing: 1.5,
            ..base
        }),
        Some(AnalyserError::Smoothing(_))
    ));
    assert!(matches!(
        bad(AnalyserConfig {
            min_db: -3.0,
            max_db: -90.0,
            ..base
        }),
        Some(AnalyserError::DecibelRange { .. })
    ));
    for size in [32, 64, 32768] {
        assert!(
            Analyser::new(AnalyserConfig {
                fft_size: size,
                ..base
            })
            .is_ok()
        );
    }
}

#[test]
fn detector_survives_odd_inputs() {
    let mut d = BeatDetector::new(48_000.0, 1024);
    // 频谱比 bin_count 短、长、为空都不应 panic。
    for (k, len) in [0_usize, 10, 1024, 4096].into_iter().enumerate() {
        let freq = vec![200_u8; len];
        let f = d.process(k as f64 * 16.0, &freq);
        assert!(!f.locked);
    }
    let mut tiny = BeatDetector::new(48_000.0, 0);
    let f = tiny.process(5.0, &[]);
    assert_eq!(f.bpm, 0.0);
    d.reset();
    let f = d.process(1000.0, &[0; 1024]);
    assert_eq!(
        f,
        MotionFrame {
            level: f.level,
            bass: f.bass,
            ..MotionFrame::IDLE
        }
    );
}

/// 节拍线程用法：持续的 120 BPM 脉冲最终锁拍，且预测的周期接近 500 ms。
#[test]
fn locks_onto_a_steady_pulse() {
    let sr = 48_000.0;
    let mut analyser = Analyser::default();
    let mut d = BeatDetector::new(sr, analyser.frequency_bin_count());
    let mut bytes = vec![0_u8; 1024];
    let signal: Vec<f32> = (0..(sr as usize * 12))
        .map(|i| {
            let t = i as f64 / sr;
            let phase = t % 0.5;
            let env = (-phase / 0.06).exp();
            (0.8 * env * (2.0 * std::f64::consts::PI * 60.0 * phase).sin()) as f32
        })
        .collect();
    let mut last = MotionFrame::IDLE;
    for k in 0..(12 * 60) {
        let now = k as f64 * 1000.0 / 60.0;
        let end = ((now * sr) / 1000.0) as usize;
        let start = end.saturating_sub(2048);
        analyser.get_byte_frequency_data(&signal[start..end], &mut bytes);
        last = d.process(now, &bytes);
    }
    assert!(last.locked, "12 秒后应已锁拍");
    let period = last.period_ms.expect("锁拍时有周期");
    assert!(
        (period - 500.0).abs() < 15.0 || (period - 1000.0).abs() < 30.0,
        "周期 {period}"
    );
    assert!(last.beats > 5);
}
