//! `update`：唯一的状态修改入口（docs/架构.md 第 3 节）。
//!
//! 每个领域一个子模块。跨领域的协调（比如开始计时会切到自习）写在对应领域的处理函数里，
//! 所有处理结束后统一同步随机表情的调度。

mod audio;
mod character;
mod expression;
mod pet;
mod settings;
mod study;
mod todo;

#[cfg(test)]
mod tests;

use crate::clock::{Ctx, WallMs};
use crate::command::Command;
use crate::effect::Effect;
use crate::model::expression::{Expressions, Situation};
use crate::state::AppState;

/// 处理一条命令，返回需要外部执行的一次性动作。
pub fn update(state: &mut AppState, command: Command, ctx: &mut Ctx<'_>) -> Vec<Effect> {
    let mut fx = Vec::new();
    let now = ctx.now;
    match command {
        Command::Startup => fx.extend([
            Effect::ScanCharacters,
            Effect::LoadCharacter {
                id: state.settings.character.clone(),
            },
            Effect::ListAudioDevices,
        ]),
        Command::Tick => {
            study::tick(state, now, &mut fx);
            let (expressions, situation) = split_expressions(state);
            expressions.tick(&situation, now, ctx.rng);
        }
        Command::Study(c) => study::handle(state, c, now, &mut fx),
        Command::Expression(c) => expression::handle(state, c),
        Command::Todo(c) => todo::handle(state, c),
        Command::Audio(c) => audio::handle(state, c, now, &mut fx),
        Command::Settings(c) => settings::handle(state, c),
        Command::Pet(c) => pet::handle(state, c, &mut fx),
        Command::Character(c) => character::handle(state, c, &mut fx),
    }
    sync_random_schedule(state, now);
    fx
}

/// 同时借出表情（可变）与判断条件（只读），两者是不同字段，互不冲突。
fn split_expressions(state: &mut AppState) -> (&mut Expressions, Situation<'_>) {
    let situation = Situation {
        activity: state.activity,
        music: state.audio.is_running(),
        switches: &state.switches,
    };
    (&mut state.expressions, situation)
}

fn sync_random_schedule(state: &mut AppState, now: WallMs) {
    let enabled = state.settings.random_enabled;
    let interval = state.settings.random_interval_s;
    let (expressions, situation) = split_expressions(state);
    expressions.sync_schedule(enabled, interval, &situation, now);
}

/// 倒计时结束时，按设置决定是否响提示音。
fn chime(state: &AppState, fx: &mut Vec<Effect>) {
    if state.settings.end_sound {
        fx.push(Effect::PlayChime);
    }
}
