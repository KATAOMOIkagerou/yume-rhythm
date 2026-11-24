//! 全局设置：默认值与取值范围集中在这里（功能清单第 11 节，以及重写新增项）。

use crate::model::shortcuts::Shortcuts;
use crate::model::theme::{AccentPreset, BackgroundMode, CORNER_RADIUS_DEFAULT, CornerMode};
use serde::{Deserialize, Serialize};

pub const RANDOM_INTERVAL_MIN_S: u32 = 5;
pub const RANDOM_INTERVAL_MAX_S: u32 = 86_400;
pub const RANDOM_INTERVAL_DEFAULT_S: u32 = 120;
pub const PET_SCALE_MIN: f32 = 0.5;
pub const PET_SCALE_MAX: f32 = 2.0;
pub const CONSOLE_OPACITY_MIN: f32 = 0.4;
pub const PET_OPACITY_MIN: f32 = 0.3;
pub const ELEMENT_OPACITY_MIN: f32 = 0.0;
pub const OPACITY_MAX: f32 = 1.0;
pub const BG_DIM_MIN: f32 = 0.2;
pub const BG_DIM_MAX: f32 = 0.95;
pub const BG_DIM_DEFAULT: f32 = 0.6;
pub const FONT_SIZE_MIN: u32 = 11;
pub const FONT_SIZE_MAX: u32 = 18;
pub const FONT_SIZE_DEFAULT: u32 = 13;
pub const DEFAULT_CHARACTER: &str = "sakura";
pub const DEFAULT_ZH_FONT_FAMILY: &str = "Microsoft YaHei UI";
pub const DEFAULT_EN_FONT_FAMILY: &str = "Segoe UI";

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
    /// 控制台背景模式：浅色、深色或自定义 HEX。
    pub background: BackgroundMode,
    /// 可选自定义背景色（`#rrggbb`）。
    pub custom_background: Option<String>,
    /// 自定义背景未输入 HEX 时的深浅兜底：切换前若是深色则保持深色底。
    pub custom_bg_dark: bool,
    /// 控制台背景透明度，0.0–1.0。
    pub bg_opacity: f32,
    /// 控制台卡片透明度，0.0–1.0。
    pub card_opacity: f32,
    /// 控制台强调色预设（含「自定义」第 6 项）。
    pub accent: AccentPreset,
    /// 切到「自定义」前或清除自定义色后兜底用的预设强调色。
    pub last_preset_accent: AccentPreset,
    /// 可选自定义强调色（`#rrggbb`）。
    pub custom_accent: Option<String>,
    /// 界面圆角模式与自定义圆角半径（px）。
    pub corner_mode: CornerMode,
    pub custom_corner_radius: u32,
    /// 控制台自定义背景图片路径。
    pub background_image: Option<String>,
    /// 背景图片之上的背景色遮罩浓度（0.2–0.95）。
    pub background_dim: f32,
    /// 控制台窗口透明度，0.4–1.0。
    pub console_opacity: f32,
    /// 桌宠窗口透明度，0.3–1.0。
    pub pet_opacity: f32,
    /// 自定义快捷键。
    pub shortcuts: Shortcuts,
    /// 中文字体家族名；`None` 表示默认（微软雅黑 UI）。
    pub zh_font_family: Option<String>,
    /// 英文字体家族名；`None` 表示默认（Segoe UI）。字形查找优先级高于中文字体。
    pub en_font_family: Option<String>,
    /// 正文字号（px，11–18，默认 13）。
    pub font_size: u32,
    /// 导入的本地 `.ttf`/`.otf`/`.ttc` 字体文件路径（重启时自动加载）。
    pub custom_font_paths: Vec<String>,
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
            background: BackgroundMode::default(),
            custom_background: None,
            custom_bg_dark: false,
            bg_opacity: OPACITY_MAX,
            card_opacity: OPACITY_MAX,
            accent: AccentPreset::Sakura,
            last_preset_accent: AccentPreset::Sakura,
            custom_accent: None,
            corner_mode: CornerMode::default(),
            custom_corner_radius: CORNER_RADIUS_DEFAULT,
            background_image: None,
            background_dim: BG_DIM_DEFAULT,
            console_opacity: OPACITY_MAX,
            pet_opacity: OPACITY_MAX,
            shortcuts: Shortcuts::default(),
            zh_font_family: None,
            en_font_family: None,
            font_size: FONT_SIZE_DEFAULT,
            custom_font_paths: Vec::new(),
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

/// 规整背景/卡片元素透明度到 0.0–1.0，按 5% 取整。
#[must_use]
pub fn clamp_element_opacity(opacity: f32) -> f32 {
    clamp_opacity(opacity, ELEMENT_OPACITY_MIN)
}

/// 规整字号大小到 11–18 px，默认 13。
#[must_use]
pub fn clamp_font_size(px: i64) -> u32 {
    let clamped = px.clamp(i64::from(FONT_SIZE_MIN), i64::from(FONT_SIZE_MAX));
    u32::try_from(clamped).unwrap_or(FONT_SIZE_DEFAULT)
}

/// 规整背景遮罩浓度到 0.2–0.95，按 5% 取整，非有限值回到 0.6。
#[must_use]
pub fn clamp_background_dim(dim: f32) -> f32 {
    if !dim.is_finite() {
        return BG_DIM_DEFAULT;
    }
    let stepped = (dim * 20.0).round() / 20.0;
    stepped.clamp(BG_DIM_MIN, BG_DIM_MAX)
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
    fn scale_and_typography_are_clamped() {
        assert!((clamp_pet_scale(f32::NAN) - 1.0).abs() < f32::EPSILON);
        assert!((clamp_pet_scale(9.0) - 2.0).abs() < f32::EPSILON);
        assert!((clamp_pet_scale(1.234) - 1.25).abs() < 1e-6);
        assert!((clamp_element_opacity(f32::NAN) - 1.0).abs() < f32::EPSILON);
        assert!((clamp_element_opacity(-0.2) - 0.0).abs() < f32::EPSILON);
        assert!((clamp_element_opacity(0.62) - 0.6).abs() < 1e-6);
        assert_eq!(clamp_font_size(2), 11);
        assert_eq!(clamp_font_size(99), 18);
    }
}
