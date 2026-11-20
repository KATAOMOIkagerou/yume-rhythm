//! 眨眼：随机间隔闭眼一下（功能清单第 7 节）。被抑制时到点顺延，不补眨。

use yume_core::Random;

#[derive(Clone, Copy, Debug)]
pub struct BlinkTiming {
    pub min_interval_ms: f64,
    pub max_interval_ms: f64,
    pub duration_ms: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Blink {
    next_at: Option<f64>,
    until: Option<f64>,
}

impl Blink {
    /// 推进并返回此刻是否闭眼。`allowed` 为假时不闭眼。
    pub fn advance(
        &mut self,
        now: f64,
        timing: &BlinkTiming,
        allowed: bool,
        rng: &mut dyn Random,
    ) -> bool {
        let schedule = |rng: &mut dyn Random| {
            now + timing.min_interval_ms
                + rng.next_f64() * (timing.max_interval_ms - timing.min_interval_ms)
        };
        if let Some(until) = self.until {
            if now < until {
                return allowed;
            }
            self.until = None;
            self.next_at = Some(schedule(rng));
        }
        let next = *self.next_at.get_or_insert_with(|| schedule(rng));
        if now >= next {
            if allowed {
                self.until = Some(now + timing.duration_ms);
                return true;
            }
            self.next_at = Some(schedule(rng));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yume_core::SplitMix64;

    const T: BlinkTiming = BlinkTiming {
        min_interval_ms: 2400.0,
        max_interval_ms: 5200.0,
        duration_ms: 150.0,
    };

    #[test]
    fn blinks_within_interval_for_duration() {
        let mut b = Blink::default();
        let mut rng = SplitMix64::new(1);
        let mut closed = Vec::new();
        let mut t = 0.0;
        while t < 20_000.0 {
            if b.advance(t, &T, true, &mut rng) {
                closed.push(t);
            }
            t += 10.0;
        }
        assert!(!closed.is_empty());
        assert!(closed[0] >= 2400.0 && closed[0] <= 5210.0);
        let first_run = closed
            .iter()
            .take_while(|&&c| c < closed[0] + 1000.0)
            .count();
        assert!(
            (14..=16).contains(&first_run),
            "闭眼约 150 ms，实际 {first_run} 帧"
        );
    }

    #[test]
    fn suppressed_never_blinks() {
        let mut b = Blink::default();
        let mut rng = SplitMix64::new(1);
        assert!((0..2000).all(|i| !b.advance(f64::from(i) * 10.0, &T, false, &mut rng)));
    }
}
