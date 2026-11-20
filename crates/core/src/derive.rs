//! 由 `AppState` 推出的持续性输出。组装层每次 `update` 后比较前后差异，有变化才应用到窗口或桌宠。

use crate::model::events::{ActivityMode, RandomEvent};
use crate::model::expression::ExpressionKind;
use crate::state::AppState;

/// 桌宠"该显示什么"。角色渲染器只读它，不关心业务规则。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetScene {
    pub activity: ActivityMode,
    /// 正在放音乐（显示耳机）。
    pub music: bool,
    /// 跟随节拍（音乐中且开启了"自动跟拍"）。
    pub follow_beat: bool,
    /// 学习计时正在运行（计时钟发光）。
    pub timer_running: bool,
    /// 基础表情 id，决定眼和嘴。
    pub base_expression: String,
    /// 叠加的附加表情 id。
    pub accessory_expression: Option<String>,
    /// 当前生效表情关联的事件。
    pub event: Option<RandomEvent>,
    /// 打瞌睡下垂：不在放音乐，且当前事件是打瞌睡。
    pub dozing: bool,
}

/// 桌宠窗口属性。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PetWindow {
    pub locked: bool,
    pub visible: bool,
    pub scale: f32,
}

#[must_use]
pub fn pet_scene(state: &AppState) -> PetScene {
    let situation = state.situation();
    let effective = state.expressions.effective(&situation);
    let base = state.expressions.effective_base(&situation).map_or_else(
        || crate::model::expression::NEUTRAL.to_string(),
        |m| m.id.clone(),
    );
    let accessory = effective
        .filter(|m| m.kind == ExpressionKind::Accessory)
        .map(|m| m.id.clone());
    let event = effective.and_then(|m| m.event);
    let music = situation.music;
    PetScene {
        activity: state.activity,
        music,
        follow_beat: music && state.settings.auto_follow,
        timer_running: state.timer.is_running(),
        base_expression: base,
        accessory_expression: accessory,
        event,
        dozing: !music && event == Some(RandomEvent::Doze),
    }
}

#[must_use]
pub fn pet_window(state: &AppState) -> PetWindow {
    PetWindow {
        locked: state.pet.locked,
        visible: !(state.settings.hide_on_fullscreen && state.pet.fullscreen_active),
        scale: state.settings.pet_scale,
    }
}
