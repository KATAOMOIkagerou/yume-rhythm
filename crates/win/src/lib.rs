//! L1 Windows 系统适配。不认识任何业务类型，见 docs/架构.md 第 2 节。
//!
//! `unsafe` 只允许出现在本 crate，每处都写 `// SAFETY:` 说明。

pub mod storage;

#[cfg(windows)]
mod dialog;
#[cfg(windows)]
mod hotkeys;
#[cfg(windows)]
mod menu;
#[cfg(windows)]
mod msgloop;
#[cfg(windows)]
mod pet_window;
#[cfg(windows)]
mod surface;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod tray;

#[cfg(windows)]
pub use dialog::{pick_open_file, pick_save_file};
#[cfg(windows)]
pub use hotkeys::{Hotkeys, register as register_hotkeys};
#[cfg(windows)]
pub use msgloop::{FrameTimer, post_quit, pump_messages};
#[cfg(windows)]
pub use pet_window::{PetEvent, PetWindow};
#[cfg(windows)]
pub use system::{
    InstanceGuard, acquire_single_instance, bring_to_front, desktop_shell_active,
    enable_per_monitor_dpi, fullscreen_app_active, notify_existing_instance, open_sound_settings,
    place_on_screen, set_autostart, set_window_opacity,
};
#[cfg(windows)]
pub use tray::{Tray, TrayEvent, TrayItem};

/// 弹出菜单项。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Action {
        id: u32,
        label: String,
        checked: bool,
    },
    Separator,
}
