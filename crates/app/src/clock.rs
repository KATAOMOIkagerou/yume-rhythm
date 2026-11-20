//! 两个时钟（docs/架构.md 第 6 节）：墙钟给学习计时，单调时钟给动画与节拍。

use std::time::{Instant, SystemTime, UNIX_EPOCH};
use yume_core::WallMs;

/// 当前墙钟时间（Unix 毫秒）。
#[must_use]
pub fn wall_now() -> WallMs {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    WallMs(ms)
}

/// 从程序启动算起的单调毫秒。可复制，各线程共用同一个起点。
#[derive(Clone, Copy, Debug)]
pub struct Mono {
    epoch: Instant,
}

impl Mono {
    #[must_use]
    pub fn start() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }

    #[must_use]
    pub fn now_ms(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64() * 1000.0
    }
}

/// 帧节拍器：只有到点才出帧。
///
/// 等待时会被窗口消息提前唤醒（拖动时每秒上千次），提前醒来不算一帧、也不推迟下一帧；
/// 出帧后下一帧最多排在"现在 + 一帧"，落后太多就从现在重新算，不补帧。
#[derive(Clone, Copy, Debug)]
pub struct FramePacer {
    next: f64,
}

impl FramePacer {
    #[must_use]
    pub fn new(now: f64) -> Self {
        Self { next: now }
    }

    #[must_use]
    pub fn due(&self, now: f64) -> bool {
        now >= self.next
    }

    /// 出完一帧后调用。
    pub fn advance(&mut self, now: f64, interval: f64) {
        self.next = if self.next + interval <= now {
            now + interval
        } else {
            self.next + interval
        };
    }

    /// 距下一帧还要等多久。帧率刚调高时不会等得比新间隔更久。
    #[must_use]
    pub fn wait_ms(&mut self, now: f64, interval: f64) -> f64 {
        self.next = self.next.min(now + interval);
        (self.next - now).max(0.0)
    }
}

/// 随机数种子：取启动时刻的纳秒。
#[must_use]
pub fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0x5eed, |d| d.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::FramePacer;

    #[test]
    fn early_wakeups_do_not_push_frames_back() {
        let mut pacer = FramePacer::new(0.0);
        assert!(pacer.due(0.0));
        pacer.advance(0.0, 16.0);
        // 拖动时被消息唤醒很多次：都不到点，也不改变下一帧时间。
        for i in 0..1000 {
            assert!(!pacer.due(f64::from(i) * 0.01));
        }
        assert!(pacer.due(16.0));
        assert!((pacer.wait_ms(10.0, 16.0) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn falling_behind_restarts_from_now() {
        let mut pacer = FramePacer::new(0.0);
        pacer.advance(500.0, 33.0);
        assert!((pacer.wait_ms(500.0, 33.0) - 33.0).abs() < 1e-9);
    }

    #[test]
    fn faster_rate_takes_effect_immediately() {
        let mut pacer = FramePacer::new(0.0);
        pacer.advance(0.0, 100.0);
        assert!((pacer.wait_ms(1.0, 16.0) - 16.0).abs() < 1e-9);
    }
}
