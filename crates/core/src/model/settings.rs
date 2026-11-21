//! 全局设置：默认值与取值范围集中在这里（功能清单第 11 节，以及重写新增项）。

use crate::model::shortcuts::Shortcuts;
use crate::model::theme::ThemePreset;
use serde::{Deserialize, Serialize};

pub const RANDOM_INTERVAL_MIN_S: u32 = 5;
pub const RANDOM_INTERVAL_MAX_S: u32 = 86_400;
pub const RANDOM_INTERVAL_DEFAULT_S: u32 = 120;
pub const PET_SCALE_MIN: f32 = 0.5;
pub const PET_SCALE_MAX: f32 = 2.0;
pub const CONSOLE_OPACITY_MIN: f32 = 0.4;
pub const PET_OPACITY_MIN: f32 = 0.3;
pub const OPACITY_MAX: f32 = 1.0;
pub const DEFAULT_CHARACTER: &str = "sakura";
pub const DEFAULT_FONT_FAMILY: &str = "Microsoft YaHei UI";

/// 桌宠窗口位置，虚拟桌面物理像素。启动时若不在任何显示器上，由窗口层拉回可见区域。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// 倒计时结束提示音。
    pub end_sound: bool,
    /// 随机表情。
    pub random_enabled: bool,
    pub random_interval_s: u32,
    /// 控制台是否显示 BPM。只影响显示，不影响节拍检测。
    pub show_bpm: bool,
    /// 角色跟随节拍。
    pub auto_follow: bool,
    /// 系统音频的输出设备 id。`None` 表示系统默认输出。
    pub audio_device: Option<String>,
    /// 桌宠缩放，0.5–2.0。
    pub pet_scale: f32,
    /// 桌宠窗口底边中点（物理像素）。缩放变化时以它为锚点，所以存它而不存左上角。
    pub pet_position: Option<PetPosition>,
    pub autostart: bool,
    /// 有全屏程序（游戏、视频）时隐藏桌宠。
    pub hide_on_fullscreen: bool,
    /// 当前角色包 id。
    pub character: String,
    /// 控制台主题预设。
    pub theme: ThemePreset,
    /// 可选自定义强调色（`#rrggbb`）。
    pub custom_accent: Option<String>,
    /// 控制台窗口透明度，0.4–1.0。
    pub console_opacity: f32,
    /// 桌宠窗口透明度，0.3–1.0。
    pub pet_opacity: f32,
    /// 自定义快捷键。
    pub shortcuts: Shortcuts,
    /// 界面字体家族名；`None` 表示默认（微软雅黑 UI）。
    pub font_family: Option<String>,
    /// 导入的本地 `.ttf`/`.otf` 字体文件路径（重启时自动加载）。
    pub custom_font_path: Option<String>,
    /// 外接 Markdown 待办清单文件路径；`None` 表示内置清单。
    pub todo_md_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            end_sound: true,
            random_enabled: true,
            random_interval_s: RANDOM_INTERVAL_DEFAULT_S,
            show_bpm: true,
            auto_follow: true,
            audio_device: None,
            pet_scale: 1.0,
            pet_position: None,
            autostart: false,
            hide_on_fullscreen: true,
            character: DEFAULT_CHARACTER.into(),
            theme: ThemePreset::default(),
            custom_accent: None,
            console_opacity: OPACITY_MAX,
            pet_opacity: OPACITY_MAX,
            shortcuts: Shortcuts::default(),
            font_family: None,
            custom_font_path: None,
            todo_md_path: None,
        }
    }
}

/// 规整随机间隔到 5–86400 秒。
#[must_use]
pub fn clamp_random_interval(seconds: i64) -> u32 {
    let clamped = seconds.clamp(
        i64::from(RANDOM_INTERVAL_MIN_S),
        i64::from(RANDOM_INTERVAL_MAX_S),
    );
    u32::try_from(clamped).unwrap_or(RANDOM_INTERVAL_DEFAULT_S)
}

/// 规整缩放到 0.5–2.0，非法值回到 1.0。缩放按 5% 取整，避免持久化出一长串小数。
#[must_use]
pub fn clamp_pet_scale(scale: f32) -> f32 {
    if !scale.is_finite() {
        return 1.0;
    }
    let stepped = (scale * 20.0).round() / 20.0;
    stepped.clamp(PET_SCALE_MIN, PET_SCALE_MAX)
}

/// 规整控制台透明度到 0.4–1.0，按 5% 取整。
#[must_use]
pub fn clamp_console_opacity(opacity: f32) -> f32 {
    clamp_opacity(opacity, CONSOLE_OPACITY_MIN)
}

/// 规整桌宠透明度到 0.3–1.0，按 5% 取整。
#[must_use]
pub fn clamp_pet_opacity(opacity: f32) -> f32 {
    clamp_opacity(opacity, PET_OPACITY_MIN)
}

fn clamp_opacity(opacity: f32, min: f32) -> f32 {
    if !opacity.is_finite() {
        return OPACITY_MAX;
    }
    let stepped = (opacity * 20.0).round() / 20.0;
    stepped.clamp(min, OPACITY_MAX)
}

/// 将 0.0–1.0 的透明度映射为 Win32 0–255 字节值。
#[must_use]
pub fn opacity_to_u8(opacity: f32) -> u8 {
    let clamped = if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "已钳位到 0..=255"
    )]
    {
        (clamped * 255.0).round() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_is_clamped() {
        assert_eq!(clamp_random_interval(1), 5);
        assert_eq!(clamp_random_interval(1_000_000), 86_400);
        assert_eq!(clamp_random_interval(60), 60);
    }

    #[test]
    fn scale_is_clamped_and_stepped() {
        assert!((clamp_pet_scale(f32::NAN) - 1.0).abs() < f32::EPSILON);
        assert!((clamp_pet_scale(9.0) - 2.0).abs() < f32::EPSILON);
        assert!((clamp_pet_scale(1.234) - 1.25).abs() < 1e-6);
    }
}
