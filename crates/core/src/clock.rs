//! 时间与随机数。core 不读系统时钟、不用全局随机数，全部由调用方注入（docs/架构.md 第 6 节）。

use serde::{Deserialize, Serialize};

/// 墙钟时间，Unix 毫秒。学习计时用它，电脑休眠期间也会照常流逝。
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct WallMs(pub i64);

impl WallMs {
    #[must_use]
    pub fn plus(self, ms: i64) -> Self {
        Self(self.0.saturating_add(ms))
    }

    /// `self - earlier`，单位毫秒。
    #[must_use]
    pub fn since(self, earlier: Self) -> i64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// 随机数源。返回 `[0, 1)` 区间的均匀分布。
pub trait Random {
    fn next_f64(&mut self) -> f64;

    /// 从 `0..len` 中均匀取一个下标。`len` 为 0 时返回 `None`。
    fn pick(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "next_f64 在 [0,1) 内，乘积落在 [0,len) 内"
        )]
        let index = (self.next_f64() * len as f64) as usize;
        Some(index.min(len - 1))
    }
}

/// SplitMix64：简单、可复现的伪随机数。测试用固定种子。
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl Random for SplitMix64 {
    fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        #[allow(clippy::cast_precision_loss, reason = "取高 53 位作为尾数，精度无损")]
        let value = (z >> 11) as f64 / (1u64 << 53) as f64;
        value
    }
}

/// 一次 `update` 的上下文：当前时间与随机数源。
pub struct Ctx<'a> {
    pub now: WallMs,
    pub rng: &'a mut dyn Random,
}

impl<'a> Ctx<'a> {
    pub fn new(now: WallMs, rng: &'a mut dyn Random) -> Self {
        Self { now, rng }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_mix_is_deterministic_and_in_range() {
        let mut a = SplitMix64::new(7);
        let mut b = SplitMix64::new(7);
        for _ in 0..1000 {
            let x = a.next_f64();
            assert!((0.0..1.0).contains(&x));
            assert!((x - b.next_f64()).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn pick_handles_empty_and_bounds() {
        let mut rng = SplitMix64::new(1);
        assert_eq!(rng.pick(0), None);
        for _ in 0..100 {
            assert!(rng.pick(3).is_some_and(|i| i < 3));
        }
    }
}
