//! Rust 结果与 JS 基准轨迹的逐帧比较和统计。

use std::fmt;

use yume_beat::MotionFrame;

use super::fixture::JsFrame;

/// 浮点字段（strength、level、bass、next_beat_at、period_ms、nod）的相对误差上限。
pub const FLOAT_TOLERANCE: f64 = 1e-6;

/// Rust 侧一帧。
#[derive(Debug, Clone, Copy)]
pub struct RustFrame {
    pub frame: MotionFrame,
    pub nod: Option<f64>,
    /// 频谱字节的 FNV-1a；只比较检测器时为 `None`。
    pub byte_hash: Option<u32>,
}

/// 一条信号的一致性统计。
#[derive(Debug, Clone, Default)]
pub struct Parity {
    pub name: String,
    pub frames: usize,
    pub bytes_equal: usize,
    pub beat_equal: usize,
    pub onset_equal: usize,
    pub locked_equal: usize,
    pub bpm_equal: usize,
    pub beats_equal: usize,
    pub js_beats: usize,
    pub rust_beats: usize,
    /// JS 的节拍帧中，Rust 在 ±1 帧内也有节拍的个数。
    pub beats_matched: usize,
    /// 离散字段（beat/onset/locked/bpm/beats、next/period 是否有值）全部相同的帧数。
    pub discrete_equal: usize,
    /// 所有字段逐位相同的帧数。
    pub exact: usize,
    /// 第一个离散字段不同的帧。
    pub first_divergence: Option<usize>,
    /// 离散字段相同的帧上，浮点字段的最大相对误差。
    pub max_rel: f64,
    /// 离散字段相同、且所有浮点字段相对误差 ≤ [`FLOAT_TOLERANCE`] 的帧数。
    pub close: usize,
    /// 各浮点字段不逐位相同的帧数：strength、level、bass、next_beat_at、period_ms、nod。
    pub inexact: [usize; 6],
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        return 0.0;
    }
    (a - b).abs() / a.abs().max(b.abs())
}

fn rel_opt(a: Option<f64>, b: Option<f64>) -> f64 {
    match (a, b) {
        (Some(x), Some(y)) => rel(x, y),
        (None, None) => 0.0,
        _ => f64::INFINITY,
    }
}

fn same_bits(a: Option<f64>, b: Option<f64>) -> bool {
    a.map(f64::to_bits) == b.map(f64::to_bits)
}

pub fn compare(name: &str, js: &[JsFrame], rust: &[RustFrame]) -> Parity {
    assert_eq!(js.len(), rust.len(), "{name}: 帧数不同");
    let mut p = Parity {
        name: name.to_owned(),
        frames: js.len(),
        ..Parity::default()
    };
    for (i, (j, r)) in js.iter().zip(rust).enumerate() {
        let f = &r.frame;
        let bytes_ok = r.byte_hash.is_none_or(|h| h == j.byte_hash);
        p.bytes_equal += usize::from(bytes_ok);
        p.beat_equal += usize::from(j.beat == f.beat);
        p.onset_equal += usize::from(j.onset == f.onset);
        p.locked_equal += usize::from(j.locked == f.locked);
        p.bpm_equal += usize::from(j.bpm == f.bpm);
        p.beats_equal += usize::from(j.beats == f.beats);
        p.js_beats += usize::from(j.beat);
        p.rust_beats += usize::from(f.beat);
        if j.beat {
            let lo = i.saturating_sub(1);
            let hi = (i + 1).min(rust.len() - 1);
            p.beats_matched += usize::from(rust[lo..=hi].iter().any(|x| x.frame.beat));
        }
        let discrete = j.beat == f.beat
            && j.onset == f.onset
            && j.locked == f.locked
            && j.bpm == f.bpm
            && j.beats == f.beats
            && j.next_beat_at.is_some() == f.next_beat_at.is_some()
            && j.period_ms.is_some() == f.period_ms.is_some()
            && j.nod.is_some() == r.nod.is_some();
        if discrete {
            p.discrete_equal += 1;
            let bits_equal = [
                j.strength.to_bits() == f.strength.to_bits(),
                j.level.to_bits() == f.level.to_bits(),
                j.bass.to_bits() == f.bass.to_bits(),
                same_bits(j.next_beat_at, f.next_beat_at),
                same_bits(j.period_ms, f.period_ms),
                same_bits(j.nod, r.nod),
            ];
            for (count, ok) in p.inexact.iter_mut().zip(bits_equal) {
                *count += usize::from(!ok);
            }
            let worst = [
                rel(j.strength, f.strength),
                rel(j.level, f.level),
                rel(j.bass, f.bass),
                rel_opt(j.next_beat_at, f.next_beat_at),
                rel_opt(j.period_ms, f.period_ms),
                rel_opt(j.nod, r.nod),
            ]
            .into_iter()
            .fold(0.0, f64::max);
            p.max_rel = p.max_rel.max(worst);
            p.close += usize::from(worst <= FLOAT_TOLERANCE);
        } else if p.first_divergence.is_none() {
            p.first_divergence = Some(i);
        }
        let exact = discrete
            && bytes_ok
            && j.strength.to_bits() == f.strength.to_bits()
            && j.level.to_bits() == f.level.to_bits()
            && j.bass.to_bits() == f.bass.to_bits()
            && same_bits(j.next_beat_at, f.next_beat_at)
            && same_bits(j.period_ms, f.period_ms)
            && same_bits(j.nod, r.nod);
        p.exact += usize::from(exact);
    }
    p
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        100.0
    } else {
        100.0 * n as f64 / d as f64
    }
}

impl Parity {
    pub fn locked_rate(&self) -> f64 {
        pct(self.locked_equal, self.frames)
    }
    pub fn bpm_rate(&self) -> f64 {
        pct(self.bpm_equal, self.frames)
    }
    pub fn onset_rate(&self) -> f64 {
        pct(self.onset_equal, self.frames)
    }
    pub fn bytes_rate(&self) -> f64 {
        pct(self.bytes_equal, self.frames)
    }
    pub fn beats_matched_rate(&self) -> f64 {
        pct(self.beats_matched, self.js_beats)
    }
    pub fn close_rate(&self) -> f64 {
        pct(self.close, self.frames)
    }

    /// 端到端容差（理由见 tests/parity.rs 顶部注释）。返回不满足的条目。
    pub fn tolerance_failures(&self) -> Vec<String> {
        let mut fails = Vec::new();
        let mut check = |ok: bool, what: String| {
            if !ok {
                fails.push(format!("{}: {what}", self.name));
            }
        };
        check(
            self.locked_rate() >= 99.0,
            format!("locked 一致 {:.2}% < 99%", self.locked_rate()),
        );
        check(
            self.bpm_rate() >= 99.0,
            format!("bpm 一致 {:.2}% < 99%", self.bpm_rate()),
        );
        check(
            self.onset_rate() >= 99.0,
            format!("onset 一致 {:.2}% < 99%", self.onset_rate()),
        );
        check(
            self.js_beats.abs_diff(self.rust_beats) <= 1,
            format!(
                "节拍总数 JS {} / Rust {} 相差超过 1",
                self.js_beats, self.rust_beats
            ),
        );
        check(
            self.beats_matched_rate() >= 98.0,
            format!("±1 帧内匹配的节拍 {:.2}% < 98%", self.beats_matched_rate()),
        );
        fails
    }
}

impl fmt::Display for Parity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<22} frames={:>5} bytes={:>6.2}% exact={:>6.2}% beat={:>6.2}% onset={:>6.2}% \
             locked={:>6.2}% bpm={:>6.2}% beats(js/rust)={}/{} matched±1={:>6.2}% close={:>6.2}% \
             maxRel={:.1e}",
            self.name,
            self.frames,
            self.bytes_rate(),
            pct(self.exact, self.frames),
            pct(self.beat_equal, self.frames),
            self.onset_rate(),
            self.locked_rate(),
            self.bpm_rate(),
            self.js_beats,
            self.rust_beats,
            self.beats_matched_rate(),
            self.close_rate(),
            self.max_rel,
        )?;
        let [s, l, b, n, per, nod] = self.inexact;
        if self.inexact.iter().any(|&c| c > 0) {
            write!(
                f,
                " inexact[strength={s} level={l} bass={b} next={n} period={per} nod={nod}]"
            )?;
        }
        if let Some(i) = self.first_divergence {
            write!(f, " firstDiverge=#{i}")?;
        }
        Ok(())
    }
}
