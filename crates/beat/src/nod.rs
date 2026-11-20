//! 锁拍点头曲线。

use crate::jsmath;

/// 锁拍时的点头幅度。基准：`m1(now, nextBeatAt, periodMs, strength)`。
///
/// 以离 `now` 最近的一拍为中心画一个升余弦脉冲，半宽 `min(170, 0.3·period)` 毫秒，
/// 峰值 `clamp(strength, 0.6, 3) × 3.4`。脉冲之外返回 `Some(0.0)`。
/// 未锁拍（`next_beat_at` 或 `period_ms` 为 `None`）、非有限值或周期不在
/// 250..=1600 毫秒时返回 `None`，调用方应改用自由摆动。
pub fn locked_beat_nod(
    now: f64,
    next_beat_at: Option<f64>,
    period_ms: Option<f64>,
    strength: f64,
) -> Option<f64> {
    let (Some(next), Some(period)) = (next_beat_at, period_ms) else {
        return None;
    };
    if !next.is_finite() || !period.is_finite() || period < 250.0 || period > 1600.0 {
        return None;
    }
    // 相对最近一拍的偏移，落在 [-period/2, period/2)。
    let offset = ((now - next + period / 2.0) % period + period) % period - period / 2.0;
    let half_width = jsmath::min(170.0, period * 0.3);
    if offset.abs() >= half_width {
        return Some(0.0);
    }
    let gain = jsmath::min(3.0, jsmath::max(0.6, strength));
    Some((1.0 + (std::f64::consts::PI * offset / half_width).cos()) * 0.5 * gain * 3.4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_on_the_beat_and_vanishes_outside_the_pulse() {
        let peak = locked_beat_nod(1000.0, Some(1000.0), Some(500.0), 1.0);
        assert_eq!(peak, Some(3.4));
        assert_eq!(
            locked_beat_nod(1200.0, Some(1000.0), Some(500.0), 1.0),
            Some(0.0)
        );
        assert_eq!(locked_beat_nod(1000.0, None, Some(500.0), 1.0), None);
        assert_eq!(
            locked_beat_nod(1000.0, Some(1000.0), Some(200.0), 1.0),
            None
        );
    }
}
