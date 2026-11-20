//! JavaScript 数值语义的等价实现。
//!
//! 基准引擎是 JS，凡是 Rust 标准库语义不同的地方都集中在这里：
//! - `Math.round`：.5 向 +∞ 舍入（`-2.5 → -2`），Rust 的 `f64::round` 是远离 0。
//! - `Math.max` / `Math.min`：任一参数为 NaN 时结果为 NaN，Rust 的 `f64::max` 会忽略 NaN。
//! - `Float32Array` 写入：f64 → f32 就近舍入（ties-to-even），即 `as f32`。
//!
//! `%` 不在这里：JS 对浮点数的 `%` 与 Rust 的 `f64 %` 都是截断取余（fmod），语义一致；
//! 整数场景下基准的 `((a % m) + m) % m` 写成 `rem_euclid`。

/// JS `Math.round`：取最接近的整数，恰好 .5 时向 +∞。
///
/// 与 V8 的实现相同：先取 `ceil`，若 `ceil - 0.5 > x` 再减 1。
/// 这样 `0.49999999999999994` 也能得到 0（`floor(x + 0.5)` 会得到 1）。
#[inline]
pub(crate) fn round(x: f64) -> f64 {
    let c = x.ceil();
    if c - 0.5 > x { c - 1.0 } else { c }
}

/// JS `Math.round` 后转成整数索引（基准的 bin 号、lag 都是这样得到的整数）。
#[inline]
pub(crate) fn round_i64(x: f64) -> i64 {
    round(x) as i64
}

/// JS `Math.max(a, b)`（NaN 传播）。
#[inline]
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

/// JS `Math.min(a, b)`（NaN 传播）。
#[inline]
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// 基准：`Ce(value, lo, hi) = Math.min(hi, Math.max(lo, value))`。
#[inline]
pub(crate) fn clamp(value: f64, lo: f64, hi: f64) -> f64 {
    min(hi, max(lo, value))
}

/// 按 JS 数组语义读取：越界得到 `undefined`，参与算术与比较时等价于 NaN。
#[inline]
pub(crate) fn at(values: &[f64], index: i64) -> f64 {
    usize::try_from(index)
        .ok()
        .and_then(|i| values.get(i))
        .copied()
        .unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_matches_js() {
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(-0.5), 0.0);
        assert_eq!(round(0.49999999999999994), 0.0);
        assert_eq!(round(1.4999999999999998), 1.0);
        assert_eq!(round(-1.5), -1.0);
        assert_eq!(round(-1.5000000000000002), -2.0);
        assert_eq!(round(4503599627370497.0), 4503599627370497.0);
    }

    #[test]
    fn max_min_propagate_nan() {
        assert!(max(0.6, f64::NAN).is_nan());
        assert!(min(f64::NAN, 3.0).is_nan());
        assert_eq!(clamp(5.0, 0.0, 1.0), 1.0);
        assert!(clamp(f64::NAN, 0.0, 1.0).is_nan());
    }

    #[test]
    fn out_of_range_reads_are_nan() {
        let v = [1.0, 2.0];
        assert_eq!(at(&v, 1), 2.0);
        assert!(at(&v, 2).is_nan());
        assert!(at(&v, -1).is_nan());
    }
}
