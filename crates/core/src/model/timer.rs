//! 学习计时（功能清单第 4 节）。按时间戳计算，不依赖帧计数，后台不漂移。

use crate::clock::WallMs;
use serde::{Deserialize, Serialize};

pub const MIN_MINUTES: u32 = 1;
pub const MAX_MINUTES: u32 = 999;
pub const DEFAULT_MINUTES: u32 = 25;
const MS_PER_MINUTE: i64 = 60_000;

/// 倒计时或正计时（秒表）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerMode {
    #[default]
    Countdown,
    Stopwatch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimerStatus {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
}

/// 暂停或计时推进的结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerOutcome {
    Unchanged,
    Paused,
    /// 倒计时归零。
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudyTimer {
    mode: TimerMode,
    status: TimerStatus,
    /// 倒计时时长，1–999 分钟，会持久化。
    minutes: u32,
    /// 本轮总时长（进度条分母）。开始时按当时的分钟数确定，运行中改分钟数不影响本轮。
    round_total_ms: i64,
    /// 未运行时的显示值：倒计时为剩余毫秒，正计时为已用毫秒。
    value_ms: i64,
    /// 运行中的锚点：倒计时为结束时刻，正计时为（扣除已用时间后的）开始时刻。
    anchor: Option<WallMs>,
}

impl Default for StudyTimer {
    fn default() -> Self {
        Self::new(DEFAULT_MINUTES, TimerMode::Countdown)
    }
}

impl StudyTimer {
    #[must_use]
    pub fn new(minutes: u32, mode: TimerMode) -> Self {
        let minutes = clamp_minutes(i64::from(minutes));
        let mut timer = Self {
            mode,
            status: TimerStatus::Idle,
            minutes,
            round_total_ms: 0,
            value_ms: 0,
            anchor: None,
        };
        timer.reset();
        timer
    }

    #[must_use]
    pub fn mode(&self) -> TimerMode {
        self.mode
    }

    #[must_use]
    pub fn status(&self) -> TimerStatus {
        self.status
    }

    #[must_use]
    pub fn minutes(&self) -> u32 {
        self.minutes
    }

    /// 本轮总时长，用于进度条。
    #[must_use]
    pub fn round_total_ms(&self) -> i64 {
        self.round_total_ms
    }

    fn minutes_ms(&self) -> i64 {
        i64::from(self.minutes) * MS_PER_MINUTE
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.status == TimerStatus::Running
    }

    /// 运行或暂停时不能切换方向、不能改时长生效。
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self.status, TimerStatus::Running | TimerStatus::Paused)
    }

    /// 当前显示值：倒计时为剩余毫秒，正计时为已用毫秒。
    #[must_use]
    pub fn value_at(&self, now: WallMs) -> i64 {
        match self.anchor {
            Some(anchor) if self.is_running() => live_value(self.mode, anchor, now),
            _ => self.value_ms,
        }
    }

    /// 倒计时进行中且尚有剩余，或正计时进行中。用于"离开自习"确认。
    #[must_use]
    pub fn has_progress_at_stake(&self, now: WallMs) -> bool {
        self.is_running() && (self.mode == TimerMode::Stopwatch || self.value_at(now) > 0)
    }

    /// 设置倒计时时长，返回规整后的值是否变化（变化时需持久化）。
    pub fn set_minutes(&mut self, minutes: i64) -> bool {
        let clamped = clamp_minutes(minutes);
        let changed = clamped != self.minutes;
        self.minutes = clamped;
        if self.mode == TimerMode::Countdown
            && matches!(self.status, TimerStatus::Idle | TimerStatus::Completed)
        {
            self.reset();
        }
        changed
    }

    /// 切换方向。运行或暂停时忽略，返回是否切换成功。
    pub fn set_mode(&mut self, mode: TimerMode) -> bool {
        if self.is_active() {
            return false;
        }
        self.mode = mode;
        self.reset();
        true
    }

    /// 开始或继续。已在运行时无变化。
    pub fn start(&mut self, now: WallMs) {
        let value = match self.status {
            TimerStatus::Running => return,
            TimerStatus::Paused => self.value_ms,
            TimerStatus::Idle | TimerStatus::Completed => self.initial_value(),
        };
        self.value_ms = value;
        self.round_total_ms = self.minutes_ms();
        self.anchor = Some(match self.mode {
            TimerMode::Countdown => now.plus(value),
            TimerMode::Stopwatch => now.plus(-value),
        });
        self.status = TimerStatus::Running;
    }

    /// 推进计时。倒计时归零时转为完成。正计时不会自动结束。
    pub fn tick(&mut self, now: WallMs) -> TimerOutcome {
        let Some(anchor) = self.anchor.filter(|_| self.is_running()) else {
            return TimerOutcome::Unchanged;
        };
        self.value_ms = live_value(self.mode, anchor, now);
        if self.mode == TimerMode::Countdown && self.value_ms <= 0 {
            self.anchor = None;
            self.status = TimerStatus::Completed;
            return TimerOutcome::Completed;
        }
        TimerOutcome::Unchanged
    }

    /// 暂停并保留毫秒进度。倒计时恰好归零时转为完成。
    pub fn pause(&mut self, now: WallMs) -> TimerOutcome {
        if !self.is_running() {
            return TimerOutcome::Unchanged;
        }
        self.value_ms = self.value_at(now);
        self.anchor = None;
        if self.mode == TimerMode::Countdown && self.value_ms <= 0 {
            self.status = TimerStatus::Completed;
            TimerOutcome::Completed
        } else {
            self.status = TimerStatus::Paused;
            TimerOutcome::Paused
        }
    }

    /// 回到空闲：倒计时恢复满时长，正计时归零。
    pub fn reset(&mut self) {
        self.anchor = None;
        self.status = TimerStatus::Idle;
        self.round_total_ms = self.minutes_ms();
        self.value_ms = self.initial_value();
    }

    fn initial_value(&self) -> i64 {
        match self.mode {
            TimerMode::Countdown => self.minutes_ms(),
            TimerMode::Stopwatch => 0,
        }
    }
}

fn live_value(mode: TimerMode, anchor: WallMs, now: WallMs) -> i64 {
    let value = match mode {
        TimerMode::Countdown => anchor.since(now),
        TimerMode::Stopwatch => now.since(anchor),
    };
    value.max(0)
}

/// 规整为 1–999 的整数分钟。
#[must_use]
pub fn clamp_minutes(minutes: i64) -> u32 {
    let clamped = minutes.clamp(i64::from(MIN_MINUTES), i64::from(MAX_MINUTES));
    u32::try_from(clamped).unwrap_or(DEFAULT_MINUTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: WallMs = WallMs(1_000_000);

    #[test]
    fn countdown_runs_on_timestamps_and_completes() {
        let mut t = StudyTimer::new(1, TimerMode::Countdown);
        t.start(T0);
        assert_eq!(t.value_at(T0.plus(15_000)), 45_000);
        assert_eq!(t.tick(T0.plus(59_999)), TimerOutcome::Unchanged);
        assert_eq!(t.tick(T0.plus(60_000)), TimerOutcome::Completed);
        assert_eq!(t.status(), TimerStatus::Completed);
        assert_eq!(t.value_at(T0.plus(90_000)), 0);
    }

    #[test]
    fn pause_keeps_milliseconds_and_resume_continues() {
        let mut t = StudyTimer::new(25, TimerMode::Countdown);
        t.start(T0);
        assert_eq!(t.pause(T0.plus(1_234)), TimerOutcome::Paused);
        assert_eq!(t.value_at(T0.plus(999_999)), 25 * 60_000 - 1_234);
        t.start(T0.plus(10_000_000));
        assert_eq!(t.value_at(T0.plus(10_000_000 + 766)), 25 * 60_000 - 2_000);
    }

    #[test]
    fn stopwatch_grows_and_never_completes() {
        let mut t = StudyTimer::new(1, TimerMode::Stopwatch);
        t.start(T0);
        assert_eq!(t.tick(T0.plus(3_600_000)), TimerOutcome::Unchanged);
        assert_eq!(t.value_at(T0.plus(3_600_000)), 3_600_000);
        t.pause(T0.plus(5_000));
        t.start(T0.plus(100_000));
        assert_eq!(t.value_at(T0.plus(101_000)), 6_000);
    }

    #[test]
    fn mode_cannot_change_while_active() {
        let mut t = StudyTimer::default();
        t.start(T0);
        assert!(!t.set_mode(TimerMode::Stopwatch));
        t.pause(T0.plus(1));
        assert!(!t.set_mode(TimerMode::Stopwatch));
        t.reset();
        assert!(t.set_mode(TimerMode::Stopwatch));
        assert_eq!(t.value_at(T0), 0);
    }

    #[test]
    fn minutes_are_clamped_and_reset_completed_countdown() {
        let mut t = StudyTimer::new(1, TimerMode::Countdown);
        t.start(T0);
        t.tick(T0.plus(60_000));
        assert!(t.set_minutes(5000));
        assert_eq!(t.minutes(), 999);
        assert_eq!(t.status(), TimerStatus::Idle);
        assert_eq!(t.value_at(T0), 999 * 60_000);
        t.set_minutes(0);
        assert_eq!(t.minutes(), 1);
    }

    #[test]
    fn changing_minutes_while_running_only_affects_next_round() {
        let mut t = StudyTimer::new(10, TimerMode::Countdown);
        t.start(T0);
        t.set_minutes(30);
        assert_eq!(t.value_at(T0.plus(1_000)), 10 * 60_000 - 1_000);
        assert_eq!(t.round_total_ms(), 10 * 60_000);
        t.reset();
        assert_eq!(t.value_at(T0), 30 * 60_000);
    }
}
