//! 全局设置：默认值与取值范围集中在这里（功能清单第 11 节，以及重写新增项）。

use serde::{Deserialize, Serialize};

pub const RANDOM_INTERVAL_MIN_S: u32 = 5;
pub const RANDOM_INTERVAL_MAX_S: u32 = 86_400;
pub const RANDOM_INTERVAL_DEFAULT_S: u32 = 120;
pub const PET_SCALE_MIN: f32 = 0.5;
pub const PET_SCALE_MAX: f32 = 2.0;
pub const DEFAULT_CHARACTER: &str = "sakura";

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
