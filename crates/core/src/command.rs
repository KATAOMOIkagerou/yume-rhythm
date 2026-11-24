//! `Command`：修改状态的唯一入口（docs/架构.md 第 3 节）。按领域分组，每组由 `update` 下的同名模块处理。

use crate::model::audio::{AudioDevice, AudioError, Telemetry, TempoShift};
use crate::model::editor::AssetSlot;
use crate::model::events::{ActivityMode, EventContext};
use crate::model::expression::{ExpressionKind, ExpressionMeta};
use crate::model::pet::CharacterInfo;
use crate::model::settings::PetPosition;
use crate::model::shortcuts::ShortcutAction;
use crate::model::theme::{AccentPreset, BackgroundMode, CornerMode};
use crate::model::timer::TimerMode;
use crate::model::todo_md::TodoMdError;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// 程序启动后发送一次，触发加载角色、枚举音频设备等。
    Startup,
    /// 时钟推进。组装层每 200 ms 发送一次。
    Tick,
    Study(StudyCommand),
    Expression(ExpressionCommand),
    Editor(EditorCommand),
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorCommand {
    Open,
    Close,
    NewDraft,
    EditExisting(String),
    CancelDraft,
    SetKind(ExpressionKind),
    SetName(String),
    PickAsset(AssetSlot),
    AssetPicked { slot: AssetSlot, path: String },
    AssetRejected(String),
    ClearAsset(AssetSlot),
    SetCondition { context: EventContext, on: bool },
    SetRandom(bool),
    SetDurationMs(i64),
    ResetRule,
    Save,
    Delete(String),
    Saved,
    SaveFailed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoCommand {
    Rename(String),
    Add,
    Remove(u32),
    SetText {
        id: u32,
        text: String,
    },
    SetDone {
        id: u32,
        done: bool,
    },
    /// 打开系统文件选择框导入已有 Markdown 清单。
    PickImportMd,
    /// 打开系统保存框另存为外置 Markdown 清单并绑定。
    PickExportMd,
    /// 从当前绑定的外置 Markdown 重新读取内容。
    ReloadMd,
    /// 解绑外置 Markdown，切回内置清单（保留当前条目）。
    UnbindMd,
    // 以下由组装层回报。
    MdLoaded {
        path: String,
        content: String,
    },
    MdBound {
        path: String,
    },
    MdFailed(TodoMdError),
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
    /// 背景模式、透明度、强调色、界面圆角与控制台背景图。
    SetBackgroundMode(BackgroundMode),
    SetCustomBackground(Option<String>),
    SetBgOpacity(f32),
    SetCardOpacity(f32),
    SetAccent(AccentPreset),
    SetCustomAccent(Option<String>),
    SetCornerMode(CornerMode),
    SetCustomCornerRadius(i64),
    PickBackgroundImage,
    BackgroundImageLoaded {
        path: String,
    },
    BackgroundImageFailed(String),
    ClearBackgroundImage,
    SetBackgroundDim(f32),
    /// 控制台与桌宠透明度。
    SetConsoleOpacity(f32),
    SetPetOpacity(f32),
    /// 自定义快捷键。
    SetShortcut {
        action: ShortcutAction,
        binding: String,
    },
    ClearShortcut(ShortcutAction),
    ResetShortcuts,
    /// 组装层回报全局热键注册失败列表。
    GlobalHotkeysFailed(Vec<ShortcutAction>),
    /// 控制台窗口收到按键事件（由 core 匹配控制台内快捷键）。
    ConsoleKeyPressed {
        ctrl: bool,
        alt: bool,
        shift: bool,
        key: String,
    },
    /// 字体与字号设置。
    SetZhFontFamily(Option<String>),
    SetEnFontFamily(Option<String>),
    SetFontSize(i64),
    PickCustomFont,
    ResetFont,
    FontsListed(Vec<String>),
    CustomFontLoaded {
        path: String,
        family: String,
        has_cjk: bool,
    },
    CustomFontFailed(String),
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
        canvas: (u32, u32),
        custom_warning: Option<String>,
    },
    LoadFailed {
        id: String,
        message: String,
    },
}
