//! CSS 缓动曲线（`cubic-bezier`）。

/// CSS `cubic-bezier(x1, y1, x2, y2)`：给定时间进度 `t`（0–1）求输出进度。
#[derive(Clone, Copy, Debug)]
pub struct CubicBezier {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

/// CSS `ease-in-out`。
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);
/// 音符飘出用的曲线。
pub const NOTE_FLOAT: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);

impl CubicBezier {
    #[must_use]
    pub const fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }

    #[must_use]
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 || t >= 1.0 {
            return t;
        }
        // 先用牛顿法由 x 反求参数 s，失败时退回二分法。
        let mut s = t;
        for _ in 0..8 {
            let err = bezier(s, self.x1, self.x2) - t;
            if err.abs() < 1e-7 {
                return bezier(s, self.y1, self.y2);
            }
            let slope = bezier_slope(s, self.x1, self.x2);
            if slope.abs() < 1e-6 {
                break;
            }
            s -= err / slope;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        s = t;
        for _ in 0..40 {
            if bezier(s, self.x1, self.x2) < t {
                lo = s;
            } else {
                hi = s;
            }
            s = (lo + hi) * 0.5;
        }
        bezier(s, self.y1, self.y2)
    }
}

fn bezier(s: f64, p1: f64, p2: f64) -> f64 {
    let inv = 1.0 - s;
    3.0 * inv * inv * s * p1 + 3.0 * inv * s * s * p2 + s * s * s
}

fn bezier_slope(s: f64, p1: f64, p2: f64) -> f64 {
    let inv = 1.0 - s;
    3.0 * inv * inv * p1 + 6.0 * inv * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
}

/// 往返动画（0→1→0）在周期内的位置，两段各用一次 `ease-in-out`。
#[must_use]
pub fn ping_pong(now_ms: f64, period_ms: f64) -> f64 {
    let phase = (now_ms / period_ms).rem_euclid(1.0);
    if phase < 0.5 {
        EASE_IN_OUT.apply(phase * 2.0)
    } else {
        EASE_IN_OUT.apply((1.0 - phase) * 2.0)
    }
}

#[must_use]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_and_symmetry() {
        assert!((EASE_IN_OUT.apply(0.0)).abs() < 1e-9);
        assert!((EASE_IN_OUT.apply(1.0) - 1.0).abs() < 1e-9);
        assert!((EASE_IN_OUT.apply(0.5) - 0.5).abs() < 1e-4);
        assert!(NOTE_FLOAT.apply(0.3) > 0.8, "前快后慢");
    }

    #[test]
    fn ping_pong_returns() {
        assert!(ping_pong(0.0, 1000.0).abs() < 1e-9);
        assert!((ping_pong(500.0, 1000.0) - 1.0).abs() < 1e-9);
        assert!(ping_pong(1000.0, 1000.0).abs() < 1e-9);
    }
}
