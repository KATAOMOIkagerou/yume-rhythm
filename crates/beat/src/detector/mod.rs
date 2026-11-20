//! 节拍检测器。基准：`class x1`（`frozen-audio-core.js`）。
//!
//! 与基准逐行对应，命名已去混淆。凡是基准用 `Float32Array` 存储的状态这里都用 `f32`
//! 存储、用 `f64` 计算（与 JS 读取 typed array 时的提升一致），其余用 `f64`；
//! 运算顺序保持原样，以便与基准逐位一致。每个函数旁注明基准标识符，便于对照。
//!
//! 数据流：每帧把 16 个对数间隔频带的正向能量变化（spectral flux）合成一个起音强度，
//! 写入 10 毫秒一格的环形缓冲（ODF）；每 200 毫秒用加权自相关、相位折叠和分频带周期性
//! 给 300–1000 毫秒的候选周期打分，再经过一系列“节拍层级”修正（4/3、2/3、×2、÷2、
//! 低音 1.5 倍）得到节奏估计；锁拍状态机据此决定锁定、微调相位或改锁。

mod estimate;
mod odf;
mod tracking;

use crate::frame::MotionFrame;
use crate::jsmath::{self, clamp};
use crate::octave::{OctaveCorrection, TempoScale, TempoShift};

/// 基准 `Lt`：ODF 每格 10 毫秒。
const BIN_MS: f64 = 10.0;
/// 基准 `Ie`：ODF 环形缓冲长度（1600 格，16 秒）。
const RING_LEN: usize = 1600;
/// 基准 `Rh`：默认分析窗口 1200 格（12 秒）。
const WINDOW_BINS: usize = 1200;
/// 基准 `Rl`：最短候选周期 30 格（300 ms，200 BPM）。
const MIN_LAG: i64 = 30;
/// 基准 `sl`：最长候选周期 100 格（1000 ms，60 BPM）。
const MAX_LAG: i64 = 100;
/// 基准 `b1`：窗口权重 `exp(-i/400)` 的时间常数（格）。
const DECAY_BINS: f64 = 400.0;
/// 打击乐模式下窗口权重改为 `exp(-i/160)`（基准 `estimate` 里的字面量）。
const PERCUSSIVE_DECAY_BINS: f64 = 160.0;
/// 基准 `S1`：两次节奏估计之间至少间隔 200 毫秒。
const ESTIMATE_INTERVAL_MS: f64 = 200.0;
/// 基准 `pl`：频带数。
const BANDS: usize = 16;
/// 基准 `bandPeak` 的初值与下限（`Float32Array.fill(.018)`）。
const BAND_PEAK_INIT: f32 = 0.018_f64 as f32;

/// 一次低音起音。基准 `bassOnsets` 的元素 `{time, strength}`。
#[derive(Debug, Clone, Copy)]
struct BassOnset {
    /// 起音所在格的时间（`bin * 10`，毫秒）。
    time: f64,
    /// 低频三个频带的 flux 之和。
    strength: f64,
}

/// 显示 BPM 的采样。基准 `shownSamples` 的元素 `{at, bpm}`。
#[derive(Debug, Clone, Copy)]
struct ShownSample {
    at: f64,
    bpm: f64,
}

/// 节拍检测器。见模块文档。
///
/// 以固定频率（应用里是 60 Hz）调用 [`BeatDetector::process`]，每次传入当前单调时钟
/// 和 [`crate::Analyser`] 输出的频谱字节。
#[derive(Debug, Clone)]
pub struct BeatDetector {
    sample_rate: f64,
    /// 基准 `freq`：上一次 `getByteFrequencyData` 写入的字节。
    freq: Vec<u8>,
    /// 基准 `odf`：起音强度环形缓冲。
    odf: Vec<f32>,
    /// 基准 `bassOdf`：低频起音强度环形缓冲。
    bass_odf: Vec<f32>,
    /// 基准 `spectralOdf`：每个频带的 flux 环形缓冲。
    spectral_odf: Vec<Vec<f32>>,
    /// 基准 `spectralFrame`：本帧各频带 flux。
    spectral_frame: [f32; BANDS],
    /// 基准 `lastBin`：最近写入的格号，-1 表示尚未写入。
    last_bin: i64,
    /// 基准 `firstBin`：分析窗口最早可用的格号。
    first_bin: i64,
    /// 基准 `prevE`：上一帧各频带能量。
    prev_energy: [f32; BANDS],
    /// 基准 `bandLo.length > 0`。
    bands_ready: bool,
    /// 基准 `bandLo` / `bandHi` / `bandW`。
    band_lo: [i64; BANDS],
    band_hi: [i64; BANDS],
    band_weight: [f64; BANDS],
    /// 基准 `peakFlux`（只写不读，保留以保持状态一致）。
    peak_flux: f64,
    /// 基准 `bandPeak`：各频带 flux 的衰减峰值。
    band_peak: [f32; BANDS],
    last_frame_at: f64,
    level: f64,
    loudest: f64,
    silent_since: f64,
    last_onset_bin: i64,
    last_bass_onset_bin: i64,
    bass_onsets: Vec<BassOnset>,
    locked: bool,
    locked_lag_ms: f64,
    bass_protected: bool,
    /// 基准 `nextBeat`。
    next_beat: f64,
    last_beat_at: f64,
    pending_lag: f64,
    pending_count: u32,
    low_conf_since: f64,
    sound_since: f64,
    last_est_at: f64,
    last_strength: f64,
    beat_count: u32,
    shown_bpm: f64,
    shown_cand: f64,
    shown_cand_at: f64,
    shown_samples: Vec<ShownSample>,
    shown_median: f64,
    fresh_lag: f64,
    fresh_count: u32,
    last_fresh_at: f64,
    committed_at: f64,
    acquired_at: f64,
    initial_octave_corrected: bool,
    /// `f32(exp(-i/400))`，基准 `window()` 每次现算，这里预先算好（值相同）。
    decay_weights: Vec<f32>,
    /// `f32(exp(-i/160))`，打击乐模式的窗口权重。
    percussive_weights: Vec<f32>,
    /// 手动八度纠正（基准引擎没有；未使用时输出不变）。见 [`crate::TempoShift`]。
    octave: OctaveCorrection,
}

impl BeatDetector {
    /// 基准：`new x1(analyser)`。`bin_count` 即 `analyser.frequencyBinCount`，
    /// `sample_rate` 即 `analyser.context.sampleRate`。
    pub fn new(sample_rate: f64, bin_count: usize) -> Self {
        let table = |tau: f64| -> Vec<f32> {
            (0..WINDOW_BINS)
                .map(|i| (-(i as f64) / tau).exp() as f32)
                .collect()
        };
        Self {
            sample_rate,
            freq: vec![0; bin_count],
            odf: vec![0.0; RING_LEN],
            bass_odf: vec![0.0; RING_LEN],
            spectral_odf: vec![vec![0.0; RING_LEN]; BANDS],
            spectral_frame: [0.0; BANDS],
            last_bin: -1,
            first_bin: -1,
            prev_energy: [0.0; BANDS],
            bands_ready: false,
            band_lo: [0; BANDS],
            band_hi: [0; BANDS],
            band_weight: [0.0; BANDS],
            peak_flux: 0.05,
            band_peak: [BAND_PEAK_INIT; BANDS],
            last_frame_at: 0.0,
            level: 0.0,
            loudest: 0.0,
            silent_since: 0.0,
            last_onset_bin: -99,
            last_bass_onset_bin: -99,
            bass_onsets: Vec::new(),
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
            shown_bpm: 0.0,
            shown_cand: 0.0,
            shown_cand_at: 0.0,
            shown_samples: Vec::new(),
            shown_median: 0.0,
            fresh_lag: 0.0,
            fresh_count: 0,
            last_fresh_at: 0.0,
            committed_at: 0.0,
            acquired_at: 0.0,
            initial_octave_corrected: false,
            decay_weights: table(DECAY_BINS),
            percussive_weights: table(PERCUSSIVE_DECAY_BINS),
            octave: OctaveCorrection::default(),
        }
    }

    /// 采样率（Hz）。
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// 频谱字节数（`frequencyBinCount`）。
    pub fn bin_count(&self) -> usize {
        self.freq.len()
    }

    /// 回到初始状态，同时取消八度纠正。基准：`x1.reset`（不清 `spectralFrame`，它每帧都会先清零）。
    pub fn reset(&mut self) {
        self.octave = OctaveCorrection::default();
        self.odf.fill(0.0);
        self.bass_odf.fill(0.0);
        for band in &mut self.spectral_odf {
            band.fill(0.0);
        }
        self.last_bin = -1;
        self.first_bin = -1;
        self.prev_energy = [0.0; BANDS];
        self.peak_flux = 0.05;
        self.band_peak = [BAND_PEAK_INIT; BANDS];
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
        self.shown_bpm = 0.0;
        self.shown_cand = 0.0;
        self.shown_cand_at = 0.0;
        self.shown_samples.clear();
        self.shown_median = 0.0;
        self.fresh_lag = 0.0;
        self.fresh_count = 0;
        self.last_fresh_at = 0.0;
        self.committed_at = 0.0;
        self.acquired_at = 0.0;
        self.initial_octave_corrected = false;
        self.bands_ready = false;
    }

    /// 处理一帧。基准：`x1.process(now)`。
    ///
    /// `now_ms` 是单调时钟（毫秒）；`freq` 是本帧的频谱字节（[`crate::Analyser`] 的输出），
    /// 写入前 `min(freq.len(), bin_count)` 个，其余保持上一帧的值（与基准的 Uint8Array 相同）。
    ///
    /// 没有调用过 [`BeatDetector::shift_tempo`] 时，输出与基准逐位相同；
    /// 否则在基准输出上做八度换算。
    pub fn process(&mut self, now_ms: f64, freq: &[u8]) -> MotionFrame {
        let frame = self.process_original(now_ms, freq);
        self.octave.apply(now_ms, frame)
    }

    /// 手动纠正八度（基准引擎没有的功能）：锁在半速时 `Double`，锁在倍速时 `Half`；
    /// 反方向再调用一次即撤销。检测器内部照常跟拍，只换算输出的
    /// `beat` / `strength` / `bpm` / `beats` / `next_beat_at` / `period_ms`。
    ///
    /// 返回 `false` 表示未生效：未锁拍、已经纠正过同一方向（不支持四倍），
    /// 或换算后的周期超出 250..=1600 ms。解锁或检测器改锁到别的周期时自动恢复原速。
    pub fn shift_tempo(&mut self, shift: TempoShift) -> bool {
        if !self.locked {
            return false;
        }
        self.octave.shift(
            shift,
            self.last_frame_at,
            self.next_beat,
            self.locked_lag_ms,
        )
    }

    /// 当前生效的八度纠正。
    pub fn tempo_scale(&self) -> TempoScale {
        self.octave.scale()
    }

    /// 与基准 `x1.process` 逐行对应。
    fn process_original(&mut self, now_ms: f64, freq: &[u8]) -> MotionFrame {
        let now = now_ms;
        let n = freq.len().min(self.freq.len());
        self.freq[..n].copy_from_slice(&freq[..n]);
        let bin_hz = self.sample_rate / 2.0 / self.freq.len() as f64;
        self.ensure_bands(bin_hz, self.freq.len());

        // 两帧间隔超过 200 ms（窗口被挂起等）：放弃锁拍与差分基准。
        let gap = self.last_frame_at > 0.0 && now - self.last_frame_at > 200.0;
        if gap {
            self.locked = false;
            self.shown_bpm = 0.0;
            self.pending_count = 0;
            self.pending_lag = 0.0;
            self.bass_onsets.clear();
            self.bass_protected = false;
            self.sound_since = 0.0;
            self.prev_energy = [0.0; BANDS];
            self.fresh_count = 0;
        }
        // 基准 `this.lastFrameAt ? ... : 16`：0 和 NaN 为假。
        let dt = if self.last_frame_at != 0.0 && !self.last_frame_at.is_nan() {
            clamp(now - self.last_frame_at, 4.0, 100.0)
        } else {
            16.0
        };
        self.last_frame_at = now;

        let mut energy = [0.0_f32; BANDS];
        self.band_energies(&mut energy);
        let mut onset_sum = 0.0; // U
        let mut weight_sum = 0.0; // M
        let mut energy_sum = 0.0; // K
        let mut loudest = 0.0; // B
        let mut flux_sum = 0.0; // H
        let mut bass_onset_sum = 0.0; // nt
        let mut bass_weight = 0.0; // et
        let mut bass_flux = 0.0; // bt
        self.spectral_frame = [0.0; BANDS];
        let decay = (-dt / 3e3).exp();
        for k in 0..BANDS {
            let weight = self.band_weight[k];
            let e = f64::from(energy[k]);
            let flux = if gap {
                0.0
            } else {
                jsmath::max(0.0, e - f64::from(self.prev_energy[k]))
            };
            self.spectral_frame[k] = flux as f32;
            let peak = jsmath::max(
                jsmath::max(flux, f64::from(self.band_peak[k]) * decay),
                0.008,
            );
            self.band_peak[k] = peak as f32;
            let ratio = flux / f64::from(self.band_peak[k]);
            let hit = clamp((ratio - 0.2) / 0.8, 0.0, 1.0);
            onset_sum += weight * hit;
            if k < 3 {
                bass_onset_sum += weight * hit;
                bass_weight += weight;
                bass_flux += flux;
            }
            flux_sum += weight * flux;
            weight_sum += weight;
            energy_sum += e;
            if e > loudest {
                loudest = e;
            }
        }
        self.prev_energy = energy;
        self.level = energy_sum / BANDS as f64;
        self.loudest = loudest;
        self.peak_flux = jsmath::max(
            jsmath::max(flux_sum / weight_sum, self.peak_flux * decay),
            0.02,
        );

        let onset_strength = clamp(onset_sum / weight_sum, 0.0, 1.0); // X
        let bass_onset = if bass_weight > 0.0 {
            clamp(bass_onset_sum / bass_weight, 0.0, 1.0)
        } else {
            0.0
        }; // rt
        let bin = jsmath::round_i64(now / BIN_MS); // Ct
        if bass_onset > 0.58 && bin - self.last_bass_onset_bin > 18 {
            self.last_bass_onset_bin = bin;
            self.bass_onsets.push(BassOnset {
                time: bin as f64 * BIN_MS,
                strength: bass_flux,
            });
            if self.bass_onsets.len() > 30 {
                self.bass_onsets.remove(0);
            }
        }
        let onset = onset_strength > 0.55 && self.last_bin >= 0 && bin - self.last_onset_bin > 12;
        if onset {
            self.last_onset_bin = bin;
            self.last_strength = clamp(0.45 + onset_strength * 0.9, 0.45, 1.6);
        }
        self.write_odf(now, onset_strength, bass_onset, gap);

        self.track_silence(now);
        if now - self.last_est_at >= ESTIMATE_INTERVAL_MS && self.silent_since == 0.0 {
            self.last_est_at = now;
            let mut committed = false;
            if self.locked && now - self.committed_at > 6e3 && now - self.last_fresh_at >= 400.0 {
                committed = self.fresh_check(now);
            }
            if !committed {
                let estimate = self.estimate(WINDOW_BINS);
                self.update_lock(now, estimate.as_ref());
            }
        }

        let mut beat = false;
        let mut strength = 0.0;
        if self.locked && self.next_beat <= now {
            if now - self.last_beat_at >= self.locked_lag_ms * 0.55 {
                beat = true;
                strength = clamp(0.55 + self.last_strength * 0.5, 0.5, 1.7);
                self.beat_count = self.beat_count.saturating_add(1);
                self.last_beat_at = now;
            }
            self.next_beat +=
                (((now - self.next_beat) / self.locked_lag_ms).floor() + 1.0) * self.locked_lag_ms;
        }
        MotionFrame {
            beat,
            onset,
            strength,
            level: jsmath::min(1.0, self.level * 1.3),
            bass: jsmath::min(1.0, f64::from(energy[0]) * 1.2 + f64::from(energy[1]) * 0.6),
            bpm: self.display_bpm(now),
            beats: self.beat_count,
            locked: self.locked,
            next_beat_at: self.locked.then_some(self.next_beat),
            period_ms: self.locked.then_some(self.locked_lag_ms),
        }
    }

    /// 静音判定（`process` 中间那段 `loudest < .04` 的逻辑）。
    ///
    /// 静音超过 400 ms 后恢复：截断分析窗口；静音超过 900 ms：清空 ODF 并解锁。
    fn track_silence(&mut self, now: f64) {
        if self.loudest < 0.04 {
            if self.silent_since == 0.0 {
                self.silent_since = now;
            }
        } else {
            if self.silent_since > 0.0 && now - self.silent_since > 400.0 {
                self.first_bin = self.last_bin;
                self.bass_onsets.retain(|o| now - o.time < 50.0);
                self.pending_count = 0;
                self.fresh_count = 0;
            }
            self.silent_since = 0.0;
        }
        if self.silent_since > 0.0 && now - self.silent_since > 900.0 {
            self.odf.fill(0.0);
            self.bass_odf.fill(0.0);
            self.bass_onsets.clear();
            self.bass_protected = false;
            self.first_bin = self.last_bin + 1;
            self.locked = false;
            self.pending_count = 0;
            self.pending_lag = 0.0;
            self.low_conf_since = 0.0;
            self.shown_bpm = 0.0;
            self.peak_flux = 0.05;
            self.band_peak = [BAND_PEAK_INIT; BANDS];
            self.sound_since = 0.0;
            self.fresh_count = 0;
        }
    }
}
