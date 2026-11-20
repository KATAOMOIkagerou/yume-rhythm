//! `Effect`：需要外部执行的一次性动作（docs/架构.md 第 3 节）。
//!
//! 持续性的属性（桌宠锁定、可见性、缩放、控制台显隐、开机自启、需要持久化的数据）
//! 不用 Effect，由组装层比较 `derive` 中的派生值前后差异后应用，这样不会漏发。

use crate::model::audio::{AudioSource, TempoShift};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// 倒计时结束的三音提示。
    PlayChime,
    StartAudio {
        request: u64,
        source: AudioSource,
        /// 系统音频的输出设备，`None` 为默认输出。
        device: Option<String>,
    },
    StopAudio,
    ShiftTempo(TempoShift),
    ListAudioDevices,
    ScanCharacters,
    LoadCharacter {
        id: String,
    },
    /// 打开 Windows 声音设置。
    OpenSoundSettings,
    /// 把控制台窗口带到最前面。显示控制台的命令都会附带它：窗口已经可见但被挡住时也要能看到。
    FocusConsole,
    Quit,
}
