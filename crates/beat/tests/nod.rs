//! 点头曲线（基准 `m1`）与空闲帧（基准 `jh`）的对比，基准见 `fixtures/nod_cases.json`。

// cfg(test) 让 clippy 把辅助代码视为测试代码（crate 的 clippy.toml 允许测试里 unwrap/expect/panic）。
#[cfg(test)]
mod common;

use serde::Deserialize;
use yume_beat::{MotionFrame, locked_beat_nod};

#[derive(Deserialize)]
struct IdleFrame {
    beat: bool,
    onset: bool,
    strength: f64,
    level: f64,
    bass: f64,
    bpm: f64,
    beats: u32,
    locked: bool,
    #[serde(rename = "nextBeatAt")]
    next_beat_at: Option<f64>,
    #[serde(rename = "periodMs")]
    period_ms: Option<f64>,
}

/// `[now, nextBeatAt, periodMs, strength, m1 结果]`。
type NodCase = (f64, Option<f64>, Option<f64>, f64, Option<f64>);

#[derive(Deserialize)]
struct NodCases {
    idle: IdleFrame,
    cases: Vec<NodCase>,
}

#[cfg(test)]
fn load() -> NodCases {
    let path = common::fixture::fixtures_dir().join("nod_cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("nod_cases.json")).expect("解析")
}

#[test]
fn default_frame_is_the_original_idle_frame() {
    let jh = load().idle;
    let expected = MotionFrame {
        beat: jh.beat,
        onset: jh.onset,
        strength: jh.strength,
        level: jh.level,
        bass: jh.bass,
        bpm: jh.bpm,
        beats: jh.beats,
        locked: jh.locked,
        next_beat_at: jh.next_beat_at,
        period_ms: jh.period_ms,
    };
    assert_eq!(MotionFrame::default(), expected);
    assert_eq!(MotionFrame::IDLE, expected);
}

/// `None`/`Some` 必须一致；数值允许 1e-12 相对误差（`Math.cos` 与平台 `cos` 可能差最后一位）。
#[test]
fn nod_curve_matches_js() {
    let cases = load().cases;
    assert!(cases.len() > 1000);
    let mut exact = 0;
    for (i, &(now, next, period, strength, expected)) in cases.iter().enumerate() {
        let got = locked_beat_nod(now, next, period, strength);
        match (expected, got) {
            (None, None) => exact += 1,
            (Some(e), Some(g)) => {
                if e.to_bits() == g.to_bits() {
                    exact += 1;
                } else {
                    let rel = (e - g).abs() / e.abs().max(g.abs());
                    assert!(rel < 1e-12, "case #{i}: JS {e} / Rust {g}");
                }
            }
            _ => panic!("case #{i} {:?}: JS {expected:?} / Rust {got:?}", cases[i]),
        }
    }
    // 绝大多数应逐位相同。
    assert!(
        exact * 100 >= cases.len() * 95,
        "逐位相同 {exact}/{}",
        cases.len()
    );
}
