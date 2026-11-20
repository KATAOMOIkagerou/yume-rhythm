//! 单生产者单消费者的采样队列：解码线程写入，播放回调读取。无锁、不分配。

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct PcmQueue {
    buf: Box<[AtomicU32]>,
    /// 已写入总数（只由生产者修改）。
    head: AtomicUsize,
    /// 已读取总数（只由消费者修改）。
    tail: AtomicUsize,
}

impl PcmQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let buf = (0..capacity.max(1)).map(|_| AtomicU32::new(0)).collect();
        Self {
            buf,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// 还能写入多少个采样。
    #[must_use]
    pub fn free(&self) -> usize {
        let used = self.head.load(Ordering::Acquire) - self.tail.load(Ordering::Acquire);
        self.buf.len() - used
    }

    /// 写入尽可能多的采样，返回实际写入数（生产者调用）。
    pub fn push(&self, samples: &[f32]) -> usize {
        let n = samples.len().min(self.free());
        let head = self.head.load(Ordering::Relaxed);
        let cap = self.buf.len();
        for (i, s) in samples[..n].iter().enumerate() {
            self.buf[(head + i) % cap].store(s.to_bits(), Ordering::Relaxed);
        }
        self.head.store(head + n, Ordering::Release);
        n
    }

    /// 读出尽可能多的采样，返回实际读出数（消费者调用）。
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let available = self.head.load(Ordering::Acquire) - tail;
        let n = out.len().min(available);
        let cap = self.buf.len();
        for (i, slot) in out[..n].iter_mut().enumerate() {
            *slot = f32::from_bits(self.buf[(tail + i) % cap].load(Ordering::Relaxed));
        }
        self.tail.store(tail + n, Ordering::Release);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_with_wraparound() {
        let q = PcmQueue::new(4);
        assert_eq!(q.push(&[1.0, 2.0, 3.0, 4.0, 5.0]), 4);
        let mut out = [0.0; 3];
        assert_eq!(q.pop(&mut out), 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
        assert_eq!(q.push(&[6.0, 7.0]), 2);
        let mut out = [0.0; 5];
        assert_eq!(q.pop(&mut out), 3);
        assert_eq!(&out[..3], &[4.0, 6.0, 7.0]);
    }
}
