//! `Command`：修改状态的唯一入口（docs/架构.md 第 3 节）。按领域分组，每组由 `update` 下的同名模块处理。

use crate::model::audio::{AudioDevice, AudioError, Telemetry, TempoShift};
use crate::model::events::{ActivityMode, EventContext, RandomEvent};
use crate::model::expression::ExpressionMeta;
use crate::model::pet::CharacterInfo;
use crate::model::settings::PetPosition;
use crate::model::timer::TimerMode;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// 程序启动后发送一次，触发加载角色、枚举音频设备等。
    Startup,
    /// 时钟推进。组装层每 200 ms 发送一次。
    Tick,
    Study(StudyCommand),
    Expression(ExpressionCommand),
    Todo(TodoCommand),
    Audio(AudioCommand),
    Settings(SettingsCommand),
    Pet(PetCommand),
    Character(CharacterCommand),
}

/// 学习计时与活动模式。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StudyCommand {
    SelectActivity(ActivityMode),
    SetTimerMode(TimerMode),
    SetMinutes(i64),
    /// 开始或继续。
    Start,
    /// 请求暂停。倒计时会先弹确认，正计时直接暂停。
    RequestPause,
    Reset,
    /// 确认当前对话框。
    ConfirmDialog,
    /// 取消当前对话框。
    CancelDialog,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpressionCommand {
    /// 手动选择表情。再次选择当前表情会回到自然表情。
    Select(String),
    SetEventSwitch {
        context: EventContext,
        event: RandomEvent,
        on: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoCommand {
    Rename(String),
    Add,
    Remove(u32),
    SetText { id: u32, text: String },
    SetDone { id: u32, done: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioCommand {
    /// 空格键：运行中则停止，否则重放上次的音源。
    Toggle,
    StartSystem,
    /// Q 键或停止按钮。
    Stop,
    /// 拖入文件。取第一个音频文件播放。
    FilesDropped(Vec<String>),
    SelectDevice(Option<String>),
    RefreshDevices,
    ShiftTempo(TempoShift),
    // 以下由组装层回报，`request` 用于丢弃过期回报。
    Started {
        request: u64,
    },
    Failed {
        request: u64,
        error: AudioError,
    },
    Ended {
        request: u64,
    },
    Telemetry(Telemetry),
    DevicesListed(Vec<AudioDevice>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsCommand {
    SetEndSound(bool),
    SetRandomEnabled(bool),
    SetRandomInterval(i64),
    SetShowBpm(bool),
    SetAutoFollow(bool),
    SetPetScale(f32),
    SetAutostart(bool),
    /// 托盘菜单的勾选项。
    ToggleAutostart,
    SetHideOnFullscreen(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PetCommand {
    SetLocked(bool),
    ToggleLocked,
    ShowConsole,
    HideConsole,
    ToggleConsole,
    /// 拖动结束后的新位置。
    Moved(PetPosition),
    FullscreenChanged(bool),
    OpenSoundSettings,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CharacterCommand {
    Select(String),
    /// 重新读取当前角色包（"重载素材"）。
    Reload,
    // 以下由组装层回报。
    Scanned(Vec<CharacterInfo>),
    Loaded {
        id: String,
        name: String,
        expressions: Vec<ExpressionMeta>,
    },
    LoadFailed {
        id: String,
        message: String,
    },
}
