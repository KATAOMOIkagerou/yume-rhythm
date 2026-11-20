//! 学习计时、活动模式与确认框（功能清单第 4、5 节）。

use super::chime;
use crate::clock::WallMs;
use crate::command::StudyCommand;
use crate::effect::Effect;
use crate::model::events::ActivityMode;
use crate::model::pet::Dialog;
use crate::model::timer::{TimerMode, TimerOutcome};
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: StudyCommand, now: WallMs, fx: &mut Vec<Effect>) {
    match cmd {
        StudyCommand::SelectActivity(mode) => select_activity(state, mode, now),
        StudyCommand::SetTimerMode(mode) => {
            state.timer.set_mode(mode);
        }
        StudyCommand::SetMinutes(minutes) => {
            state.timer.set_minutes(minutes);
        }
        StudyCommand::Start => {
            if !state.timer.is_running() {
                state.timer.start(now);
                // 开始计时强制切到自习。
                state.activity = ActivityMode::Study;
            }
        }
        StudyCommand::RequestPause => request_pause(state, now, fx),
        StudyCommand::Reset => state.timer.reset(),
        StudyCommand::ConfirmDialog => confirm(state, now, fx),
        StudyCommand::CancelDialog => cancel(state),
    }
}

pub(super) fn tick(state: &mut AppState, now: WallMs, fx: &mut Vec<Effect>) {
    if state.timer.tick(now) == TimerOutcome::Completed {
        state.dialog = Dialog::None;
        chime(state, fx);
    }
}

/// 计时中从自习切到休闲需要确认，其余情况直接切换。
fn select_activity(state: &mut AppState, mode: ActivityMode, now: WallMs) {
    if mode == state.activity {
        return;
    }
    let leaving_study = state.activity == ActivityMode::Study && mode == ActivityMode::Chill;
    if leaving_study && state.timer.has_progress_at_stake(now) {
        state.dialog = Dialog::ConfirmLeaveStudy;
        return;
    }
    state.activity = mode;
}

/// 倒计时暂停需要确认；正计时直接暂停。
fn request_pause(state: &mut AppState, now: WallMs, fx: &mut Vec<Effect>) {
    if !state.timer.is_running() {
        return;
    }
    match state.timer.mode() {
        TimerMode::Stopwatch => pause(state, now, false, fx),
        TimerMode::Countdown => state.dialog = Dialog::ConfirmPause,
    }
}

fn confirm(state: &mut AppState, now: WallMs, fx: &mut Vec<Effect>) {
    match state.dialog {
        Dialog::ConfirmPause => pause(state, now, false, fx),
        Dialog::ConfirmLeaveStudy => pause(state, now, true, fx),
        Dialog::Encourage | Dialog::None => state.dialog = Dialog::None,
    }
}

/// 拒绝暂停或离开时给一句鼓励；关闭鼓励框即结束。
fn cancel(state: &mut AppState) {
    state.dialog = match state.dialog {
        Dialog::ConfirmPause | Dialog::ConfirmLeaveStudy => Dialog::Encourage,
        Dialog::Encourage | Dialog::None => Dialog::None,
    };
}

fn pause(state: &mut AppState, now: WallMs, leave_study: bool, fx: &mut Vec<Effect>) {
    if state.timer.pause(now) == TimerOutcome::Completed {
        chime(state, fx);
    }
    if leave_study {
        state.activity = ActivityMode::Chill;
    }
    state.dialog = Dialog::None;
}
