//! 采样环形缓冲：音频线程写入单声道采样，节拍线程读取最近 N 个采样。
//!
//! 无锁、写入时不分配内存。读取方只要最新的一段窗口，容量远大于窗口长度，
//! 所以读取过程中不会被写入方覆盖。
//!
//! 系统回环在没有声音时不送数据。读取方根据"距上次写入过了多久"在窗口末尾补零，
//! 效果等同于浏览器 AnalyserNode 在静音时看到的全零输入。

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

pub struct SampleRing {
    buf: Box<[AtomicU32]>,
    /// 已写入的采样总数（单调递增）。
    written: AtomicUsize,
    sample_rate: AtomicU32,
    /// 每次换音源递增，读取方据此重置分析器。
    generation: AtomicU32,
    /// 最近一次写入的时刻（相对 `epoch` 的微秒）。
    last_write_us: AtomicU64,
    epoch: Instant,
}

impl SampleRing {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let buf = (0..capacity.max(1)).map(|_| AtomicU32::new(0)).collect();
        Self {
            buf,
            written: AtomicUsize::new(0),
            sample_rate: AtomicU32::new(48_000),
            generation: AtomicU32::new(0),
            last_write_us: AtomicU64::new(0),
            epoch: Instant::now(),
        }
    }

    /// 开始新的音源：设定采样率，清空历史。
    pub fn begin(&self, sample_rate: u32) {
        self.sample_rate
            .store(sample_rate.max(1), Ordering::Relaxed);
        self.written.store(0, Ordering::Relaxed);
        self.touch();
        self.generation.fetch_add(1, Ordering::Release);
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// 本音源开始以来写入的采样总数（诊断用）。
    #[must_use]
    pub fn written(&self) -> usize {
        self.written.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// 写入单声道采样（音频线程调用）。
    pub fn push(&self, samples: &[f32]) {
        let start = self.written.load(Ordering::Relaxed);
        let cap = self.buf.len();
        for (i, s) in samples.iter().enumerate() {
            self.buf[(start + i) % cap].store(s.to_bits(), Ordering::Relaxed);
        }
        self.written.store(start + samples.len(), Ordering::Release);
        self.touch();
    }

    /// 写入交错的多声道采样，按声道取平均。
    pub fn push_interleaved(&self, samples: &[f32], channels: usize) {
        let channels = channels.max(1);
        let start = self.written.load(Ordering::Relaxed);
        let cap = self.buf.len();
        #[allow(clippy::cast_precision_loss, reason = "声道数很小")]
        let norm = 1.0 / channels as f32;
        let mut n = 0;
        for frame in samples.chunks_exact(channels) {
            let mono = frame.iter().sum::<f32>() * norm;
            self.buf[(start + n) % cap].store(mono.to_bits(), Ordering::Relaxed);
            n += 1;
        }
        self.written.store(start + n, Ordering::Release);
        self.touch();
    }

    /// 取最近 `out.len()` 个采样，不足处补零；距上次写入超过一小段时间时，把这段时间按静音计。
    pub fn latest(&self, out: &mut [f32]) {
        let written = self.written.load(Ordering::Acquire);
        let silence = self.silence_samples().min(out.len());
        let take = (out.len() - silence).min(written).min(self.buf.len());
        let pad = out.len() - silence - take;
        out[..pad].fill(0.0);
        let cap = self.buf.len();
        for (i, slot) in out[pad..pad + take].iter_mut().enumerate() {
            let index = (written - take + i) % cap;
            *slot = f32::from_bits(self.buf[index].load(Ordering::Relaxed));
        }
        out[pad + take..].fill(0.0);
    }

    fn touch(&self) {
        let us = u64::try_from(self.epoch.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.last_write_us.store(us, Ordering::Relaxed);
    }

    /// 回环静音时没有回调。超过 40 ms 没有新数据，就把空档按静音补进窗口。
    fn silence_samples(&self) -> usize {
        const GRACE_US: u64 = 40_000;
        let now = u64::try_from(self.epoch.elapsed().as_micros()).unwrap_or(u64::MAX);
        let gap = now.saturating_sub(self.last_write_us.load(Ordering::Relaxed));
        if gap <= GRACE_US {
            return 0;
        }
        let samples = u128::from(gap) * u128::from(self.sample_rate()) / 1_000_000;
        usize::try_from(samples).unwrap_or(usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_window_is_most_recent_and_zero_padded() {
        let ring = SampleRing::new(8);
        ring.begin(48_000);
        ring.push(&[1.0, 2.0, 3.0]);
        let mut out = [9.0; 5];
        ring.latest(&mut out);
        assert_eq!(out, [0.0, 0.0, 1.0, 2.0, 3.0]);
        ring.push(&[4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        let mut out = [0.0; 4];
        ring.latest(&mut out);
        assert_eq!(out, [7.0, 8.0, 9.0, 10.0]);
    }

    #[test]
    fn interleaved_is_downmixed() {
        let ring = SampleRing::new(8);
        ring.begin(48_000);
        ring.push_interleaved(&[1.0, 3.0, -1.0, 1.0], 2);
        let mut out = [0.0; 2];
        ring.latest(&mut out);
        assert_eq!(out, [2.0, 0.0]);
    }

    #[test]
    fn begin_bumps_generation_and_clears() {
        let ring = SampleRing::new(8);
        let g = ring.generation();
        ring.push(&[1.0]);
        ring.begin(44_100);
        assert_eq!(ring.generation(), g + 1);
        assert_eq!(ring.sample_rate(), 44_100);
        let mut out = [5.0; 2];
        ring.latest(&mut out);
        assert_eq!(out, [0.0, 0.0]);
    }
}
