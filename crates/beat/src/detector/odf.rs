//! 频带划分、起音强度环形缓冲（ODF）与基础统计。

use super::{BANDS, BIN_MS, BeatDetector, RING_LEN, WINDOW_BINS};
use crate::jsmath;

/// 从 ODF 取出的分析窗口，下标 0 是最新一格。基准 `window()` 的返回值 `{v, bass, r, n}`。
#[derive(Debug, Clone)]
pub(super) struct OdfWindow {
    /// 起音强度（基准 `v`）。
    pub v: Vec<f32>,
    /// 低频起音强度（基准 `bass`）。
    pub bass: Vec<f32>,
    /// 权重 `exp(-i/400)`（基准 `r`）。
    pub weights: Vec<f32>,
    /// 长度（基准 `n`）。
    pub n: usize,
}

/// 基准：`x1.ring`，即 `((i % 1600) + 1600) % 1600`。
#[inline]
pub(super) fn ring(i: i64) -> usize {
    i.rem_euclid(RING_LEN as i64) as usize
}

/// 加权自相关。基准：`x1.acf(h, m, f, b)`。
///
/// `Σ v[i]·v[i+lag]·w[i] / Σ w[i]`，`i + lag < n`；`lag >= n - 4` 时为 0。
pub(super) fn acf(values: &[f32], weights: &[f32], n: usize, lag: i64) -> f64 {
    if lag >= n as i64 - 4 {
        return 0.0;
    }
    // 基准负 lag 会读到 undefined，结果为 NaN；调用方不会传负数。
    let Ok(lag) = usize::try_from(lag) else {
        return f64::NAN;
    };
    let mut sum = 0.0;
    let mut weight_sum = 0.0;
    let count = n - lag;
    for ((&a, &b), &w) in values[..count]
        .iter()
        .zip(&values[lag..n])
        .zip(&weights[..count])
    {
        let w = f64::from(w);
        sum += f64::from(a) * f64::from(b) * w;
        weight_sum += w;
    }
    if weight_sum > 0.0 {
        sum / weight_sum
    } else {
        0.0
    }
}

impl BeatDetector {
    /// 16 个对数间隔频带（40 Hz–12 kHz）的频谱下标范围。基准：`x1.ensureBands`。
    pub(super) fn ensure_bands(&mut self, bin_hz: f64, bin_count: usize) {
        if self.bands_ready {
            return;
        }
        let lo_hz = 40.0_f64;
        let hi_hz = 12e3_f64;
        let last = bin_count as i64 - 1;
        for x in 0..BANDS {
            let start = lo_hz * (hi_hz / lo_hz).powf(x as f64 / BANDS as f64);
            let end = lo_hz * (hi_hz / lo_hz).powf((x + 1) as f64 / BANDS as f64);
            let start_bin = (start / bin_hz).floor() as i64;
            let end_bin = (end / bin_hz).ceil() as i64;
            self.band_lo[x] = start_bin.max(1);
            self.band_hi[x] = last.min(start_bin.max(end_bin));
            self.band_weight[x] = if start < 250.0 { 1.5 } else { 1.0 };
        }
        self.bands_ready = true;
    }

    /// 各频带字节均值 / 255。基准：`x1.bandEnergies`。
    pub(super) fn band_energies(&self, out: &mut [f32; BANDS]) {
        for (f, slot) in out.iter_mut().enumerate() {
            let (lo, hi) = (self.band_lo[f], self.band_hi[f]);
            let mut sum = 0.0_f64;
            for r in lo..=hi {
                let byte = usize::try_from(r)
                    .ok()
                    .and_then(|i| self.freq.get(i))
                    .copied()
                    .unwrap_or(0);
                sum += f64::from(byte);
            }
            *slot = (sum / (hi - lo + 1) as f64 / 255.0) as f32;
        }
    }

    /// 把本帧的起音强度写入从上一格到当前格之间的每一格。基准：`x1.writeOdf`。
    ///
    /// 第一次写入、间隔过大（`gap`）或跳过半个环形缓冲以上时，从当前格重新开始。
    pub(super) fn write_odf(&mut self, now: f64, value: f64, bass: f64, gap: bool) {
        let bin = jsmath::round_i64(now / BIN_MS);
        let value = value as f32;
        let bass = bass as f32;
        if self.last_bin < 0 || gap || bin - self.last_bin > (RING_LEN / 2) as i64 {
            self.first_bin = bin;
            if gap {
                self.odf.fill(0.0);
                self.bass_odf.fill(0.0);
                for band in &mut self.spectral_odf {
                    band.fill(0.0);
                }
            }
            self.store_bin(bin, value, bass);
            self.last_bin = bin;
            return;
        }
        for b in self.last_bin + 1..=bin {
            self.store_bin(b, value, bass);
        }
        self.last_bin = bin;
    }

    fn store_bin(&mut self, bin: i64, value: f32, bass: f32) {
        let i = ring(bin);
        self.odf[i] = value;
        self.bass_odf[i] = bass;
        for (band, &flux) in self.spectral_odf.iter_mut().zip(&self.spectral_frame) {
            band[i] = flux;
        }
    }

    /// 最近 `min(bins, 可用格数)` 格的 ODF，最新在前。基准：`x1.window(h = 1200)`。
    pub(super) fn window(&self, bins: usize) -> OdfWindow {
        let available = self.last_bin - self.first_bin + 1;
        // 基准可用格数为负时会抛 RangeError；状态机保证不会发生，这里按 0 处理。
        let n = (bins.min(WINDOW_BINS) as i64).min(available).max(0) as usize;
        let mut v = Vec::with_capacity(n);
        let mut bass = Vec::with_capacity(n);
        for i in 0..n {
            let idx = ring(self.last_bin - i as i64);
            v.push(self.odf[idx]);
            bass.push(self.bass_odf[idx]);
        }
        OdfWindow {
            v,
            bass,
            weights: self.decay_weights[..n].to_vec(),
            n,
        }
    }

    /// 按周期 `lag` 折叠 ODF，返回得分接近最高的相位（以格号表示）。基准：`x1.phasesFor`。
    pub(super) fn phases_for(
        &self,
        values: &[f32],
        weights: &[f32],
        n: usize,
        lag: i64,
    ) -> Vec<i64> {
        // 基准 lag 为 0 时结果是空数组；负数会抛异常，调用方不会传。
        let Ok(size) = usize::try_from(lag) else {
            return Vec::new();
        };
        if size == 0 {
            return Vec::new();
        }
        let last = self.last_bin;
        let mut num = vec![0.0_f64; size];
        let mut den = vec![0.0_f64; size];
        for (i, (&v, &w)) in values[..n].iter().zip(&weights[..n]).enumerate() {
            let h = (last - i as i64).rem_euclid(lag) as usize;
            num[h] += f64::from(v) * f64::from(w);
            den[h] += f64::from(w);
        }
        let mut score = vec![0.0_f64; size];
        let mut best = 0.0;
        for b in 0..size {
            let prev = (b + size - 1) % size;
            let next = (b + 1) % size;
            let d = den[prev] + den[b] + den[next];
            score[b] = if d > 0.0 {
                (num[prev] + num[b] + num[next]) / d
            } else {
                0.0
            };
            if score[b] > best {
                best = score[b];
            }
        }
        let mut phases = Vec::new();
        for b in 0..size {
            if best > 0.0
                && score[b] >= 0.95 * best
                && score[b] >= score[(b + 1) % size]
                && score[b] >= score[(b + size - 1) % size]
            {
                phases.push(last - (last - b as i64).rem_euclid(lag));
            }
        }
        phases
    }
}
