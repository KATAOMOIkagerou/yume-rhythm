//! 频谱分析器：Web Audio `AnalyserNode.getByteFrequencyData` 的等价实现。
//!
//! 按 Web Audio 规范“计算频率数据”一节逐步实现（全部 f64）：
//! 1. 取最近 `fft_size` 个采样，乘 Blackman 窗（α = 0.16）；
//! 2. FFT，幅度 `|X[k]| / fft_size`；
//! 3. 与上一次调用的结果做时间平滑 `S = τ·S_prev + (1−τ)·|X|`，非有限值置 0；
//! 4. `dB = 20·log10(S)`，字节 `floor(255/(maxDb−minDb)·(dB−minDb))`，截断到 0..=255。
//!
//! 基准引擎的宿主（`E1.ensureGraph`）用的参数是 fftSize 2048、平滑 0.08、-90..-3 dB，
//! 即 [`AnalyserConfig::default`]。宿主每帧调用一次，平滑每次调用都会推进，
//! 这里同样是每调用一次推进一次。
//!
//! 与 Chrome 的差别：Chrome 内部用 f32 做 FFT 和平滑存储，这里用 f64；量化到字节后
//! 两者几乎总是相同。基准工具 `tools/beat-baseline` 里的假 AnalyserNode 与本实现逐行相同。

use std::fmt;

use crate::fft::{UnitRoots, fft_in_place};

const BLACKMAN_ALPHA: f64 = 0.16;
const MIN_FFT_SIZE: usize = 32;
const MAX_FFT_SIZE: usize = 32768;

/// 分析器参数，对应 `AnalyserNode` 的同名属性。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalyserConfig {
    /// `fftSize`：32..=32768 之间的 2 的幂。
    pub fft_size: usize,
    /// `smoothingTimeConstant`：0..=1。
    pub smoothing: f64,
    /// `minDecibels`。
    pub min_db: f64,
    /// `maxDecibels`，必须大于 `min_db`。
    pub max_db: f64,
}

impl Default for AnalyserConfig {
    /// 基准引擎宿主的设置：fftSize 2048、平滑 0.08、-90..-3 dB。
    fn default() -> Self {
        Self {
            fft_size: 2048,
            smoothing: 0.08,
            min_db: -90.0,
            max_db: -3.0,
        }
    }
}

/// 参数非法（与 `AnalyserNode` 抛异常的条件相同）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalyserError {
    /// fftSize 不是 32..=32768 之间的 2 的幂。
    FftSize(usize),
    /// 平滑系数不在 0..=1。
    Smoothing(f64),
    /// `min_db >= max_db` 或不是有限值。
    DecibelRange { min_db: f64, max_db: f64 },
}

impl fmt::Display for AnalyserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FftSize(n) => write!(f, "fft size {n} is not a power of two in 32..=32768"),
            Self::Smoothing(s) => write!(f, "smoothing time constant {s} is not in 0..=1"),
            Self::DecibelRange { min_db, max_db } => {
                write!(f, "decibel range {min_db}..{max_db} is empty or not finite")
            }
        }
    }
}

impl std::error::Error for AnalyserError {}

/// 频谱分析器。见模块文档。
#[derive(Debug, Clone)]
pub struct Analyser {
    config: AnalyserConfig,
    roots: UnitRoots,
    window: Vec<f64>,
    smoothed: Vec<f64>,
    re: Vec<f64>,
    im: Vec<f64>,
}

impl Default for Analyser {
    fn default() -> Self {
        Self::build(AnalyserConfig::default())
    }
}

impl Analyser {
    /// 按给定参数创建；参数非法时返回错误。
    pub fn new(config: AnalyserConfig) -> Result<Self, AnalyserError> {
        let n = config.fft_size;
        if !n.is_power_of_two() || !(MIN_FFT_SIZE..=MAX_FFT_SIZE).contains(&n) {
            return Err(AnalyserError::FftSize(n));
        }
        if !(0.0..=1.0).contains(&config.smoothing) {
            return Err(AnalyserError::Smoothing(config.smoothing));
        }
        if !(config.min_db.is_finite()
            && config.max_db.is_finite()
            && config.min_db < config.max_db)
        {
            return Err(AnalyserError::DecibelRange {
                min_db: config.min_db,
                max_db: config.max_db,
            });
        }
        Ok(Self::build(config))
    }

    fn build(config: AnalyserConfig) -> Self {
        let n = config.fft_size;
        let roots = UnitRoots::new(n);
        let a0 = 0.5 * (1.0 - BLACKMAN_ALPHA);
        let a1 = 0.5;
        let a2 = 0.5 * BLACKMAN_ALPHA;
        let window = (0..n)
            .map(|i| a0 - a1 * roots.cos[i] + a2 * roots.cos[(2 * i) % n])
            .collect();
        Self {
            config,
            roots,
            window,
            smoothed: vec![0.0; n / 2],
            re: vec![0.0; n],
            im: vec![0.0; n],
        }
    }

    /// 当前参数。
    pub fn config(&self) -> AnalyserConfig {
        self.config
    }

    /// `fftSize`。
    pub fn fft_size(&self) -> usize {
        self.config.fft_size
    }

    /// `frequencyBinCount`，即 `fft_size / 2`。
    pub fn frequency_bin_count(&self) -> usize {
        self.config.fft_size / 2
    }

    /// 清空平滑状态（相当于新建一个 AnalyserNode）。
    pub fn reset(&mut self) {
        self.smoothed.fill(0.0);
    }

    /// `getByteFrequencyData`。
    ///
    /// `samples` 是最近的单声道采样（时间顺序，最后一个最新）：多于 `fft_size` 时只用
    /// 最后 `fft_size` 个，不足时前面按静音补零。`out` 写入
    /// `min(out.len(), frequency_bin_count)` 个字节，其余元素不动（与 Web Audio 相同）。
    pub fn get_byte_frequency_data(&mut self, samples: &[f32], out: &mut [u8]) {
        self.analyse(samples);
        let byte_scale = 255.0 / (self.config.max_db - self.config.min_db);
        for (dst, &s) in out.iter_mut().zip(&self.smoothed) {
            let db = 20.0 * s.log10();
            let b = (byte_scale * (db - self.config.min_db)).floor();
            // log10(0) = -∞ 得到 -∞，按规范截断为 0。
            *dst = if b.is_nan() || b < 0.0 {
                0
            } else if b > 255.0 {
                255
            } else {
                b as u8
            };
        }
    }

    /// 加窗、FFT、取幅度并平滑，结果留在 `self.smoothed`。
    fn analyse(&mut self, samples: &[f32]) {
        let n = self.config.fft_size;
        let take = samples.len().min(n);
        let pad = n - take;
        let recent = &samples[samples.len() - take..];
        for i in 0..n {
            let x = if i < pad {
                0.0
            } else {
                f64::from(recent[i - pad])
            };
            self.re[i] = x * self.window[i];
            self.im[i] = 0.0;
        }
        fft_in_place(&mut self.re, &mut self.im, &self.roots);
        let scale = 1.0 / n as f64;
        let tau = self.config.smoothing;
        for (k, s) in self.smoothed.iter_mut().enumerate() {
            let (re, im) = (self.re[k], self.im[k]);
            let mag = (re * re + im * im).sqrt() * scale;
            let v = tau * *s + (1.0 - tau) * mag;
            *s = if v.is_finite() { v } else { 0.0 };
        }
    }
}
