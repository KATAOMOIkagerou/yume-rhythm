//! 主线程上的系统外壳：托盘、全局热键、定时器，以及音频事件到命令的翻译。

use crate::ToMain;
use crate::app;
use std::time::Duration;
use yume_audio::{AudioEvent, ErrorKind};
use yume_core::Command;
use yume_core::command::{AudioCommand, PetCommand, SettingsCommand};
use yume_core::model::audio::{AudioDevice, AudioError};
use yume_core::model::shortcuts::{ShortcutAction, Shortcuts};
use yume_win::{Hotkeys, Tray, TrayEvent, TrayItem};

const ICON: &[u8] = include_bytes!("../assets/icon.png");

pub fn tray(to_main: ToMain) -> Option<Tray> {
    let icon = match yume_character::sprite::decode_png(ICON) {
        Ok(icon) => icon,
        Err(e) => {
            log::warn!("托盘图标解码失败：{e}");
            return None;
        }
    };
    let item = |id, label: &str, checked| {
        Some(TrayItem {
            id,
            label: label.into(),
            checked,
        })
    };
    let entries = [
        item("console", "打开控制台", None),
        item("audio", "开始听歌", None),
        item("lock", "锁定桌宠", Some(false)),
        None,
        item("autostart", "开机自启", Some(false)),
        None,
        item("quit", "退出", None),
    ];
    let on_event = move |event: TrayEvent| {
        let cmd = match event {
            TrayEvent::DoubleClick => Some(Command::Pet(PetCommand::ShowConsole)),
            TrayEvent::Menu(id) => match id.as_str() {
                "console" => Some(Command::Pet(PetCommand::ShowConsole)),
                "audio" => Some(Command::Audio(AudioCommand::Toggle)),
                "lock" => Some(Command::Pet(PetCommand::ToggleLocked)),
                "autostart" => Some(Command::Settings(SettingsCommand::ToggleAutostart)),
                "quit" => Some(Command::Pet(PetCommand::Quit)),
                _ => None,
            },
        };
        if let Some(cmd) = cmd {
            to_main(cmd);
        }
    };
    match Tray::new(icon.rgba, icon.width, "韵梦桌伴", &entries, on_event) {
        Ok(tray) => Some(tray),
        Err(e) => {
            log::warn!("托盘图标创建失败：{e}");
            None
        }
    }
}

/// 按当前设置注册全局快捷键。被占用时只记日志并回报给界面，托盘仍可用。
pub fn hotkeys(to_main: ToMain, shortcuts: &Shortcuts) -> (Option<Hotkeys>, Vec<ShortcutAction>) {
    let owned = shortcuts.global_accelerators();
    let refs: Vec<(&'static str, &str)> = owned.iter().map(|(id, a)| (*id, a.as_str())).collect();
    let on_press = move |id: &'static str| {
        let cmd = match ShortcutAction::from_id(id) {
            Some(ShortcutAction::GlobalToggleConsole) => {
                Some(Command::Pet(PetCommand::ToggleConsole))
            }
            Some(ShortcutAction::GlobalToggleLocked) => {
                Some(Command::Pet(PetCommand::ToggleLocked))
            }
            Some(ShortcutAction::GlobalToggleAudio) => Some(Command::Audio(AudioCommand::Toggle)),
            _ => None,
        };
        if let Some(cmd) = cmd {
            to_main(cmd);
        }
    };
    match yume_win::register_hotkeys(&refs, on_press) {
        Ok((hotkeys, failed)) => {
            let actions = map_failed_hotkeys(&failed);
            (Some(hotkeys), actions)
        }
        Err(e) => {
            log::warn!("全局快捷键不可用：{e}");
            (None, Vec::new())
        }
    }
}

/// 热更新全局快捷键绑定，返回注册失败的动作列表。
pub fn rebind_hotkeys(hotkeys: &mut Hotkeys, shortcuts: &Shortcuts) -> Vec<ShortcutAction> {
    let owned = shortcuts.global_accelerators();
    let refs: Vec<(&'static str, &str)> = owned.iter().map(|(id, a)| (*id, a.as_str())).collect();
    let failed = hotkeys.rebind(&refs);
    map_failed_hotkeys(&failed)
}

fn map_failed_hotkeys(failed: &[&'static str]) -> Vec<ShortcutAction> {
    let mut actions = Vec::new();
    for &id in failed {
        log::warn!("全局快捷键 {id} 注册失败，可能被其他程序占用");
        if let Some(action) = ShortcutAction::from_id(id) {
            actions.push(action);
        }
    }
    actions
}

/// 主线程定时器：计时推进与全屏检测。返回值要一直持有。
pub fn timers() -> [slint::Timer; 2] {
    let tick = slint::Timer::default();
    tick.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        || app::dispatch(Command::Tick),
    );
    let fullscreen = slint::Timer::default();
    let mut last = false;
    fullscreen.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            let now = yume_win::fullscreen_app_active();
            if now != last {
                last = now;
                app::dispatch(Command::Pet(PetCommand::FullscreenChanged(now)));
            }
        },
    );
    [tick, fullscreen]
}

/// 音频服务的回报 → 命令。
#[must_use]
pub fn audio_command(event: AudioEvent) -> Command {
    Command::Audio(match event {
        AudioEvent::Started { request } => AudioCommand::Started { request },
        AudioEvent::Failed {
            request,
            kind,
            detail,
        } => {
            log::warn!("音频启动失败：{detail}");
            let error = match kind {
                ErrorKind::DeviceUnavailable => AudioError::DeviceUnavailable,
                ErrorKind::UnsupportedFile => AudioError::UnsupportedFile,
                ErrorKind::FileUnreadable => AudioError::FileUnreadable,
                ErrorKind::Other => AudioError::Other,
            };
            AudioCommand::Failed { request, error }
        }
        AudioEvent::Devices(list) => AudioCommand::DevicesListed(
            list.into_iter()
                .map(|d| AudioDevice {
                    id: d.id,
                    name: d.name,
                })
                .collect(),
        ),
    })
}
