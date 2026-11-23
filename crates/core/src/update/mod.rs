//! `update`：唯一的状态修改入口（docs/架构.md 第 3 节）。
//!
//! 每个领域一个子模块。跨领域的协调（比如开始计时会切到自习）写在对应领域的处理函数里，
//! 所有处理结束后统一同步随机表情的调度。

mod audio;
mod character;
mod editor;
mod expression;
mod pet;
mod settings;
mod study;
mod todo;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_editor;

use crate::clock::{Ctx, WallMs};
use crate::command::Command;
use crate::effect::Effect;
use crate::model::expr_rules::resolve_catalog;
use crate::model::expression::{Expressions, Situation};
use crate::state::AppState;

/// 处理一条命令，返回需要外部执行的一次性动作。
pub fn update(state: &mut AppState, command: Command, ctx: &mut Ctx<'_>) -> Vec<Effect> {
    let mut fx = Vec::new();
    let now = ctx.now;
    match command {
        Command::Startup => {
            fx.extend([
                Effect::ScanCharacters,
                Effect::LoadCharacter {
                    id: state.settings.character.clone(),
                },
                Effect::ListAudioDevices,
                Effect::ListFonts,
            ]);
            if let Some(path) = state.settings.custom_font_path.clone() {
                fx.push(Effect::LoadCustomFont { path });
            }
            if let Some(path) = state.settings.todo_md_path.clone() {
                fx.push(Effect::LoadTodoMd { path });
            }
            if let Some(path) = state.settings.background_image.clone() {
                fx.push(Effect::ApplyBackgroundImage { path: Some(path) });
            }
        }
        Command::Tick => {
            study::tick(state, now, &mut fx);
            let (expressions, situation) = split_expressions(state);
            expressions.tick(&situation, now, ctx.rng);
        }
        Command::Study(c) => study::handle(state, c, now, &mut fx),
        Command::Expression(c) => expression::handle(state, c),
        Command::Editor(c) => editor::handle(state, c, &mut fx),
        Command::Todo(c) => todo::handle(state, c, &mut fx),
        Command::Audio(c) => audio::handle(state, c, now, &mut fx),
        Command::Settings(c) => settings::handle(state, c, now, &mut fx),
        Command::Pet(c) => pet::handle(state, c, &mut fx),
        Command::Character(c) => character::handle(state, c, &mut fx),
    }
    sync_random_schedule(state, now);
    fx
}

/// 根据当前角色原始表情目录与用户条件规则重新生成生效目录。
pub(super) fn apply_rules(state: &mut AppState) {
    let resolved = resolve_catalog(
        &state.raw_catalog,
        &state.settings.character,
        &state.expression_rules,
        &state.switches,
    );
    state.expressions.set_catalog(resolved);
}

/// 同时借出表情（可变）与判断条件（只读），两者是不同字段，互不冲突。
fn split_expressions(state: &mut AppState) -> (&mut Expressions, Situation) {
    let situation = Situation {
        activity: state.activity,
        music: state.audio.is_running(),
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
