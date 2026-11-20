//! 系统服务：DPI、单实例、开机自启、全屏检测、屏幕位置、打开系统设置。

use crate::pet_window::ACTIVATE_MESSAGE;
use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO,
    MonitorFromPoint, MonitorFromRect,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteKeyValueW, RegSetValueExW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
    ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging as wm;
use windows::core::{HSTRING, w};

/// 声明按显示器感知 DPI。要在创建任何窗口之前调用；重复调用无害。
pub fn enable_per_monitor_dpi() {
    // SAFETY: 无指针参数。已设置过时会返回错误，忽略即可。
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// 单实例锁。持有期间其他实例无法获得。
pub struct InstanceGuard(HANDLE);

// SAFETY: 互斥体句柄可以跨线程持有，本类型只在 Drop 时使用它。
unsafe impl Send for InstanceGuard {}

/// 获取单实例锁。已有实例在运行时返回 `None`。
#[must_use]
pub fn acquire_single_instance(name: &str) -> Option<InstanceGuard> {
    // SAFETY: 创建命名互斥体后立即检查 GetLastError。
    unsafe {
        let handle = CreateMutexW(None, true, &HSTRING::from(name)).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            return None;
        }
        Some(InstanceGuard(handle))
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: 句柄由本对象持有。
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// 通知已运行的实例显示自己（桌宠窗口会收到 `PetEvent::ActivateRequested`）。
///
/// 新启动的进程有前台权限，先把权限让出去，已运行的实例才能把控制台带到最前面。
pub fn notify_existing_instance() {
    // SAFETY: 让出前台权限、广播已注册的窗口消息，都没有指针参数。
    unsafe {
        let _ = wm::AllowSetForegroundWindow(wm::ASFW_ANY);
        let msg = wm::RegisterWindowMessageW(ACTIVATE_MESSAGE);
        if msg != 0 {
            let _ = wm::PostMessageW(Some(wm::HWND_BROADCAST), msg, WPARAM(0), LPARAM(0));
        }
    }
}

/// 把窗口带到最前面并激活（最小化时先还原）。`hwnd` 来自其他库提供的原生句柄。
///
/// Windows 只允许刚收到用户输入的进程抢前台；做不到时至少把窗口提到同层最上面。
pub fn bring_to_front(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: 句柄由调用方提供；对无效句柄这些调用只会失败返回，不会访问内存。
    unsafe {
        if wm::IsIconic(hwnd).as_bool() {
            let _ = wm::ShowWindow(hwnd, wm::SW_RESTORE);
        }
        let flags = wm::SWP_NOMOVE | wm::SWP_NOSIZE | wm::SWP_SHOWWINDOW;
        let _ = wm::SetWindowPos(hwnd, Some(wm::HWND_TOP), 0, 0, 0, 0, flags);
        let _ = wm::SetForegroundWindow(hwnd);
    }
}

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

/// 开机自启：`command` 为 `Some` 时写入注册表 Run 项，`None` 时删除。
///
/// # Errors
/// 注册表操作失败。
pub fn set_autostart(name: &str, command: Option<&str>) -> windows::core::Result<()> {
    let name = HSTRING::from(name);
    // SAFETY: 打开当前用户的 Run 键，写入或删除字符串值，最后关闭键。
    unsafe {
        let Some(command) = command else {
            let err = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, &name);
            return if err.is_ok() || err.0 == 2 {
                Ok(())
            } else {
                Err(err.into())
            };
        };
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &raw mut key,
            None,
        )
        .ok()?;
        let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes: Vec<u8> = wide.iter().flat_map(|c| c.to_le_bytes()).collect();
        let result = RegSetValueExW(key, &name, None, REG_SZ, Some(&bytes)).ok();
        let _ = RegCloseKey(key);
        result
    }
}

/// 前台是否有全屏程序（游戏、全屏视频、演示）。
#[must_use]
pub fn fullscreen_app_active() -> bool {
    // SAFETY: 无参数。
    let state = unsafe { SHQueryUserNotificationState() };
    matches!(state, Ok(s) if s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE)
}

/// 打开 Windows 声音设置。
pub fn open_sound_settings() {
    // SAFETY: 参数均为静态字符串。
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("ms-settings:sound"),
            None,
            None,
            wm::SW_SHOWNORMAL,
        );
    }
}

/// 决定窗口位置：保存的位置仍在某个显示器上就沿用（并拉回工作区内），
/// 否则放在主显示器工作区右下角。
#[must_use]
pub fn place_on_screen(saved: Option<(i32, i32)>, size: (i32, i32)) -> (i32, i32) {
    let (w, h) = size;
    if let Some((x, y)) = saved {
        let rect = RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        };
        // SAFETY: `rect` 是局部变量。
        let on_screen =
            unsafe { !MonitorFromRect(&raw const rect, MONITOR_DEFAULTTONULL).is_invalid() };
        if on_screen {
            // SAFETY: 同上。
            let monitor = unsafe { MonitorFromRect(&raw const rect, MONITOR_DEFAULTTONEAREST) };
            let area = work_area(monitor);
            return (
                x.clamp(area.left, (area.right - w).max(area.left)),
                y.clamp(area.top, (area.bottom - h).max(area.top)),
            );
        }
    }
    // SAFETY: 原点总在主显示器上。
    let primary = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTONEAREST) };
    let area = work_area(primary);
    (area.right - w - 24, area.bottom - h)
}

fn work_area(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> RECT {
    let mut info = MONITORINFO {
        cbSize: u32::try_from(std::mem::size_of::<MONITORINFO>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `info` 是局部变量且 cbSize 已设置。
    unsafe {
        let _ = GetMonitorInfoW(monitor, &raw mut info);
    }
    info.rcWork
}
