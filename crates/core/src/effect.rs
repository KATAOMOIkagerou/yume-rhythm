//! `Effect`：需要外部执行的一次性动作（docs/架构.md 第 3 节）。
//!
//! 持续性的属性（桌宠锁定、可见性、缩放、控制台显隐、开机自启、需要持久化的数据）
//! 不用 Effect，由组装层比较 `derive` 中的派生值前后差异后应用，这样不会漏发。

use crate::model::audio::{AudioSource, TempoShift};
use crate::model::editor::{AssetSlot, CustomExpressionSpec};

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
    /// 弹出文件选择框导入外置 Markdown 待办清单。
    PickTodoMdImport,
    /// 弹出文件保存框新建并绑定外置 Markdown 待办清单。
    PickTodoMdExport,
    /// 从磁盘读取指定外置 Markdown 待办清单。
    LoadTodoMd {
        path: String,
    },
    /// 弹出文件选择框导入 `.ttf`/`.otf` 字体文件。
    PickFontFile,
    /// 加载并注册本地自定义字体文件。
    LoadCustomFont {
        path: String,
    },
    /// 枚举系统已安装字体。
    ListFonts,
    /// 弹出文件选择框导入控制台背景图片。
    PickBackgroundImage,
    /// 将选定的背景图片设置到控制台窗口，`None` 表示清除。
    ApplyBackgroundImage {
        path: Option<String>,
    },
    /// 弹出 PNG 选择框导入自定义表情某个槽位的素材。
    PickExpressionAsset {
        slot: AssetSlot,
    },
    /// 复制 PNG 并写出 `expressions.json`。
    SaveCustomExpression(CustomExpressionSpec),
    /// 删除自定义表情及其 PNG 文件。
    DeleteCustomExpression {
        character: String,
        id: String,
    },
    /// 把控制台窗口带到最前面。显示控制台的命令都会附带它：窗口已经可见但被挡住时也要能看到。
    FocusConsole,
    Quit,
}
