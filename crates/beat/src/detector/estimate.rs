//! 节奏估计：给 30–100 格的候选周期打分并做节拍层级修正。

use super::odf::acf;
use super::{BIN_MS, BassOnset, BeatDetector, MAX_LAG, MIN_LAG};
use crate::jsmath::{self, at, clamp, round_i64};

/// 一次节奏估计的结果。基准 `estimate()` 的返回对象。
///
/// 基准另有 `fastLag`（恒为 `null`）、`fastF`、`fastPhases`、`cover` 几个字段，
/// 前三者只在 `fastLag !== null` 时使用，`cover` 从未被读取，这里都省略。
#[derive(Debug, Clone)]
pub(super) struct Estimate {
    /// 修正后的周期（格，整数）。基准 `rawLag`。
    pub raw_lag: i64,
    /// 抛物线插值后的周期（格）。基准 `rawF`。
    pub raw_f: f64,
    /// 该周期下的候选相位（格号）。基准 `rawPhases`。
    pub raw_phases: Vec<i64>,
    /// 峰值得分 / 平均得分。基准 `conf`。
    pub conf: f64,
    /// 各周期总分，下标 0..=101。基准 `scores`。
    pub scores: Vec<f64>,
    /// 由低音间隔改判为 2/3 周期。基准 `bassOverride`。
    pub bass_override: bool,
    /// 经过 4/3、2/3、×2、÷2 修正。基准 `metricalOverride`。
    pub metrical_override: bool,
    /// 分频带周期性得分。基准 `periodicity`。
    pub periodicity: Vec<f64>,
    /// 相位折叠覆盖率。基准 `coverageByLag`。
    pub coverage_by_lag: Vec<f64>,
    /// 低音规律性很强（打击乐模式）。基准 `percussive`。
    pub percussive: bool,
}

/// 节奏先验，以 110 BPM 为中心、0.75 个八度为标准差的对数高斯。基准：`p1`。
fn tempo_prior(bpm: f64) -> f64 {
    let h = (bpm / 110.0).log2() / 0.75;
    (-0.5 * h * h).exp()
}

/// 在得分峰附近做抛物线插值。基准：`estimate` 里的 `ft`。
fn refine(scores: &[f64], lag: i64) -> f64 {
    let a = at(scores, lag - 1);
    let b = at(scores, lag);
    let c = at(scores, lag + 1);
    let g = a - 2.0 * b + c;
    lag as f64
        + if g != 0.0 {
            clamp(0.5 * (a - c) / g, -0.5, 0.5)
        } else {
            0.0
        }
}

/// 在 `center - 1..=center + 1` 里找得分最高的周期（同分取先出现的）。
fn best_near(scores: &[f64], center: i64) -> i64 {
    let mut best = center;
    for d in -1..=1 {
        if at(scores, center + d) > at(scores, best) {
            best = center + d;
        }
    }
    best
}

impl BeatDetector {
    /// 节奏估计。基准：`x1.estimate(h = 1200)`。可用数据不足或没有明显周期时返回 `None`。
    pub(super) fn estimate(&self, window_bins: usize) -> Option<Estimate> {
        let window = self.window(window_bins);
        let n = window.n;
        if n < 120 {
            return None;
        }
        let v = &window.v;
        let now_ms = self.last_bin as f64 * BIN_MS;
        let strong_bass = self
            .bass_onsets
            .iter()
            .filter(|o| now_ms - o.time < 4200.0 && o.strength > 0.65)
            .count();

        // 低音 ODF 在某个周期上高度自相关：认为是规律的打击乐，改用更短的记忆。
        let mut percussive = false;
        if strong_bass >= 3 {
            let mut e = 0.0;
            let mut w_sum = 0.0;
            for (&b, &w) in window.bass.iter().zip(&window.weights) {
                let b = f64::from(b);
                e += b * b * f64::from(w);
                w_sum += f64::from(w);
            }
            let mean = e / w_sum;
            if mean > 0.002 {
                for lag in MIN_LAG..=MAX_LAG {
                    if acf(&window.bass, &window.weights, n, lag) / mean > 0.65 {
                        percussive = true;
                        break;
                    }
                }
            }
        }
        let mut weights = window.weights;
        if percussive {
            weights.copy_from_slice(&self.percussive_weights[..n]);
        }

        let mut energy = 0.0; // U
        let mut w_sum = 0.0; // M
        let mut total = 0.0; // K
        for (&x, &w) in v.iter().zip(&weights) {
            let (x, w) = (f64::from(x), f64::from(w));
            energy += x * x * w;
            total += x * w;
            w_sum += w;
        }
        energy = if w_sum > 0.0 { energy / w_sum } else { 0.0 };
        if energy < 0.002 || total <= 0.0 {
            return None;
        }

        // 各频带 flux 去均值后的序列与方差（基准 `B`）。
        let spectral: Vec<(Vec<f32>, f64)> = self
            .spectral_odf
            .iter()
            .map(|band| {
                let mut values = Vec::with_capacity(n);
                let mut mean = 0.0;
                for (g, &w) in weights.iter().enumerate() {
                    let x = band[super::odf::ring(self.last_bin - g as i64)];
                    values.push(x);
                    mean += f64::from(x) * f64::from(w);
                }
                mean /= w_sum;
                let mut var = 0.0;
                for (x, &w) in values.iter_mut().zip(&weights) {
                    *x = (f64::from(*x) - mean) as f32;
                    let d = f64::from(*x);
                    var += d * d * f64::from(w);
                }
                (values, var / w_sum)
            })
            .collect();

        let size = (MAX_LAG + 2) as usize;
        let mut scores = vec![0.0_f64; size]; // H
        let mut coverage = vec![0.0_f64; size]; // nt
        let mut periodicity = vec![0.0_f64; size]; // et
        let last = self.last_bin;
        let mut fold = Vec::with_capacity(size);
        for lag in MIN_LAG - 1..=MAX_LAG + 1 {
            let li = lag as usize;
            let corr = acf(v, &weights, n, lag) / energy;
            fold.clear();
            fold.resize(li, 0.0_f64);
            for (d, (&x, &w)) in v.iter().zip(&weights).enumerate() {
                let z = (last - d as i64).rem_euclid(lag) as usize;
                fold[z] += f64::from(x) * f64::from(w);
            }
            let mut best = 0.0;
            for d in 0..li {
                let z = fold[d] + fold[(d + 1) % li] + fold[(d + li - 1) % li];
                if z > best {
                    best = z;
                }
            }
            coverage[li] = best / total;
            let mut g = 0.0;
            for (d, (values, var)) in spectral.iter().enumerate() {
                if *var > 1e-7 {
                    let band_weight = if d < 5 {
                        0.03
                    } else if d < 10 {
                        0.04
                    } else {
                        0.1083333333
                    };
                    g += band_weight * jsmath::max(0.0, acf(values, &weights, n, lag) / var);
                }
            }
            periodicity[li] = g;
            let tail = if percussive {
                1.0
            } else {
                (lag as f64 / 50.0).powf(0.6) * (0.1 + g)
            };
            scores[li] = jsmath::max(0.0, corr).sqrt()
                * coverage[li]
                * tempo_prior(6e3 / lag as f64).sqrt()
                * tail;
        }

        // 取局部峰中的最高者；没有局部峰时取全局最大。
        let mut lag: i64 = -1; // X
        let mut peak = 0.0; // rt
        let mut score_sum = 0.0; // Ct
        for t in MIN_LAG..=MAX_LAG {
            let s = at(&scores, t);
            score_sum += s;
            if t > MIN_LAG
                && t < MAX_LAG
                && s > at(&scores, t - 1)
                && s >= at(&scores, t + 1)
                && s > peak
            {
                peak = s;
                lag = t;
            }
        }
        if lag < 0 {
            for t in MIN_LAG..=MAX_LAG {
                if at(&scores, t) > peak {
                    peak = at(&scores, t);
                    lag = t;
                }
            }
            if lag < 0 || peak <= 0.0 {
                return None;
            }
        }

        let sc = |i: i64| at(&scores, i);
        let per = |i: i64| at(&periodicity, i);
        let cov = |i: i64| at(&coverage, i);
        let mut metrical = false; // st

        // 4/3：峰落在附点周期上时，改选 4/3 周期（及其一半也成立）。
        if !percussive {
            let up = round_i64(lag as f64 * 4.0 / 3.0);
            let down = round_i64(up as f64 / 2.0);
            if up <= MAX_LAG && down >= MIN_LAG {
                let weak_bass = strong_bass < 3;
                let mut a = up;
                let mut b = down;
                for d in -1..=1 {
                    if sc(up + d) > sc(a) {
                        a = up + d;
                    }
                    if sc(down + d) > sc(b) {
                        b = down + d;
                    }
                }
                let x = sc(lag);
                if sc(a) >= x * if weak_bass { 0.22 } else { 0.5 }
                    && sc(b) >= x * 0.3
                    && sc(a) + sc(b) >= x * if weak_bass { 0.7 } else { 0.9 }
                    && per(a) >= per(lag) * if weak_bass { 0.25 } else { 0.45 }
                    && per(b) >= per(lag) * 0.6
                    && cov(a) >= cov(lag) * if weak_bass { 0.6 } else { 0.85 }
                {
                    lag = a;
                    metrical = true;
                }
            }
        }

        // 2/3（结果慢于 145 BPM 时）。
        let two_thirds = round_i64(lag as f64 * 2.0 / 3.0);
        if (MIN_LAG..=MAX_LAG).contains(&two_thirds) && 6e3 / (two_thirds as f64) < 145.0 {
            let t = best_near(&scores, two_thirds);
            if sc(t) >= sc(lag) * 0.65 && per(t) >= per(lag) * 0.8 && cov(t) >= cov(lag) * 1.2 {
                lag = t;
                metrical = true;
            }
        }

        // ×2：快于 145 BPM 且低音不支持该周期时，改用两倍周期。
        if 6e3 / lag as f64 > 145.0 && lag * 2 <= MAX_LAG {
            let t = best_near(&scores, lag * 2);
            if !percussive
                && sc(t) >= sc(lag) * 0.5
                && per(t) > per(lag) * 0.95
                && !self.bass_cadence_supports(now_ms, lag as f64 * BIN_MS, 3, 0.9)
            {
                lag = t;
                metrical = true;
            }
        }

        // ÷2：低音按半周期规律出现时，改用一半周期。
        let half = round_i64(lag as f64 / 2.0);
        if half >= MIN_LAG {
            let t = best_near(&scores, half);
            if sc(t) >= sc(lag) * 0.25
                && self.bass_cadence_supports(now_ms, t as f64 * BIN_MS, 3, 0.55)
            {
                lag = t;
                metrical = true;
            }
        }

        // 低音 1.5 倍：强低音以 2/3 周期的两倍间隔出现时，改选 2/3 周期。
        let mut bass_override = false;
        let strong: Vec<BassOnset> = self
            .bass_onsets
            .iter()
            .filter(|o| now_ms - o.time < 4500.0 && o.strength > 0.9)
            .copied()
            .collect();
        if strong.len() >= 2 {
            let t = round_i64(lag as f64 / 1.5);
            let span = t as f64 * 2.0 * BIN_MS;
            let mut pairs = 0;
            for (i, first) in strong.iter().enumerate() {
                if strong[i + 1..]
                    .iter()
                    .any(|o| (o.time - first.time - span).abs() < span * 0.07)
                {
                    pairs += 1;
                }
            }
            if pairs >= if self.locked { 2 } else { 1 }
                && (MIN_LAG..=MAX_LAG).contains(&t)
                && 6e3 / t as f64 <= 130.0
                && (lag as f64 / t as f64 - 1.5).abs() < 0.13
            {
                lag = t;
                bass_override = true;
            }
        }

        let mean_score = score_sum / (MAX_LAG - MIN_LAG + 1) as f64;
        let conf = if mean_score > 0.0 {
            peak / mean_score
        } else {
            0.0
        };
        Some(Estimate {
            raw_lag: lag,
            raw_f: if bass_override {
                lag as f64
            } else {
                refine(&scores, lag)
            },
            raw_phases: self.phases_for(v, &weights, n, lag),
            conf,
            scores,
            bass_override,
            metrical_override: metrical,
            periodicity,
            coverage_by_lag: coverage,
            percussive,
        })
    }
}
