//! 2048-point Blackman-windowed FFT matching Web Audio `AnalyserNode` semantics.
//!
//! Parameters match the original engine:
//! - `fftSize = 2048` (`frequencyBinCount = 1024`)
//! - `smoothingTimeConstant = 0.08`
//! - `minDecibels = -90.0`
//! - `maxDecibels = -3.0`

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::f32::consts::PI;
use std::sync::Arc;

pub const FFT_SIZE: usize = 2048;
pub const FREQ_BINS: usize = FFT_SIZE / 2;
pub const SMOOTHING_TIME_CONSTANT: f32 = 0.08;
pub const MIN_DECIBELS: f32 = -90.0;
pub const MAX_DECIBELS: f32 = -3.0;

/// Pure-Rust equivalent of Web Audio API `AnalyserNode::getByteFrequencyData`.
pub struct AnalyserFft {
    sample_rate: u32,
    fft: Arc<dyn Fft<f32>>,
    window: [f32; FFT_SIZE],
    time_ring: [f32; FFT_SIZE],
    write_pos: usize,
    fft_buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    smoothed: [f32; FREQ_BINS],
}

impl AnalyserFft {
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let scratch_len = fft.get_inplace_scratch_len();

        // W3C Web Audio Blackman window: alpha = 0.16 -> a0 = 0.42, a1 = 0.5, a2 = 0.08.
        let mut window = [0.0_f32; FFT_SIZE];
        let n_f = FFT_SIZE as f32;
        for (i, w) in window.iter_mut().enumerate() {
            let phase = 2.0 * PI * (i as f32) / n_f;
            *w = 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
        }

        Self {
            sample_rate: sample_rate.max(8_000),
            fft,
            window,
            time_ring: [0.0; FFT_SIZE],
            write_pos: 0,
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            smoothed: [0.0; FREQ_BINS],
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(8_000);
    }

    pub fn reset(&mut self) {
        self.time_ring.fill(0.0);
        self.write_pos = 0;
        self.smoothed.fill(0.0);
    }

    /// Pushes mono `f32` PCM samples into the 2048-sample sliding window.
    #[inline]
    pub fn push_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        if samples.len() >= FFT_SIZE {
            let tail = &samples[samples.len() - FFT_SIZE..];
            self.time_ring.copy_from_slice(tail);
            self.write_pos = 0;
            return;
        }
        let first = (FFT_SIZE - self.write_pos).min(samples.len());
        self.time_ring[self.write_pos..self.write_pos + first].copy_from_slice(&samples[..first]);
        let rem = samples.len() - first;
        if rem > 0 {
            self.time_ring[..rem].copy_from_slice(&samples[first..]);
            self.write_pos = rem;
        } else {
            self.write_pos = (self.write_pos + first) % FFT_SIZE;
        }
    }

    /// Pushes `count` zero samples (silence) into the sliding window.
    #[inline]
    pub fn push_silence(&mut self, count: usize) {
        if count >= FFT_SIZE {
            self.time_ring.fill(0.0);
            self.write_pos = 0;
            return;
        }
        let first = (FFT_SIZE - self.write_pos).min(count);
        self.time_ring[self.write_pos..self.write_pos + first].fill(0.0);
        let rem = count - first;
        if rem > 0 {
            self.time_ring[..rem].fill(0.0);
            self.write_pos = rem;
        } else {
            self.write_pos = (self.write_pos + first) % FFT_SIZE;
        }
    }

    /// Computes Web Audio `AnalyserNode`-compatible byte frequency data (`0..=255`).
    pub fn get_byte_frequency_data(&mut self, out: &mut [u8; FREQ_BINS]) {
        // Read samples in chronological order starting at write_pos (oldest sample).
        let split = self.write_pos;
        let first_len = FFT_SIZE - split;
        for i in 0..first_len {
            let s = self.time_ring[split + i];
            let safe = if s.is_finite() { s } else { 0.0 };
            self.fft_buf[i] = Complex::new(safe * self.window[i], 0.0);
        }
        for i in 0..split {
            let s = self.time_ring[i];
            let safe = if s.is_finite() { s } else { 0.0 };
            let idx = first_len + i;
            self.fft_buf[idx] = Complex::new(safe * self.window[idx], 0.0);
        }

        self.fft
            .process_with_scratch(&mut self.fft_buf, &mut self.scratch);

        let inv_n = 1.0 / (FFT_SIZE as f32);
        let inv_db_range = 255.0 / (MAX_DECIBELS - MIN_DECIBELS);
        let one_minus_smooth = 1.0 - SMOOTHING_TIME_CONSTANT;

        for k in 0..FREQ_BINS {
            let c = self.fft_buf[k];
            let mag = (c.re * c.re + c.im * c.im).sqrt() * inv_n;
            let sm = SMOOTHING_TIME_CONSTANT * self.smoothed[k] + one_minus_smooth * mag;
            self.smoothed[k] = if sm.is_finite() { sm } else { 0.0 };

            let byte = if self.smoothed[k] <= 1e-20 {
                0u8
            } else {
                let db = 20.0 * self.smoothed[k].log10();
                let scaled = (db - MIN_DECIBELS) * inv_db_range;
                scaled.clamp(0.0, 255.0) as u8
            };
            out[k] = byte;
        }
    }
}
