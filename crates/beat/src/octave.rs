//! 手动八度纠正：基准引擎没有的功能，默认不生效。
//!
//! 检测器有时锁在半速或倍速（例如 94 ↔ 188 BPM）。直接改检测器内部的锁定周期行不通：
//! 之后的节奏估计仍偏好原来的八度，`updateLock` 的八度跳变逻辑会在两秒左右改回去。
//! 所以这里不碰检测器状态，而是在输出端换算：检测器照常按它偏好的八度跟拍，
//! 本模块据此推出倍速网格（在两拍之间插入中点拍）或半速网格（隔一拍取一拍）。
//! 检测器的相位微调会原样传到换算后的网格上。
//!
//! 纠正只对当前这次锁拍有效：解锁、或检测器改锁到差别超过 12% 的周期（换歌、或它自己
//! 跳到了另一个八度）时自动恢复原速。未纠正时输出与基准逐位相同。

use crate::frame::MotionFrame;
use crate::jsmath;

/// 八度纠正方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoShift {
    /// 速度加倍（周期减半），用于锁在半速时。
    Double,
    /// 速度减半（周期加倍），用于锁在倍速时。
    Half,
}

/// 当前生效的纠正。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TempoScale {
    /// 不纠正（基准行为）。
    #[default]
    Original,
    /// 输出为检测器速度的两倍。
    Double,
    /// 输出为检测器速度的一半。
    Half,
}

/// 纠正后的周期必须落在点头曲线（`locked_beat_nod`）接受的范围内。
const MIN_PERIOD_MS: f64 = 250.0;
const MAX_PERIOD_MS: f64 = 1600.0;
/// 检测器周期变化超过这个比例视为改锁，纠正失效。
const RELOCK_RATIO: f64 = 0.12;

/// 输出端的八度换算状态。
#[derive(Debug, Clone)]
pub(crate) struct OctaveCorrection {
    scale: TempoScale,
    /// 上一帧检测器的周期。
    anchor_period: f64,
    /// 倍速：待触发的中点拍时间。
    mid_due: Option<f64>,
    /// 半速：检测器下一拍是否保留（否则跳过）。
    upcoming_kept: bool,
    /// 半速：保留的下一拍时间。
    kept_next: f64,
    /// 最近一次检测器节拍的力度，用作中点拍力度。
    last_strength: f64,
    /// 最近一次输出节拍的时间。
    last_beat_at: f64,
    /// 输出节拍数与检测器节拍数之差。
    extra_beats: i64,
}

impl Default for OctaveCorrection {
    fn default() -> Self {
        Self {
            scale: TempoScale::Original,
            anchor_period: 0.0,
            mid_due: None,
            upcoming_kept: true,
            kept_next: 0.0,
            last_strength: 0.0,
            last_beat_at: f64::NEG_INFINITY,
            extra_beats: 0,
        }
    }
}

impl OctaveCorrection {
    pub fn scale(&self) -> TempoScale {
        self.scale
    }

    /// 在当前锁拍（`next_beat`、`period`，时间 `now`）上叠加一次纠正。
    pub fn shift(&mut self, shift: TempoShift, now: f64, next_beat: f64, period: f64) -> bool {
        let target = match (self.scale, shift) {
            (TempoScale::Original, TempoShift::Double) => TempoScale::Double,
            (TempoScale::Original, TempoShift::Half) => TempoScale::Half,
            (TempoScale::Double, TempoShift::Half) | (TempoScale::Half, TempoShift::Double) => {
                TempoScale::Original
            }
            // 不支持四倍速 / 四分之一速。
            (TempoScale::Double, TempoShift::Double) | (TempoScale::Half, TempoShift::Half) => {
                return false;
            }
        };
        let out_period = match target {
            TempoScale::Original => period,
            TempoScale::Double => period / 2.0,
            TempoScale::Half => period * 2.0,
        };
        if !(MIN_PERIOD_MS..=MAX_PERIOD_MS).contains(&out_period) {
            return false;
        }
        self.scale = target;
        self.anchor_period = period;
        self.mid_due = None;
        self.upcoming_kept = true;
        self.kept_next = next_beat;
        if target == TempoScale::Double && next_beat - period / 2.0 > now {
            self.mid_due = Some(next_beat - period / 2.0);
        }
        true
    }

    /// 换算一帧。`Original` 时只调整累计节拍数（从未纠正过时原样返回）。
    pub fn apply(&mut self, now: f64, frame: MotionFrame) -> MotionFrame {
        if frame.beat {
            self.last_strength = frame.strength;
        }
        let locked = match (frame.next_beat_at, frame.period_ms) {
            (Some(next), Some(period)) if frame.locked => Some((next, period)),
            _ => None,
        };
        if self.scale != TempoScale::Original {
            match locked {
                Some((_, period)) if (period / self.anchor_period - 1.0).abs() <= RELOCK_RATIO => {
                    self.anchor_period = period;
                }
                _ => self.scale = TempoScale::Original,
            }
        }
        let mut out = match (self.scale, locked) {
            (TempoScale::Double, Some((next, period))) => self.double(now, frame, next, period),
            (TempoScale::Half, Some((next, period))) => self.half(frame, next, period),
            _ => frame,
        };
        if out.beat {
            self.last_beat_at = now;
        }
        if self.extra_beats != 0 {
            let total = i64::from(frame.beats) + self.extra_beats;
            out.beats = u32::try_from(total.max(0)).unwrap_or(u32::MAX);
        }
        out
    }

    fn double(&mut self, now: f64, frame: MotionFrame, next: f64, period: f64) -> MotionFrame {
        let half = period / 2.0;
        let mut out = frame;
        if frame.beat {
            self.mid_due = None;
        } else if let Some(due) = self.mid_due
            && now >= due
        {
            self.mid_due = None;
            if now - self.last_beat_at >= half * 0.55 {
                out.beat = true;
                out.strength = self.last_strength;
                self.extra_beats += 1;
            }
        }
        let mid = next - half;
        if self.mid_due.is_none() && mid > now {
            self.mid_due = Some(mid);
        }
        out.next_beat_at = Some(if mid > now { mid } else { next });
        out.period_ms = Some(half);
        out.bpm = frame.bpm * 2.0;
        out
    }

    fn half(&mut self, frame: MotionFrame, next: f64, period: f64) -> MotionFrame {
        let mut out = frame;
        // 本帧的检测器节拍就是上一帧预告的那一拍。
        if frame.beat && !self.upcoming_kept {
            out.beat = false;
            out.strength = 0.0;
            self.extra_beats -= 1;
        }
        // 保留网格 kept_next + 2k·period 上，离 next 最近的点决定 next 是否保留。
        let steps = jsmath::round((next - self.kept_next) / period);
        self.upcoming_kept = steps.rem_euclid(2.0) == 0.0;
        self.kept_next = if self.upcoming_kept {
            next
        } else {
            next + period
        };
        out.next_beat_at = Some(self.kept_next);
        out.period_ms = Some(period * 2.0);
        out.bpm = (frame.bpm / 2.0).round();
        out
    }
}
