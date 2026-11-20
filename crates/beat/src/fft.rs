//! 基 2 FFT 与单位根表。
//!
//! 单位根 `cos/sin(2πk/N)` 只用 IEEE 基本运算（`+ - * / sqrt`）生成，不调用平台
//! libm 的 `sin/cos`。基本运算是正确舍入的，所以本表在 Linux、Windows 和基准工具
//! （`tools/beat-baseline/lib/dsp.mjs` 中逐行相同的 `unitRoots`）里逐位相同，
//! 回归测试因此可以做到频谱字节完全一致。

/// `cos[k] = cos(2πk/N)`、`sin[k] = sin(2πk/N)`，`k ∈ [0, N)`。
#[derive(Debug, Clone)]
pub(crate) struct UnitRoots {
    pub cos: Vec<f64>,
    pub sin: Vec<f64>,
}

impl UnitRoots {
    /// `n` 必须是不小于 8 的 2 的幂（调用方保证）。
    ///
    /// 做法：半角递推得到基角 `2π/2^j`；第一个八分圆内按 `k` 的二进制位连乘；
    /// 其余部分用精确的对称关系填充。
    pub fn new(n: usize) -> Self {
        let levels = n.trailing_zeros() as usize;
        let mut bc = vec![0.0_f64; levels + 1];
        let mut bs = vec![0.0_f64; levels + 1];
        bc[0] = 1.0;
        if levels >= 1 {
            bc[1] = -1.0;
        }
        if levels >= 2 {
            bc[2] = 0.0;
            bs[2] = 1.0;
        }
        for j in 3..=levels {
            let c = ((1.0 + bc[j - 1]) / 2.0).sqrt();
            bc[j] = c;
            bs[j] = bs[j - 1] / (2.0 * c);
        }

        let mut cos = vec![0.0_f64; n];
        let mut sin = vec![0.0_f64; n];
        let eighth = n / 8;
        let quarter = n / 4;
        let half = n / 2;
        for k in 0..=eighth {
            let (mut c, mut s) = (1.0_f64, 0.0_f64);
            let mut b = 0;
            while (1_usize << b) <= k {
                if k & (1 << b) != 0 {
                    let j = levels - b;
                    let nc = c * bc[j] - s * bs[j];
                    let ns = c * bs[j] + s * bc[j];
                    c = nc;
                    s = ns;
                }
                b += 1;
            }
            cos[k] = c;
            sin[k] = s;
        }
        for k in eighth + 1..=quarter {
            cos[k] = sin[quarter - k];
            sin[k] = cos[quarter - k];
        }
        for k in quarter + 1..=half {
            cos[k] = -sin[k - quarter];
            sin[k] = cos[k - quarter];
        }
        for k in half + 1..n {
            cos[k] = -cos[k - half];
            sin[k] = -sin[k - half];
        }
        Self { cos, sin }
    }
}

/// 原地迭代基 2 时域抽取 FFT（正变换，核为 `e^{-2πi·kn/N}`），不做归一化。
///
/// `re`、`im` 长度必须等于 `roots` 的长度（调用方保证）。
pub(crate) fn fft_in_place(re: &mut [f64], im: &mut [f64], roots: &UnitRoots) {
    let n = re.len();
    let mut j = 0_usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut size = 2;
    while size <= n {
        let half = size >> 1;
        let step = n / size;
        let mut start = 0;
        while start < n {
            for k in 0..half {
                let wr = roots.cos[k * step];
                let wi = -roots.sin[k * step];
                let a = start + k;
                let b = a + half;
                let tr = wr * re[b] - wi * im[b];
                let ti = wr * im[b] + wi * re[b];
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
            start += size;
        }
        size <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_roots_are_accurate() {
        for n in [8, 32, 2048, 32768] {
            let r = UnitRoots::new(n);
            for k in 0..n {
                let a = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
                assert!((r.cos[k] - a.cos()).abs() < 1e-14, "cos n={n} k={k}");
                assert!((r.sin[k] - a.sin()).abs() < 1e-14, "sin n={n} k={k}");
            }
        }
    }

    #[test]
    fn fft_matches_naive_dft() {
        let n = 64;
        let roots = UnitRoots::new(n);
        let input: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 11) as f64 - 5.0).collect();
        let mut re = input.clone();
        let mut im = vec![0.0; n];
        fft_in_place(&mut re, &mut im, &roots);
        for k in 0..n {
            let (mut er, mut ei) = (0.0, 0.0);
            for (t, x) in input.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * t) as f64 / n as f64;
                er += x * a.cos();
                ei += x * a.sin();
            }
            assert!(
                (re[k] - er).abs() < 1e-9 && (im[k] - ei).abs() < 1e-9,
                "k={k}"
            );
        }
    }
}
