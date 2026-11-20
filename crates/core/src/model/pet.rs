//! 桌宠窗口、控制台与角色包的运行时状态（不持久化的部分）。

/// 确认框。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dialog {
    #[default]
    None,
    /// 暂停倒计时前确认。
    ConfirmPause,
    /// 计时中从自习切到休闲前确认。确认后暂停并切换。
    ConfirmLeaveStudy,
    /// 取消暂停或取消离开后的鼓励。
    Encourage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PetState {
    /// 锁定后整窗点击穿透、不可拖动。不持久化，重启后解除。
    pub locked: bool,
    pub console_visible: bool,
    /// 前台有全屏程序。
    pub fullscreen_active: bool,
}

/// 可选角色。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CharacterStatus {
    #[default]
    Loading,
    Ready,
    /// 加载失败。若之前已有角色，桌宠继续显示旧角色。
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterState {
    pub status: CharacterStatus,
    /// 已加载角色的显示名。
    pub name: String,
    pub available: Vec<CharacterInfo>,
}
