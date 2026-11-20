//! Pure-Rust port of the `x1` beat detection engine with working `sensitivity` scaling.
//!
//! Algorithm structure:
//! - 16 logarithmic frequency bands (`40 Hz .. 12 kHz`, 1.5x bass weight below 250 Hz)
//! - Adaptive half-wave rectified spectral flux -> 10 ms (100 Hz) ODF ring buffer (16 s)
//! - Weighted autocorrelation (`acf`), 3-bin phase-folding coverage, 16-band spectral
//!   periodicity, and log-normal 110 BPM prior (`p1`)
//! - Percussive mode switching, metrical (4/3, 2/3) / octave (2x, 0.5x) / bass-cadence
//!   disambiguation, and parabolic sub-bin lag interpolation
//! - 92/8 PLL lag tracking, ±8 ms phase nudging, fresh tempo-change detection, and
//!   9-sample rolling median BPM hysteresis
//! - `sensitivity` (`1.05..=2.0`, baseline `1.22`) scales flux gate, onset thresholds,
//!   and silence gate identically to baseline at `1.22`.

use crate::fft::FREQ_BINS;
use pet_core::{clamp_sensitivity, DEFAULT_SENSITIVITY};
use std::collections::VecDeque;

const BIN_MS: f64 = 10.0; // Lt
const RING_LEN: usize = 1600; // Ie (16 s)
const EST_WIN: usize = 1200; // Rh (12 s)
const LAG_MIN: usize = 30; // Rl (300 ms = 200 BPM)
const LAG_MAX: usize = 100; // sl (1000 ms = 60 BPM)
const DECAY_LONG: f64 = 400.0; // b1 (4 s)
const EST_INTERVAL_MS: f64 = 200.0; // S1
const NUM_BANDS: usize = 16; // pl

#[inline]
fn clamp_f64(val: f64, lo: f64, hi: f64) -> f64 {
    val.max(lo).min(hi)
}

/// Log-normal tempo prior centered at 110 BPM (sigma = 0.75 octaves).
#[inline]
fn tempo_prior_p1(bpm: f64) -> f64 {
    let h = (bpm / 110.0).log2() / 0.75;
    (-0.5 * h * h).exp()
}

#[derive(Debug, Clone, Copy)]
struct BassOnset {
    time_ms: f64,
    strength: f64,
}

#[derive(Debug, Clone, Copy)]
struct BpmSample {
    at_ms: f64,
    bpm: u16,
}

/// Single-frame output from `BeatDetector::process`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BeatFrame {
    pub beat: bool,
    pub onset: bool,
    pub strength: f32,
    pub level: f32,
    pub bass: f32,
    pub bpm: u16,
    pub beats: u64,
    pub locked: bool,
    pub next_beat_at_ms: Option<f64>,
    pub period_ms: Option<f64>,
}

struct WindowSlice {
    v: Vec<f32>,
    bass: Vec<f32>,
    r: Vec<f32>,
    n: usize,
}

struct EstimateResult {
    raw_lag: usize,
    raw_f: f64,
    raw_phases: Vec<isize>,
    conf: f64,
    scores: [f64; LAG_MAX + 2],
    bass_override: bool,
    metrical_override: bool,
    periodicity: [f64; LAG_MAX + 2],
    coverage_by_lag: [f64; LAG_MAX + 2],
    percussive: bool,
}

/// Clean Rust port of the `x1` beat & BPM tracker.
pub struct BeatDetector {
    sensitivity: f32,
    odf: [f32; RING_LEN],
    bass_odf: [f32; RING_LEN],
    spectral_odf: [[f32; RING_LEN]; NUM_BANDS],
    spectral_frame: [f32; NUM_BANDS],
    last_bin: isize,
    first_bin: isize,
    prev_e: [f32; NUM_BANDS],
    band_lo: Vec<usize>,
    band_hi: Vec<usize>,
    band_w: Vec<f64>,
    bands_sample_rate: u32,
    peak_flux: f64,
    band_peak: [f64; NUM_BANDS],
    last_frame_at: f64,
    level: f64,
    loudest: f64,
    silent_since: f64,
    last_onset_bin: isize,
    last_bass_onset_bin: isize,
    bass_onsets: Vec<BassOnset>,
    locked: bool,
    locked_lag_ms: f64,
    bass_protected: bool,
    next_beat: f64,
    last_beat_at: f64,
    pending_lag: f64,
    pending_count: usize,
    low_conf_since: f64,
    sound_since: f64,
    last_est_at: f64,
    last_strength: f64,
    beat_count: u64,
    shown_bpm: u16,
    shown_cand: u16,
    shown_cand_at: f64,
    shown_samples: VecDeque<BpmSample>,
    shown_median: u16,
    fresh_lag: f64,
    fresh_count: usize,
    last_fresh_at: f64,
    committed_at: f64,
    acquired_at: f64,
    initial_octave_corrected: bool,
}

impl Default for BeatDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl BeatDetector {
    pub fn new() -> Self {
        Self {
            sensitivity: DEFAULT_SENSITIVITY,
            odf: [0.0; RING_LEN],
            bass_odf: [0.0; RING_LEN],
            spectral_odf: [[0.0; RING_LEN]; NUM_BANDS],
            spectral_frame: [0.0; NUM_BANDS],
            last_bin: -1,
            first_bin: -1,
            prev_e: [0.0; NUM_BANDS],
            band_lo: Vec::with_capacity(NUM_BANDS),
            band_hi: Vec::with_capacity(NUM_BANDS),
            band_w: Vec::with_capacity(NUM_BANDS),
            bands_sample_rate: 0,
            peak_flux: 0.05,
            band_peak: [0.018; NUM_BANDS],
            last_frame_at: 0.0,
            level: 0.0,
            loudest: 0.0,
            silent_since: 0.0,
            last_onset_bin: -99,
            last_bass_onset_bin: -99,
            bass_onsets: Vec::with_capacity(32),
            locked: false,
            locked_lag_ms: 500.0,
            bass_protected: false,
            next_beat: 0.0,
            last_beat_at: f64::NEG_INFINITY,
            pending_lag: 0.0,
            pending_count: 0,
            low_conf_since: 0.0,
            sound_since: 0.0,
            last_est_at: 0.0,
            last_strength: 0.8,
            beat_count: 0,
            shown_bpm: 0,
            shown_cand: 0,
            shown_cand_at: 0.0,
            shown_samples: VecDeque::with_capacity(10),
            shown_median: 0,
            fresh_lag: 0.0,
            fresh_count: 0,
            last_fresh_at: 0.0,
            committed_at: 0.0,
            acquired_at: 0.0,
            initial_octave_corrected: false,
        }
    }

    pub fn sensitivity(&self) -> f32 {
        self.sensitivity
    }

    pub fn set_sensitivity(&mut self, value: f32) {
        self.sensitivity = clamp_sensitivity(value);
    }

    pub fn reset(&mut self) {
        self.odf.fill(0.0);
        self.bass_odf.fill(0.0);
        for band in &mut self.spectral_odf {
            band.fill(0.0);
        }
        self.spectral_frame.fill(0.0);
        self.last_bin = -1;
        self.first_bin = -1;
        self.prev_e.fill(0.0);
        self.band_lo.clear();
        self.band_hi.clear();
        self.band_w.clear();
        self.bands_sample_rate = 0;
        self.peak_flux = 0.05;
        self.band_peak.fill(0.018);
        self.last_frame_at = 0.0;
        self.level = 0.0;
        self.loudest = 0.0;
        self.silent_since = 0.0;
        self.last_onset_bin = -99;
        self.last_bass_onset_bin = -99;
        self.bass_onsets.clear();
        self.locked = false;
        self.locked_lag_ms = 500.0;
        self.bass_protected = false;
        self.next_beat = 0.0;
        self.last_beat_at = f64::NEG_INFINITY;
        self.pending_lag = 0.0;
        self.pending_count = 0;
        self.low_conf_since = 0.0;
        self.sound_since = 0.0;
        self.last_est_at = 0.0;
        self.last_strength = 0.8;
        self.beat_count = 0;
        self.shown_bpm = 0;
        self.shown_cand = 0;
        self.shown_cand_at = 0.0;
        self.shown_samples.clear();
        self.shown_median = 0;
        self.fresh_lag = 0.0;
        self.fresh_count = 0;
        self.last_fresh_at = 0.0;
        self.committed_at = 0.0;
        self.acquired_at = 0.0;
        self.initial_octave_corrected = false;
    }

    #[inline]
    fn ring(idx: isize) -> usize {
        idx.rem_euclid(RING_LEN as isize) as usize
    }

    fn ensure_bands(&mut self, sample_rate: u32, num_bins: usize) {
        if !self.band_lo.is_empty() && self.bands_sample_rate == sample_rate {
            return;
        }
        self.band_lo.clear();
        self.band_hi.clear();
        self.band_w.clear();
        self.bands_sample_rate = sample_rate;

        let bin_hz = (sample_rate as f64) / 2.0 / (num_bins as f64);
        let f_lo = 40.0_f64;
        let f_hi = 12_000.0_f64;
        let ratio = f_hi / f_lo;

        for x in 0..NUM_BANDS {
            let f0 = f_lo * ratio.powf((x as f64) / (NUM_BANDS as f64));
            let f1 = f_lo * ratio.powf(((x + 1) as f64) / (NUM_BANDS as f64));
            let lo = (f0 / bin_hz).floor().max(1.0) as usize;
            let hi = (f1 / bin_hz)
                .ceil()
                .max((f0 / bin_hz).floor())
                .min((num_bins - 1) as f64) as usize;
            self.band_lo.push(lo.min(num_bins - 1));
            self.band_hi.push(hi.max(lo.min(num_bins - 1)));
            self.band_w.push(if f0 < 250.0 { 1.5 } else { 1.0 });
        }
    }

    fn band_energies(&self, freq: &[u8; FREQ_BINS], out: &mut [f32; NUM_BANDS]) {
        for f in 0..NUM_BANDS {
            let lo = self.band_lo[f];
            let hi = self.band_hi[f];
            let mut sum = 0u32;
            for &byte in &freq[lo..=hi] {
                sum += byte as u32;
            }
            out[f] = (sum as f32) / ((hi - lo + 1) as f32) / 255.0;
        }
    }

    fn write_odf(&mut self, now_ms: f64, odf_val: f32, bass_val: f32, gap: bool) {
        let bin = (now_ms / BIN_MS).round() as isize;
        if self.last_bin < 0 || gap || bin - self.last_bin > (RING_LEN as isize) / 2 {
            self.first_bin = bin;
            if gap {
                self.odf.fill(0.0);
                self.bass_odf.fill(0.0);
                for band in &mut self.spectral_odf {
                    band.fill(0.0);
                }
            }
            let r = Self::ring(bin);
            self.odf[r] = odf_val;
            self.bass_odf[r] = bass_val;
            for n in 0..NUM_BANDS {
                self.spectral_odf[n][r] = self.spectral_frame[n];
            }
            self.last_bin = bin;
            return;
        }
        for n in (self.last_bin + 1)..=bin {
            let r = Self::ring(n);
            self.odf[r] = odf_val;
            self.bass_odf[r] = bass_val;
            for band in 0..NUM_BANDS {
                self.spectral_odf[band][r] = self.spectral_frame[band];
            }
        }
        self.last_bin = bin;
    }

    fn window(&self, max_len: usize) -> WindowSlice {
        let avail = (self.last_bin - self.first_bin + 1).max(0) as usize;
        let n = max_len.min(avail);
        let mut v = vec![0.0_f32; n];
        let mut bass = vec![0.0_f32; n];
        let mut r = vec![0.0_f32; n];
        for i in 0..n {
            let idx = Self::ring(self.last_bin - i as isize);
            v[i] = self.odf[idx];
            bass[i] = self.bass_odf[idx];
            r[i] = (-(i as f64) / DECAY_LONG).exp() as f32;
        }
        WindowSlice { v, bass, r, n }
    }

    #[inline]
    fn acf(signal: &[f32], weights: &[f32], n: usize, lag: usize) -> f64 {
        if lag + 4 >= n {
            return 0.0;
        }
        let mut acc = 0.0_f64;
        let mut wsum = 0.0_f64;
        let limit = n - lag;
        for i in 0..limit {
            let w = weights[i] as f64;
            acc += (signal[i] as f64) * (signal[i + lag] as f64) * w;
            wsum += w;
        }
        if wsum > 0.0 {
            acc / wsum
        } else {
            0.0
        }
    }

    fn phases_for(&self, v: &[f32], r: &[f32], n: usize, lag: usize) -> Vec<isize> {
        let last = self.last_bin;
        let lag_i = lag as isize;
        let mut num = vec![0.0_f64; lag];
        let mut den = vec![0.0_f64; lag];
        for i in 0..n {
            let slot = (last - i as isize).rem_euclid(lag_i) as usize;
            let w = r[i] as f64;
            num[slot] += (v[i] as f64) * w;
            den[slot] += w;
        }
        let mut u = vec![0.0_f64; lag];
        let mut max_u = 0.0_f64;
        for b in 0..lag {
            let prev = (b + lag - 1) % lag;
            let next = (b + 1) % lag;
            let d = den[prev] + den[b] + den[next];
            let val = if d > 0.0 {
                (num[prev] + num[b] + num[next]) / d
            } else {
                0.0
            };
            u[b] = val;
            if val > max_u {
                max_u = val;
            }
        }
        let mut phases = Vec::new();
        if max_u > 0.0 {
            for b in 0..lag {
                let prev = (b + lag - 1) % lag;
                let next = (b + 1) % lag;
                if u[b] >= 0.95 * max_u && u[b] >= u[next] && u[b] >= u[prev] {
                    phases.push(last - (last - b as isize).rem_euclid(lag_i));
                }
            }
        }
        phases
    }

    fn bass_cadence_supports(
        &self,
        now_ms: f64,
        period_ms: f64,
        min_pairs: usize,
        strict: bool,
        min_strength_override: Option<f64>,
    ) -> bool {
        let min_strength = min_strength_override.unwrap_or(if strict { 0.9 } else { 0.0 });
        let recent: Vec<f64> = self
            .bass_onsets
            .iter()
            .filter(|o| now_ms - o.time_ms < 4200.0 && o.strength > min_strength)
            .map(|o| o.time_ms)
            .collect();
        let mut pairs = 0usize;
        for i in 0..recent.len() {
            let t0 = recent[i];
            if recent[(i + 1)..]
                .iter()
                .any(|&t1| (t1 - t0 - period_ms).abs() < period_ms * 0.12)
            {
                pairs += 1;
            }
        }
        pairs >= min_pairs
    }

    fn bass_anchor(&self, now_ms: f64, period_ms: f64) -> Option<f64> {
        let recent: Vec<BassOnset> = self
            .bass_onsets
            .iter()
            .copied()
            .filter(|o| now_ms - o.time_ms < 4500.0)
            .collect();
        if recent.len() < 2 {
            return None;
        }
        let mut best_score = 0.0_f64;
        let mut best_time = recent.last().unwrap().time_ms;
        for &anchor in &recent {
            let mut count = 0usize;
            let mut str_sum = 0.0_f64;
            for &other in &recent {
                let k = (other.time_ms - anchor.time_ms) / period_ms;
                if (k - k.round()).abs() < 0.15 {
                    count += 1;
                    str_sum += other.strength;
                }
            }
            let score = (count as f64) + str_sum * 0.2;
            if score > best_score {
                best_score = score;
                best_time = anchor.time_ms;
            }
        }
        if best_score >= 2.2 {
            Some(best_time)
        } else {
            None
        }
    }

    fn estimate(&self, win_len: usize) -> Option<EstimateResult> {
        let mut win = self.window(win_len);
        let x = win.n;
        if x < 120 {
            return None;
        }

        let now_bin_ms = (self.last_bin as f64) * BIN_MS;
        let strong_bass_count = self
            .bass_onsets
            .iter()
            .filter(|o| now_bin_ms - o.time_ms < 4200.0 && o.strength > 0.65)
            .count();

        let mut percussive = false;
        if strong_bass_count >= 3 {
            let mut b_pow = 0.0_f64;
            let mut w_sum = 0.0_f64;
            for i in 0..x {
                let b = win.bass[i] as f64;
                let w = win.r[i] as f64;
                b_pow += b * b * w;
                w_sum += w;
            }
            let p = if w_sum > 0.0 { b_pow / w_sum } else { 0.0 };
            if p > 0.002 {
                for lag in LAG_MIN..=LAG_MAX {
                    if Self::acf(&win.bass, &win.r, x, lag) / p > 0.65 {
                        percussive = true;
                        break;
                    }
                }
            }
        }

        if percussive {
            for i in 0..x {
                win.r[i] = (-(i as f64) / 160.0).exp() as f32;
            }
        }

        let mut u_pow = 0.0_f64;
        let mut w_sum = 0.0_f64;
        let mut k_sum = 0.0_f64;
        for i in 0..x {
            let v = win.v[i] as f64;
            let w = win.r[i] as f64;
            u_pow += v * v * w;
            k_sum += v * w;
            w_sum += w;
        }
        let u_mean = if w_sum > 0.0 { u_pow / w_sum } else { 0.0 };
        if u_mean < 0.002 || k_sum <= 0.0 {
            return None;
        }

        // Mean-centered per-band spectral ODFs for periodicity scoring.
        let mut band_series: Vec<(Vec<f32>, f64)> = Vec::with_capacity(NUM_BANDS);
        for band in 0..NUM_BANDS {
            let mut vals = vec![0.0_f32; x];
            let mut mean = 0.0_f64;
            for i in 0..x {
                let val = self.spectral_odf[band][Self::ring(self.last_bin - i as isize)];
                vals[i] = val;
                mean += (val as f64) * (win.r[i] as f64);
            }
            mean /= w_sum;
            let mut var = 0.0_f64;
            for i in 0..x {
                let centered = (vals[i] as f64) - mean;
                vals[i] = centered as f32;
                var += centered * centered * (win.r[i] as f64);
            }
            band_series.push((vals, var / w_sum));
        }

        let mut scores = [0.0_f64; LAG_MAX + 2];
        let mut coverage = [0.0_f64; LAG_MAX + 2];
        let mut periodicity = [0.0_f64; LAG_MAX + 2];
        let last_bin = self.last_bin;

        let mut folded = vec![0.0_f64; LAG_MAX + 2];
        for lag in (LAG_MIN - 1)..=(LAG_MAX + 1) {
            let acf_norm = Self::acf(&win.v, &win.r, x, lag) / u_mean;

            folded[..lag].fill(0.0);
            let lag_i = lag as isize;
            for i in 0..x {
                let slot = (last_bin - i as isize).rem_euclid(lag_i) as usize;
                folded[slot] += (win.v[i] as f64) * (win.r[i] as f64);
            }
            let mut best_cover = 0.0_f64;
            for d in 0..lag {
                let tri = folded[d] + folded[(d + 1) % lag] + folded[(d + lag - 1) % lag];
                if tri > best_cover {
                    best_cover = tri;
                }
            }
            coverage[lag] = best_cover / k_sum;

            let mut per = 0.0_f64;
            for (band_idx, (vals, var)) in band_series.iter().enumerate() {
                if *var > 1e-7 {
                    let weight = if band_idx < 5 {
                        0.03
                    } else if band_idx < 10 {
                        0.04
                    } else {
                        0.1083333333
                    };
                    let band_acf = Self::acf(vals, &win.r, x, lag) / *var;
                    per += weight * band_acf.max(0.0);
                }
            }
            periodicity[lag] = per;

            let lag_f = lag as f64;
            let prior = tempo_prior_p1(6000.0 / lag_f).sqrt();
            let shape = if percussive {
                1.0
            } else {
                (lag_f / 50.0).powf(0.6) * (0.1 + per)
            };
            scores[lag] = acf_norm.max(0.0).sqrt() * coverage[lag] * prior * shape;
        }

        // Pick best interior local peak in LAG_MIN..=LAG_MAX; fallback to global max.
        let mut best_lag: Option<usize> = None;
        let mut best_score = 0.0_f64;
        let mut score_sum = 0.0_f64;
        for lag in LAG_MIN..=LAG_MAX {
            score_sum += scores[lag];
            if lag > LAG_MIN
                && lag < LAG_MAX
                && scores[lag] > scores[lag - 1]
                && scores[lag] >= scores[lag + 1]
                && scores[lag] > best_score
            {
                best_score = scores[lag];
                best_lag = Some(lag);
            }
        }
        let mut x_lag = match best_lag {
            Some(l) => l,
            None => {
                let mut fallback = None;
                for lag in LAG_MIN..=LAG_MAX {
                    if scores[lag] > best_score {
                        best_score = scores[lag];
                        fallback = Some(lag);
                    }
                }
                match fallback {
                    Some(l) if best_score > 0.0 => l,
                    _ => return None,
                }
            }
        };

        let mut metrical_override = false;

        // 1) 4/3 metrical override (non-percussive)
        if !percussive {
            let t = ((x_lag as f64) * 4.0 / 3.0).round() as usize;
            let f = ((t as f64) / 2.0).round() as usize;
            if t <= LAG_MAX && f >= LAG_MIN {
                let low_bass = strong_bass_count < 3;
                let mut best_t = t;
                let mut best_f = f;
                for d in -1_isize..=1 {
                    let cand_t = (t as isize + d) as usize;
                    let cand_f = (f as isize + d) as usize;
                    if scores[cand_t] > scores[best_t] {
                        best_t = cand_t;
                    }
                    if scores[cand_f] > scores[best_f] {
                        best_f = cand_f;
                    }
                }
                if scores[best_t] >= scores[x_lag] * if low_bass { 0.22 } else { 0.5 }
                    && scores[best_f] >= scores[x_lag] * 0.3
                    && scores[best_t] + scores[best_f]
                        >= scores[x_lag] * if low_bass { 0.7 } else { 0.9 }
                    && periodicity[best_t] >= periodicity[x_lag] * if low_bass { 0.25 } else { 0.45 }
                    && periodicity[best_f] >= periodicity[x_lag] * 0.6
                    && coverage[best_t] >= coverage[x_lag] * if low_bass { 0.6 } else { 0.85 }
                {
                    x_lag = best_t;
                    metrical_override = true;
                }
            }
        }

        // 2) 2/3 metrical override (<145 BPM)
        let dt = ((x_lag as f64) * 2.0 / 3.0).round() as usize;
        if dt >= LAG_MIN && dt <= LAG_MAX && 6000.0 / (dt as f64) < 145.0 {
            let mut best_t = dt;
            for d in -1_isize..=1 {
                let cand = (dt as isize + d) as usize;
                if scores[cand] > scores[best_t] {
                    best_t = cand;
                }
            }
            if scores[best_t] >= scores[x_lag] * 0.65
                && periodicity[best_t] >= periodicity[x_lag] * 0.8
                && coverage[best_t] >= coverage[x_lag] * 1.2
            {
                x_lag = best_t;
                metrical_override = true;
            }
        }

        // 3) Half-time octave override (>145 BPM -> 2x lag)
        if 6000.0 / (x_lag as f64) > 145.0 && x_lag * 2 <= LAG_MAX {
            let base = x_lag * 2;
            let mut best_t = base;
            for d in -1_isize..=1 {
                let cand = (base as isize + d) as usize;
                if scores[cand] > scores[best_t] {
                    best_t = cand;
                }
            }
            if !percussive
                && scores[best_t] >= scores[x_lag] * 0.5
                && periodicity[best_t] > periodicity[x_lag] * 0.95
                && !self.bass_cadence_supports(now_bin_ms, (x_lag as f64) * BIN_MS, 3, true, None)
            {
                x_lag = best_t;
                metrical_override = true;
            }
        }

        // 4) Double-time octave override (half lag supported by bass cadence)
        let half_lag = ((x_lag as f64) / 2.0).round() as usize;
        if half_lag >= LAG_MIN {
            let mut best_t = half_lag;
            for d in -1_isize..=1 {
                let cand = (half_lag as isize + d) as usize;
                if scores[cand] > scores[best_t] {
                    best_t = cand;
                }
            }
            if scores[best_t] >= scores[x_lag] * 0.25
                && self.bass_cadence_supports(
                    now_bin_ms,
                    (best_t as f64) * BIN_MS,
                    3,
                    false,
                    Some(0.55),
                )
            {
                x_lag = best_t;
                metrical_override = true;
            }
        }

        // 5) Strong-bass 1.5x cadence override
        let mut bass_override = false;
        let strong_bass: Vec<BassOnset> = self
            .bass_onsets
            .iter()
            .copied()
            .filter(|o| now_bin_ms - o.time_ms < 4500.0 && o.strength > 0.9)
            .collect();
        if strong_bass.len() >= 2 {
            let t = ((x_lag as f64) / 1.5).round() as usize;
            let f_ms = (t as f64) * 2.0 * BIN_MS;
            let mut pairs = 0usize;
            for i in 0..strong_bass.len() {
                if strong_bass[(i + 1)..]
                    .iter()
                    .any(|o| (o.time_ms - strong_bass[i].time_ms - f_ms).abs() < f_ms * 0.07)
                {
                    pairs += 1;
                }
            }
            let min_pairs = if self.locked { 2 } else { 1 };
            if pairs >= min_pairs
                && t >= LAG_MIN
                && t <= LAG_MAX
                && 6000.0 / (t as f64) <= 130.0
                && ((x_lag as f64) / (t as f64) - 1.5).abs() < 0.13
            {
                x_lag = t;
                bass_override = true;
            }
        }

        let mean_score = score_sum / ((LAG_MAX - LAG_MIN + 1) as f64);
        let conf = if mean_score > 0.0 {
            best_score / mean_score
        } else {
            0.0
        };

        let interp = |lag: usize| -> f64 {
            let prev = scores[lag - 1];
            let curr = scores[lag];
            let next = scores[lag + 1];
            let denom = prev - 2.0 * curr + next;
            let delta = if denom != 0.0 {
                clamp_f64(0.5 * (prev - next) / denom, -0.5, 0.5)
            } else {
                0.0
            };
            (lag as f64) + delta
        };

        let raw_f = if bass_override {
            x_lag as f64
        } else {
            interp(x_lag)
        };
        let raw_phases = self.phases_for(&win.v, &win.r, x, x_lag);

        Some(EstimateResult {
            raw_lag: x_lag,
            raw_f,
            raw_phases,
            conf,
            scores,
            bass_override,
            metrical_override,
            periodicity,
            coverage_by_lag: coverage,
            percussive,
        })
    }

    fn next_grid_after(now_ms: f64, phase_bin: isize, period_ms: f64) -> f64 {
        let mut x = (phase_bin as f64) * BIN_MS;
        if x <= now_ms {
            x += ((now_ms - x) / period_ms).ceil() * period_ms;
        }
        if x <= now_ms {
            x += period_ms;
        }
        x
    }

    fn pick_phase(&self, phases: &[isize], period_ms: f64, now_ms: f64) -> isize {
        if phases.is_empty() {
            return (now_ms / BIN_MS).floor() as isize;
        }
        if !self.locked {
            return phases[0];
        }
        let mut best = phases[0];
        let mut best_dist = f64::INFINITY;
        for &cand in phases {
            let r = ((cand as f64) * BIN_MS - self.next_beat).rem_euclid(period_ms);
            let dist = r.min(period_ms - r);
            if dist < best_dist {
                best_dist = dist;
                best = cand;
            }
        }
        best
    }

    fn commit_with(
        &mut self,
        now_ms: f64,
        phases: &[isize],
        lag_ms: f64,
        bass_override: bool,
        percussive: bool,
    ) {
        if self.locked {
            if ((lag_ms / self.locked_lag_ms).log2().abs() - 1.0).abs() < 0.08 {
                self.initial_octave_corrected = true;
            }
        } else {
            self.acquired_at = now_ms;
            self.initial_octave_corrected = false;
        }

        if !self.locked || (lag_ms / self.locked_lag_ms - 1.0).abs() > 0.12 {
            self.shown_samples.clear();
            self.shown_median = 0;
            self.shown_cand = 0;
        }

        if self.locked && percussive && (lag_ms / self.locked_lag_ms - 1.0).abs() > 0.12 {
            self.first_bin = self.first_bin.max(self.last_bin - 250);
            self.bass_onsets.retain(|o| now_ms - o.time_ms < 2500.0);
        }

        self.locked = true;
        self.committed_at = now_ms;
        self.locked_lag_ms = lag_ms;
        self.bass_protected = bass_override;

        let anchor = if bass_override {
            self.bass_anchor(now_ms, lag_ms)
        } else {
            None
        };
        let phase_bin = match anchor {
            Some(t) => (t / BIN_MS).round() as isize,
            None => self.pick_phase(phases, lag_ms, now_ms),
        };
        self.next_beat = Self::next_grid_after(now_ms, phase_bin, lag_ms);
        self.pending_count = 0;
        self.pending_lag = 0.0;
        self.low_conf_since = 0.0;
        self.fresh_count = 0;
        self.fresh_lag = 0.0;
    }

    fn update_lock(&mut self, now_ms: f64, est: Option<EstimateResult>) {
        let Some(m) = est else {
            self.pending_count = 0;
            return;
        };

        if self.sound_since == 0.0 {
            self.sound_since = now_ms;
        }

        if !self.locked {
            let cand_lag_ms = m.raw_f * BIN_MS;
            if now_ms - self.sound_since > 4000.0 && m.conf >= 1.08 {
                self.commit_with(now_ms, &m.raw_phases, cand_lag_ms, false, false);
                return;
            }
            if m.conf >= 1.25 {
                if self.pending_lag > 0.0
                    && (cand_lag_ms - self.pending_lag).abs() / cand_lag_ms < 0.04
                {
                    self.pending_count += 1;
                } else {
                    self.pending_lag = cand_lag_ms;
                    self.pending_count = 1;
                }
                if self.pending_count >= 2 {
                    self.commit_with(now_ms, &m.raw_phases, cand_lag_ms, m.bass_override, false);
                }
            } else {
                self.pending_count = 0;
            }
            return;
        }

        let x = m.raw_f * BIN_MS;
        let rel_diff = (x - self.locked_lag_ms).abs() / self.locked_lag_ms;

        if self.bass_protected
            && !m.bass_override
            && (x / self.locked_lag_ms - 1.5).abs() < 0.13
        {
            if self.bass_cadence_supports(now_ms, self.locked_lag_ms * 2.0, 2, true, None)
                || !self.bass_cadence_supports(now_ms, x * 2.0, 1, true, None)
            {
                return;
            }
            self.bass_protected = false;
        }

        if x / self.locked_lag_ms > 1.85
            && x / self.locked_lag_ms < 2.15
            && self.bass_cadence_supports(now_ms, self.locked_lag_ms, 2, false, None)
        {
            self.pending_count = 0;
            return;
        }

        if m.conf < 1.05 {
            if self.low_conf_since == 0.0 {
                self.low_conf_since = now_ms;
            }
            if now_ms - self.low_conf_since > 5000.0 {
                self.locked = false;
                self.low_conf_since = 0.0;
                self.shown_bpm = 0;
                return;
            }
        } else {
            self.low_conf_since = 0.0;
        }

        let locked_bin = (self.locked_lag_ms / BIN_MS).round() as isize;
        let mut locked_score = 0.0_f64;
        for d in -1_isize..=1 {
            let idx = locked_bin + d;
            if idx >= LAG_MIN as isize && idx <= LAG_MAX as isize {
                locked_score = locked_score.max(m.scores[idx as usize]);
            }
        }
        let cand_score = m.scores[m.raw_lag];
        let locked_idx = locked_bin.clamp(LAG_MIN as isize, LAG_MAX as isize) as usize;

        let tri_metrical = !m.percussive
            && !self.bass_protected
            && m.periodicity[m.raw_lag] >= 0.25
            && x / self.locked_lag_ms > 0.64
            && x / self.locked_lag_ms < 0.70
            && cand_score >= locked_score * 0.85
            && cand_score < locked_score * 1.2
            && m.coverage_by_lag[m.raw_lag] >= m.coverage_by_lag[locked_idx] * 1.25
            && m.periodicity[m.raw_lag] >= m.periodicity[locked_idx] * 0.8;

        let metrical = m.metrical_override || tri_metrical;
        let is_half = x / self.locked_lag_ms > 0.46 && x / self.locked_lag_ms < 0.54;
        let half_bass = is_half && self.bass_cadence_supports(now_ms, x, 3, false, Some(0.55));
        let keep_ratio = if is_half { 0.65 } else { 0.80 };

        if (!m.bass_override && !metrical && !half_bass && locked_score >= keep_ratio * cand_score)
            || rel_diff < 0.035
        {
            let target_lag = if rel_diff < 0.035 {
                x
            } else {
                self.locked_lag_ms
            };
            self.locked_lag_ms = self.locked_lag_ms * 0.92 + target_lag * 0.08;

            let win = self.window(EST_WIN);
            let lag_bins = (self.locked_lag_ms / BIN_MS).round().clamp(1.0, LAG_MAX as f64) as usize;
            let phases = self.phases_for(&win.v, &win.r, win.n, lag_bins);
            let anchor = if self.bass_protected {
                self.bass_anchor(now_ms, self.locked_lag_ms)
            } else {
                None
            };
            let target_bin = match anchor {
                Some(t) => (t / BIN_MS).round() as isize,
                None => self.pick_phase(&phases, self.locked_lag_ms, now_ms),
            };
            let phase_err = ((target_bin as f64) * BIN_MS - self.next_beat
                + self.locked_lag_ms * 1.5)
                .rem_euclid(self.locked_lag_ms)
                - self.locked_lag_ms * 0.5;
            self.next_beat += clamp_f64(phase_err, -8.0, 8.0);
            self.pending_count = 0;
            return;
        }

        if rel_diff < 0.12 && m.conf >= 1.25 && locked_score >= cand_score * 0.5 {
            self.locked_lag_ms = self.locked_lag_ms * 0.88 + x * 0.12;
            self.pending_count = 0;
            return;
        }

        let ratio_score = if locked_score > 0.0 {
            cand_score / locked_score
        } else {
            f64::INFINITY
        };
        let log_oct = (x / self.locked_lag_ms).log2();
        let is_octave = (log_oct - log_oct.round()).abs() < 0.06 && log_oct.round().abs() >= 1.0;
        let early_metrical = now_ms - self.acquired_at < 20_000.0
            && !self.initial_octave_corrected
            && (half_bass
                || (metrical && m.periodicity[m.raw_lag] >= m.periodicity[locked_idx] * 0.9));
        let allow_oct_fix =
            !self.initial_octave_corrected && (now_ms - self.acquired_at < 6500.0 || early_metrical);

        let min_ratio = if is_octave {
            if allow_oct_fix {
                if half_bass {
                    0.45
                } else if early_metrical {
                    0.75
                } else {
                    1.15
                }
            } else {
                1.8
            }
        } else if metrical || half_bass {
            0.45
        } else if is_half {
            1.15
        } else {
            1.2
        };

        let min_confirm = if is_octave {
            if allow_oct_fix {
                if half_bass {
                    2
                } else {
                    5
                }
            } else {
                10
            }
        } else if m.bass_override || half_bass {
            2
        } else if metrical {
            5
        } else if is_half {
            4
        } else {
            3
        };

        if m.conf >= 1.25 && (m.bass_override || ratio_score >= min_ratio) {
            if self.pending_lag > 0.0 && (x - self.pending_lag).abs() / x < 0.04 {
                self.pending_count += 1;
            } else {
                self.pending_lag = x;
                self.pending_count = 1;
            }
            if self.pending_count >= min_confirm {
                self.commit_with(now_ms, &m.raw_phases, x, m.bass_override, m.percussive);
            }
        } else {
            self.pending_count = 0;
        }
    }

    fn display_bpm(&mut self, now_ms: f64) -> u16 {
        if !self.locked {
            self.shown_samples.clear();
            self.shown_median = 0;
            return 0;
        }
        let bpm = (60_000.0 / self.locked_lag_ms).round().clamp(1.0, 400.0) as u16;
        let should_push = self
            .shown_samples
            .back()
            .map(|last| now_ms - last.at_ms >= 100.0)
            .unwrap_or(true);
        if should_push {
            self.shown_samples.push_back(BpmSample { at_ms: now_ms, bpm });
            if self.shown_samples.len() > 9 {
                self.shown_samples.pop_front();
            }
            let mut sorted: Vec<u16> = self.shown_samples.iter().map(|s| s.bpm).collect();
            sorted.sort_unstable();
            self.shown_median = sorted[sorted.len() / 2];
        }
        let med = self.shown_median;
        if self.shown_bpm == 0 {
            self.shown_bpm = med;
            self.shown_cand = med;
            self.shown_cand_at = now_ms;
            return med;
        }
        if (med as i32 - self.shown_bpm as i32).abs() <= 1 {
            self.shown_cand = self.shown_bpm;
            return self.shown_bpm;
        }
        if med != self.shown_cand {
            self.shown_cand = med;
            self.shown_cand_at = now_ms;
        } else if now_ms - self.shown_cand_at > 400.0 {
            self.shown_bpm = med;
        }
        self.shown_bpm
    }

    /// Processes one FFT byte-spectrum frame at monotonic timestamp `now_ms`.
    pub fn process(
        &mut self,
        now_ms: f64,
        freq: &[u8; FREQ_BINS],
        sample_rate: u32,
    ) -> BeatFrame {
        self.ensure_bands(sample_rate, freq.len());

        let gap = self.last_frame_at > 0.0 && now_ms - self.last_frame_at > 200.0;
        if gap {
            self.locked = false;
            self.shown_bpm = 0;
            self.pending_count = 0;
            self.pending_lag = 0.0;
            self.bass_onsets.clear();
            self.bass_protected = false;
            self.sound_since = 0.0;
            self.prev_e.fill(0.0);
            self.fresh_count = 0;
        }

        let dt = if self.last_frame_at > 0.0 {
            clamp_f64(now_ms - self.last_frame_at, 4.0, 100.0)
        } else {
            16.0
        };
        self.last_frame_at = now_ms;

        let mut energies = [0.0_f32; NUM_BANDS];
        self.band_energies(freq, &mut energies);

        // Sensitivity scaling normalized at baseline 1.22 (ratio == 1.0 -> exact original thresholds).
        let sens_ratio = (self.sensitivity as f64) / (DEFAULT_SENSITIVITY as f64);
        let flux_gate = clamp_f64(0.2 / sens_ratio, 0.08, 0.28);
        let flux_denom = (1.0 - flux_gate).max(0.1);
        let onset_thresh = clamp_f64(0.55 / sens_ratio.sqrt(), 0.35, 0.65);
        let bass_thresh = clamp_f64(0.58 / sens_ratio.sqrt(), 0.38, 0.68);
        let silence_thresh = clamp_f64(0.04 / sens_ratio, 0.02, 0.05);

        let mut u_sum = 0.0_f64;
        let mut m_sum = 0.0_f64;
        let mut k_sum = 0.0_f64;
        let mut loudest = 0.0_f64;
        let mut h_sum = 0.0_f64;
        let mut bass_u = 0.0_f64;
        let mut bass_w = 0.0_f64;
        let mut bass_flux_raw = 0.0_f64;

        let decay = (-dt / 3000.0).exp();
        self.spectral_frame.fill(0.0);

        for k in 0..NUM_BANDS {
            let weight = self.band_w[k];
            let r_k = energies[k] as f64;
            let flux = if gap {
                0.0
            } else {
                (r_k - self.prev_e[k] as f64).max(0.0)
            };
            self.spectral_frame[k] = flux as f32;
            self.band_peak[k] = flux.max(self.band_peak[k] * decay).max(0.008);
            let norm_flux = flux / self.band_peak[k];
            let contrib = clamp_f64((norm_flux - flux_gate) / flux_denom, 0.0, 1.0);
            u_sum += weight * contrib;
            if k < 3 {
                bass_u += weight * contrib;
                bass_w += weight;
                bass_flux_raw += flux;
            }
            h_sum += weight * flux;
            m_sum += weight;
            k_sum += r_k;
            if r_k > loudest {
                loudest = r_k;
            }
        }

        self.prev_e = energies;
        self.level = k_sum / (NUM_BANDS as f64);
        self.loudest = loudest;
        self.peak_flux = (h_sum / m_sum).max(self.peak_flux * decay).max(0.02);

        let odf_val = clamp_f64(u_sum / m_sum, 0.0, 1.0);
        let bass_val = if bass_w > 0.0 {
            clamp_f64(bass_u / bass_w, 0.0, 1.0)
        } else {
            0.0
        };
        let cur_bin = (now_ms / BIN_MS).round() as isize;

        if bass_val > bass_thresh && cur_bin - self.last_bass_onset_bin > 18 {
            self.last_bass_onset_bin = cur_bin;
            self.bass_onsets.push(BassOnset {
                time_ms: (cur_bin as f64) * BIN_MS,
                strength: bass_flux_raw,
            });
            if self.bass_onsets.len() > 30 {
                self.bass_onsets.remove(0);
            }
        }

        let onset = odf_val > onset_thresh && self.last_bin >= 0 && cur_bin - self.last_onset_bin > 12;
        if onset {
            self.last_onset_bin = cur_bin;
            self.last_strength = clamp_f64(0.45 + odf_val * 0.9, 0.45, 1.6);
        }

        self.write_odf(now_ms, odf_val as f32, bass_val as f32, gap);

        if self.loudest < silence_thresh {
            if self.silent_since == 0.0 {
                self.silent_since = now_ms;
            }
        } else {
            if self.silent_since > 0.0 && now_ms - self.silent_since > 400.0 {
                self.first_bin = self.last_bin;
                self.bass_onsets.retain(|o| now_ms - o.time_ms < 50.0);
                self.pending_count = 0;
                self.fresh_count = 0;
            }
            self.silent_since = 0.0;
        }

        if self.silent_since > 0.0 && now_ms - self.silent_since > 900.0 {
            self.odf.fill(0.0);
            self.bass_odf.fill(0.0);
            self.bass_onsets.clear();
            self.bass_protected = false;
            self.first_bin = self.last_bin + 1;
            self.locked = false;
            self.pending_count = 0;
            self.pending_lag = 0.0;
            self.low_conf_since = 0.0;
            self.shown_bpm = 0;
            self.peak_flux = 0.05;
            self.band_peak.fill(0.018);
            self.sound_since = 0.0;
            self.fresh_count = 0;
        }

        if now_ms - self.last_est_at >= EST_INTERVAL_MS && self.silent_since == 0.0 {
            self.last_est_at = now_ms;
            let mut committed_fresh = false;
            if self.locked
                && now_ms - self.committed_at > 6000.0
                && now_ms - self.last_fresh_at >= 400.0
            {
                self.last_fresh_at = now_ms;
                if let Some(fresh) = self.estimate(400) {
                    let w = fresh.raw_f * BIN_MS;
                    let locked_bin = (self.locked_lag_ms / BIN_MS).round() as isize;
                    let mut locked_score = 0.0_f64;
                    for d in -1_isize..=1 {
                        let idx = locked_bin + d;
                        if idx >= 0 && (idx as usize) < fresh.scores.len() {
                            locked_score = locked_score.max(fresh.scores[idx as usize]);
                        }
                    }
                    let bass_guard = self.bass_protected
                        && self.bass_cadence_supports(
                            now_ms,
                            self.locked_lag_ms * 2.0,
                            2,
                            true,
                            None,
                        );
                    let is_oct = ((w / self.locked_lag_ms).log2().abs() - 1.0).abs() < 0.08;
                    if !is_oct
                        && !bass_guard
                        && (w / self.locked_lag_ms - 1.0).abs() > 0.12
                        && fresh.conf >= 2.0
                        && fresh.scores[fresh.raw_lag] > locked_score * 2.0
                    {
                        self.fresh_count = if self.fresh_lag > 0.0
                            && (w / self.fresh_lag - 1.0).abs() < 0.04
                        {
                            self.fresh_count + 1
                        } else {
                            1
                        };
                        self.fresh_lag = w;
                        if self.fresh_count >= 4 {
                            self.first_bin = self.first_bin.max(self.last_bin - 400);
                            self.commit_with(
                                now_ms,
                                &fresh.raw_phases,
                                w,
                                fresh.bass_override,
                                false,
                            );
                            self.fresh_count = 0;
                            committed_fresh = true;
                        }
                    } else {
                        self.fresh_count = 0;
                    }
                } else {
                    self.fresh_count = 0;
                }
            }

            if !committed_fresh {
                let est = self.estimate(EST_WIN);
                self.update_lock(now_ms, est);
            }
        }

        let mut beat = false;
        let mut strength = 0.0_f64;
        if self.locked && self.next_beat <= now_ms {
            if now_ms - self.last_beat_at >= self.locked_lag_ms * 0.55 {
                beat = true;
                strength = clamp_f64(0.55 + self.last_strength * 0.5, 0.5, 1.7);
                self.beat_count += 1;
                self.last_beat_at = now_ms;
            }
            let steps = ((now_ms - self.next_beat) / self.locked_lag_ms).floor() + 1.0;
            self.next_beat += steps * self.locked_lag_ms;
        }

        let bpm = self.display_bpm(now_ms);
        BeatFrame {
            beat,
            onset,
            strength: strength as f32,
            level: (self.level * 1.3).min(1.0) as f32,
            bass: ((energies[0] as f64) * 1.2 + (energies[1] as f64) * 0.6).min(1.0) as f32,
            bpm,
            beats: self.beat_count,
            locked: self.locked,
            next_beat_at_ms: if self.locked {
                Some(self.next_beat)
            } else {
                None
            },
            period_ms: if self.locked {
                Some(self.locked_lag_ms)
            } else {
                None
            },
        }
    }
}
