//! 与 JS 基准引擎的逐帧回归对比。基准轨迹由 `tools/beat-baseline`（Node 里运行
//! `frozen-audio-core.js`）生成，见该目录的 README。
//!
//! 三层对比：
//! 1. **检测器，输入相同**：`silence_gap` 录制了基准每帧拿到的频谱字节，直接喂给
//!    `BeatDetector`。所有离散输出必须逐帧相同，浮点误差 ≤ 1e-9（相对）。
//! 2. **端到端，合成信号**：Rust 重建同一信号（逐位相同，由采样哈希保证），经 Rust
//!    `Analyser` + `BeatDetector`，与 JS 轨迹比较。
//! 3. **端到端，真实歌曲**（`#[ignore]`）：`YUME_BEAT_AUDIO=歌曲.wav`。
//!
//! 端到端容差的理由：信号、窗函数、FFT 单位根都只用正确舍入的基本运算生成，两边逐位
//! 相同；剩下唯一的差异来源是超越函数——Rust 用平台 libm（glibc / UCRT），V8 用自带的
//! fdlibm 移植——`log10`（分析器）以及 `exp`、`pow`、`log2`（检测器）可能在最后一位
//! 不同。这种差异只有在某个值恰好落在阈值或量化边界上时才会改变一次判断，之后状态
//! 可能分叉几百毫秒到几秒再重新收敛。因此：
//! - locked / bpm / onset 逐帧一致率 ≥ 99%（24 秒信号允许约 14 帧、0.24 秒的分叉）；
//! - ≥ 99% 的帧上所有浮点字段相对误差 ≤ 1e-6（最后一位的差异约 1e-16，传播后仍远小于此；
//!   真正的实现错误会让 `next_beat_at`、`nod` 在大部分帧上偏离）；
//! - 节拍总数相差 ≤ 1；
//! - JS 的节拍中 ≥ 98% 在 Rust 的 ±1 帧（±16.7 ms，低于人眼可分辨的点头误差）内出现。
//!
//! 当前实测（Linux / glibc）：所有信号的频谱字节、全部离散字段和检测器的全部浮点字段都
//! 逐位相同；只有 `nod`（`Math.cos` 与 glibc `cos`）在约 1% 的帧上差最后一位。
//! 报告见 `cargo test -p yume-beat --test parity -- --nocapture`。

// cfg(test) 让 clippy 把辅助代码视为测试代码（crate 的 clippy.toml 允许测试里 unwrap/expect/panic）。
#[cfg(test)]
mod common;

use std::path::PathBuf;

use common::compare::{Parity, RustFrame, compare};
use common::fixture::{Fixture, Source, frame_times, samples_hash};
use common::{analyser_config, nod_at, run_end_to_end, synth, wav};
use yume_beat::BeatDetector;

#[allow(clippy::print_stderr, reason = "一致性报告需要在 --nocapture 时可见")]
fn report(title: &str, rows: &[Parity]) {
    eprintln!("\n== {title} ==");
    for row in rows {
        eprintln!("{row}");
    }
}

fn sample_count(f: &Fixture) -> usize {
    match &f.source {
        Source::Synth { spec } => (spec.duration * spec.sample_rate).round() as usize,
        Source::Wav { samples, .. } => *samples,
    }
}

#[test]
fn detector_matches_js_on_recorded_bytes() {
    let fx = Fixture::load_named("silence_gap");
    let (bins, bytes) = fx.recorded_bytes().expect("silence_gap 应录制频谱字节");
    let bin_count = fx.analyser.fft_size / 2;
    let max_band_bin = fx.bands.hi.iter().copied().max().expect("频带");
    assert!(
        bins as i64 > max_band_bin,
        "录制的 {bins} 个频点必须覆盖检测器读取的最高频点 {max_band_bin}"
    );
    let times = frame_times(sample_count(&fx), fx.sample_rate, fx.clock.hz, &fx.skip);
    assert_eq!(times.len(), fx.frames);
    assert_eq!(bytes.len(), bins * fx.frames);

    let mut detector = BeatDetector::new(fx.sample_rate, bin_count);
    let mut freq = vec![0_u8; bin_count];
    let rust: Vec<RustFrame> = times
        .iter()
        .zip(bytes.chunks(bins))
        .map(|(&now, chunk)| {
            freq[..bins].copy_from_slice(chunk);
            let frame = detector.process(now, &freq);
            RustFrame {
                frame,
                nod: nod_at(&fx.clock, now, &frame),
                byte_hash: None,
            }
        })
        .collect();
    let parity = compare(&fx.name, &fx.js_frames(), &rust);
    report(
        "检测器（输入相同的频谱字节）",
        std::slice::from_ref(&parity),
    );

    assert!(parity.js_beats > 20, "基准轨迹里应有足够多的节拍");
    assert_eq!(
        parity.first_divergence, None,
        "离散输出必须逐帧相同：{parity}"
    );
    assert!(parity.max_rel <= 1e-9, "浮点误差过大：{parity}");
}

/// 端到端对比一条合成信号（每条信号一个测试，测试框架会并行运行）。
#[cfg(test)]
fn end_to_end_synthetic(name: &str) {
    let fx = Fixture::load_named(name);
    let Source::Synth { spec } = &fx.source else {
        panic!("{name} 不是合成信号");
    };
    let samples = synth::synthesize(spec);
    assert_eq!(
        samples_hash(&samples),
        fx.samples_hash,
        "{name}: Rust 重建的信号与 Node 不一致（synth.rs 与 signals.mjs 不同步？）"
    );
    let rust = run_end_to_end(
        &samples,
        fx.sample_rate,
        analyser_config(&fx),
        &fx.clock,
        &fx.skip,
    );
    let parity = compare(&fx.name, &fx.js_frames(), &rust);
    report("端到端（合成信号）", std::slice::from_ref(&parity));
    assert!(parity.js_beats > 20, "{name}: 基准轨迹里应有足够多的节拍");
    let failures = parity.tolerance_failures();
    assert!(failures.is_empty(), "超出容差：\n{}", failures.join("\n"));
}

macro_rules! end_to_end_tests {
    ($($name:ident),* $(,)?) => {
        const SYNTHETIC: &[&str] = &[$(stringify!($name)),*];
        $(
            #[test]
            fn $name() {
                end_to_end_synthetic(stringify!($name));
            }
        )*
    };
}

end_to_end_tests!(
    bpm094_hiphop,
    bpm120_four_floor,
    bpm140_rock,
    bpm175_dnb,
    tempo_change_100_128,
    silence_gap,
    quiet_demo112,
    stall_128,
);

/// 新增基准轨迹时必须在上面的列表里登记，否则不会被测试。
#[test]
fn every_synthetic_fixture_has_a_test() {
    let mut on_disk: Vec<String> = Fixture::all_synthetic()
        .into_iter()
        .map(|f| f.name)
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = SYNTHETIC.iter().map(|s| (*s).to_owned()).collect();
    listed.sort();
    assert_eq!(on_disk, listed);
}

/// 真实歌曲对比。步骤见 `tools/beat-baseline/README.md`：
/// ```text
/// node tools/beat-baseline/generate.mjs --wav song.wav        # 生成 song.wav.trace.json
/// YUME_BEAT_AUDIO=song.wav cargo test -p yume-beat --release -- --ignored --nocapture
/// ```
/// 轨迹默认在 `<wav>.trace.json`，可用 `YUME_BEAT_TRACE` 指定。
#[test]
#[ignore = "需要 YUME_BEAT_AUDIO=歌曲.wav 和对应的 JS 轨迹"]
fn end_to_end_matches_js_on_real_audio() {
    let audio =
        PathBuf::from(std::env::var_os("YUME_BEAT_AUDIO").expect("设置 YUME_BEAT_AUDIO=歌曲.wav"));
    let trace = std::env::var_os("YUME_BEAT_TRACE").map_or_else(
        || {
            let mut p = audio.clone().into_os_string();
            p.push(".trace.json");
            PathBuf::from(p)
        },
        PathBuf::from,
    );
    let fx = Fixture::load(&trace);
    let wav = wav::read_wav(&audio);
    assert_eq!(wav.sample_rate, fx.sample_rate, "采样率与轨迹不符");
    if let Source::Wav {
        samples, channels, ..
    } = &fx.source
    {
        assert_eq!(wav.samples.len(), *samples, "采样数与轨迹不符");
        assert_eq!(wav.channels, *channels, "声道数与轨迹不符");
    }
    assert_eq!(
        samples_hash(&wav.samples),
        fx.samples_hash,
        "下混后的采样与 Node 不一致"
    );
    let rust = run_end_to_end(
        &wav.samples,
        wav.sample_rate,
        analyser_config(&fx),
        &fx.clock,
        &fx.skip,
    );
    let parity = compare(&fx.name, &fx.js_frames(), &rust);
    report("端到端（真实歌曲）", std::slice::from_ref(&parity));
    let failures = parity.tolerance_failures();
    assert!(failures.is_empty(), "超出容差：\n{}", failures.join("\n"));
}
