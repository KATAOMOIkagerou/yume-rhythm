use crate::settings::{MAX_STUDY_MINUTES, MIN_STUDY_MINUTES};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerMode {
    #[default]
    Countdown,
    Stopwatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerState {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
}

/// Drift-free wall-clock timer calculation matching `src/state/timer.ts`.
pub fn timer_value(mode: TimerMode, anchor_ms: i64, now_ms: i64) -> u64 {
    let diff = match mode {
        TimerMode::Countdown => anchor_ms.saturating_sub(now_ms),
        TimerMode::Stopwatch => now_ms.saturating_sub(anchor_ms),
    };
    diff.max(0) as u64
}

#[derive(Debug, Clone)]
pub struct StudyTimer {
    pub mode: TimerMode,
    pub state: TimerState,
    pub study_minutes: u32,
    pub total_ms: u64,
    pub remaining_ms: u64,
    anchor_ms: Option<i64>,
}

impl StudyTimer {
    pub fn new(study_minutes: u32) -> Self {
        let clamped = study_minutes.clamp(MIN_STUDY_MINUTES, MAX_STUDY_MINUTES);
        let ms = u64::from(clamped) * 60_000;
        Self {
            mode: TimerMode::Countdown,
            state: TimerState::Idle,
            study_minutes: clamped,
            total_ms: ms,
            remaining_ms: ms,
            anchor_ms: None,
        }
    }

    pub fn set_mode(&mut self, mode: TimerMode) -> bool {
        if matches!(self.state, TimerState::Running | TimerState::Paused) {
            return false;
        }
        self.anchor_ms = None;
        self.mode = mode;
        self.state = TimerState::Idle;
        self.remaining_ms = match mode {
            TimerMode::Countdown => u64::from(self.study_minutes) * 60_000,
            TimerMode::Stopwatch => 0,
        };
        true
    }

    pub fn set_study_minutes(&mut self, minutes: u32) -> u32 {
        let clamped = minutes.clamp(MIN_STUDY_MINUTES, MAX_STUDY_MINUTES);
        self.study_minutes = clamped;
        if self.mode == TimerMode::Countdown
            && matches!(self.state, TimerState::Idle | TimerState::Completed)
        {
            let ms = u64::from(clamped) * 60_000;
            self.total_ms = ms;
            self.remaining_ms = ms;
            if self.state == TimerState::Completed {
                self.state = TimerState::Idle;
            }
        }
        clamped
    }

    pub fn start_or_resume(&mut self, now_ms: i64) {
        if self.state == TimerState::Running {
            return;
        }
        let duration = u64::from(self.study_minutes) * 60_000;
        let value = match self.state {
            TimerState::Paused => self.remaining_ms,
            _ => match self.mode {
                TimerMode::Countdown => duration,
                TimerMode::Stopwatch => 0,
            },
        };
        self.total_ms = duration;
        self.remaining_ms = value;
        self.anchor_ms = Some(match self.mode {
            TimerMode::Countdown => now_ms.saturating_add(value as i64),
            TimerMode::Stopwatch => now_ms.saturating_sub(value as i64),
        });
        self.state = TimerState::Running;
    }

    /// Settles elapsed/remaining ms and transitions to Paused (or Completed if countdown reached 0).
    /// Returns `true` if countdown just completed.
    pub fn pause(&mut self, now_ms: i64) -> bool {
        let value = match self.anchor_ms.take() {
            Some(anchor) => timer_value(self.mode, anchor, now_ms),
            None => self.remaining_ms,
        };
        self.remaining_ms = value;
        if self.mode == TimerMode::Countdown && value == 0 {
            self.state = TimerState::Completed;
            true
        } else {
            self.state = TimerState::Paused;
            false
        }
    }

    pub fn reset(&mut self) {
        self.anchor_ms = None;
        let ms = u64::from(self.study_minutes.clamp(MIN_STUDY_MINUTES, MAX_STUDY_MINUTES)) * 60_000;
        self.total_ms = ms;
        self.remaining_ms = match self.mode {
            TimerMode::Countdown => ms,
            TimerMode::Stopwatch => 0,
        };
        self.state = TimerState::Idle;
    }

    /// Advances running timer against wall clock. Returns `true` on the exact tick countdown completes.
    pub fn tick(&mut self, now_ms: i64) -> bool {
        if self.state != TimerState::Running {
            return false;
        }
        let Some(anchor) = self.anchor_ms else {
            return false;
        };
        let value = timer_value(self.mode, anchor, now_ms);
        self.remaining_ms = value;
        if self.mode == TimerMode::Countdown && value == 0 {
            self.anchor_ms = None;
            self.state = TimerState::Completed;
            return true;
        }
        false
    }

    pub fn progress_ratio(&self) -> f32 {
        if self.total_ms == 0 {
            1.0
        } else {
            (self.remaining_ms as f32 / self.total_ms as f32).clamp(0.0, 1.0)
        }
    }

    pub fn format_display(&self) -> String {
        let display_ms = match self.mode {
            TimerMode::Stopwatch => (self.remaining_ms / 1000) * 1000,
            TimerMode::Countdown => self.remaining_ms,
        };
        format_mm_ss(display_ms)
    }
}

pub fn format_mm_ss(ms: u64) -> String {
    let total_sec = (ms + 999) / 1000;
    let mins = total_sec / 60;
    let secs = total_sec % 60;
    format!("{mins:02}:{secs:02}")
}
