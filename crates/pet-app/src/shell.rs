use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use pet_core::{AppAction, LoadedCharacterAssets};
use std::ptr;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MessageBoxW, MsgWaitForMultipleObjects, PeekMessageW, TranslateMessage,
    MB_ICONERROR, MB_OK, MSG, PM_REMOVE, QS_ALLINPUT, SW_SHOWNORMAL, WM_QUIT,
};

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(dw_process_id: u32) -> i32;
}

/// Attaches to the parent console in CLI diagnostic modes (`--self-test` / `--bench`)
/// when built with `windows_subsystem = "windows"`, so stdout works in PowerShell.
pub fn attach_parent_console_for_cli() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// Enables Per-Monitor V2 DPI awareness before creating any Win32 windows.
pub fn init_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Drains all pending Win32 messages on the current thread. Returns `true` if `WM_QUIT` was received.
pub fn pump_win32_messages() -> bool {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let has_msg = unsafe { PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) };
        if has_msg == 0 {
            return false;
        }
        if msg.message == WM_QUIT {
            return true;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Sleeps with zero CPU until either a Win32 input/message arrives or `timeout_ms` elapses.
pub fn wait_for_messages_or_timeout(timeout_ms: u32) {
    unsafe {
        let _ = MsgWaitForMultipleObjects(0, ptr::null(), 0, timeout_ms, QS_ALLINPUT);
    }
}

pub fn open_system_sound_settings() {
    let verb = to_wide("open");
    let target = to_wide("ms-settings:sound");
    let res = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (res as isize) <= 32 {
        let _ = std::process::Command::new("explorer.exe")
            .arg("ms-settings:sound")
            .spawn();
    }
}

pub fn show_fatal_error(message: &str) {
    let text = to_wide(&format!("桌宠节奏师启动失败：{message}"));
    let title = to_wide("桌宠节奏师");
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct ShellIntegration {
    _tray: TrayIcon,
    console_item: MenuItem,
    lock_item: MenuItem,
    audio_item: MenuItem,
    console_id: MenuId,
    lock_id: MenuId,
    audio_id: MenuId,
    quit_id: MenuId,
    _hotkeys: Option<GlobalHotKeyManager>,
    hotkey_console_id: Option<u32>,
    hotkey_lock_id: Option<u32>,
    last_console_visible: bool,
    last_locked: bool,
    last_audio_running: bool,
}

impl ShellIntegration {
    pub fn init(console_visible: bool, locked: bool, audio_running: bool) -> Result<Self, String> {
        let menu = Menu::new();
        let console_item = MenuItem::new(console_label(console_visible), true, None);
        let lock_item = MenuItem::new(lock_label(locked), true, None);
        let audio_item = MenuItem::new(audio_label(audio_running), true, None);
        let quit_item = MenuItem::new("退出桌宠节奏师", true, None);

        menu.append_items(&[
            &console_item,
            &lock_item,
            &audio_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])
        .map_err(|e| format!("初始化托盘菜单失败：{e}"))?;

        let (rgba, w, h) = LoadedCharacterAssets::embedded_tray_icon_rgba(24)?;
        let icon = Icon::from_rgba(rgba, w, h).map_err(|e| format!("创建托盘图标失败：{e}"))?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("桌宠节奏师")
            .with_icon(icon)
            .build()
            .map_err(|e| format!("创建系统托盘失败：{e}"))?;

        let console_id = console_item.id().clone();
        let lock_id = lock_item.id().clone();
        let audio_id = audio_item.id().clone();
        let quit_id = quit_item.id().clone();

        // Register global hotkeys gracefully (do not fail startup if already occupied).
        let (hotkeys, hotkey_console_id, hotkey_lock_id) = match GlobalHotKeyManager::new() {
            Ok(manager) => {
                let console_hk = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);
                let lock_hk = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL);
                let c_id = match manager.register(console_hk) {
                    Ok(()) => Some(console_hk.id()),
                    Err(err) => {
                        eprintln!("全局快捷键 Ctrl+Alt+C 注册失败（可能被其他程序占用）：{err}");
                        None
                    }
                };
                let l_id = match manager.register(lock_hk) {
                    Ok(()) => Some(lock_hk.id()),
                    Err(err) => {
                        eprintln!("全局快捷键 Ctrl+Alt+L 注册失败（可能被其他程序占用）：{err}");
                        None
                    }
                };
                (Some(manager), c_id, l_id)
            }
            Err(err) => {
                eprintln!("初始化全局快捷键管理器失败：{err}");
                (None, None, None)
            }
        };

        Ok(Self {
            _tray: tray,
            console_item,
            lock_item,
            audio_item,
            console_id,
            lock_id,
            audio_id,
            quit_id,
            _hotkeys: hotkeys,
            hotkey_console_id,
            hotkey_lock_id,
            last_console_visible: console_visible,
            last_locked: locked,
            last_audio_running: audio_running,
        })
    }

    pub fn sync_menu_state(&mut self, console_visible: bool, locked: bool, audio_running: bool) {
        if self.last_console_visible != console_visible {
            self.console_item.set_text(console_label(console_visible));
            self.last_console_visible = console_visible;
        }
        if self.last_locked != locked {
            self.lock_item.set_text(lock_label(locked));
            self.last_locked = locked;
        }
        if self.last_audio_running != audio_running {
            self.audio_item.set_text(audio_label(audio_running));
            self.last_audio_running = audio_running;
        }
    }

    pub fn poll_actions(&self) -> Vec<AppAction> {
        let mut actions = Vec::new();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.console_id {
                actions.push(AppAction::ToggleConsole);
            } else if event.id == self.lock_id {
                actions.push(AppAction::ToggleLock);
            } else if event.id == self.audio_id {
                actions.push(AppAction::TogglePlayLastAudio);
            } else if event.id == self.quit_id {
                actions.push(AppAction::Quit);
            }
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                actions.push(AppAction::ShowConsole);
            }
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state != HotKeyState::Pressed {
                continue;
            }
            if self.hotkey_console_id == Some(event.id) {
                actions.push(AppAction::ToggleConsole);
            } else if self.hotkey_lock_id == Some(event.id) {
                actions.push(AppAction::ToggleLock);
            }
        }

        actions
    }
}

fn console_label(visible: bool) -> &'static str {
    if visible {
        "隐藏控制台（Ctrl+Alt+C）"
    } else {
        "打开控制台（Ctrl+Alt+C）"
    }
}

fn lock_label(locked: bool) -> &'static str {
    if locked {
        "解除桌宠锁定（Ctrl+Alt+L）"
    } else {
        "锁定桌宠（点击穿透）（Ctrl+Alt+L）"
    }
}

fn audio_label(running: bool) -> &'static str {
    if running {
        "停止音频同步"
    } else {
        "开始音频同步"
    }
}
