//! 系统服务：DPI、单实例、开机自启、全屏检测、屏幕位置、打开系统设置。

use crate::pet_window::ACTIVATE_MESSAGE;
use windows::Win32::Foundation::{
    COLORREF, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO,
    MonitorFromPoint, MonitorFromRect, MonitorFromWindow,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteKeyValueW, RegSetValueExW,
};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
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

/// 设置任意原生窗口的整体透明度（0–255）。255 时清除分层样式避免额外合成开销。
pub fn set_window_opacity(hwnd: isize, alpha: u8) {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: 句柄由调用方提供；无效句柄时 Win32 调用安全返回错误，不访问内存。
    unsafe {
        let ex = wm::GetWindowLongPtrW(hwnd, wm::GWL_EXSTYLE);
        let layered = wm::WS_EX_LAYERED.0 as isize;
        if alpha == 255 {
            if ex & layered != 0 {
                wm::SetWindowLongPtrW(hwnd, wm::GWL_EXSTYLE, ex & !layered);
            }
            return;
        }
        if ex & layered == 0 {
            wm::SetWindowLongPtrW(hwnd, wm::GWL_EXSTYLE, ex | layered);
        }
        let _ = wm::SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, wm::LWA_ALPHA);
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
///
/// 当桌面上所有窗口关闭或最小化时，系统会把焦点转移到 `WorkerW`/`Progman` 等满屏桌面宿主，
/// 导致 `SHQueryUserNotificationState` 返回 `QUNS_BUSY`；因此这里必须结合前台窗口类与几何检查排除桌面与壳层窗口。
#[must_use]
pub fn fullscreen_app_active() -> bool {
    // SAFETY: 仅查询系统前台窗口句柄与通知状态，无指针写入或生命周期依赖。
    let fg = unsafe { wm::GetForegroundWindow() };
    if fg.is_invalid() {
        return false;
    }
    // SAFETY: 传入经判空的 `HWND` 查询可见性、最小化状态与所属进程，Win32 对失效句柄返回失败不会越界。
    let (visible, iconic, own_process) = unsafe {
        let mut pid = 0u32;
        let _ = wm::GetWindowThreadProcessId(fg, Some(&raw mut pid));
        (
            wm::IsWindowVisible(fg).as_bool(),
            wm::IsIconic(fg).as_bool(),
            pid == GetCurrentProcessId(),
        )
    };
    if !visible || iconic || own_process || is_window_cloaked(fg) {
        return false;
    }
    if is_shell_or_desktop_window(fg) || ancestor_is_shell(fg) {
        return false;
    }

    // SAFETY: 无指针或外部内存依赖。
    let Ok(state) = (unsafe { SHQueryUserNotificationState() }) else {
        return false;
    };
    if state == QUNS_RUNNING_D3D_FULL_SCREEN {
        return true;
    }
    if state != QUNS_BUSY && state != QUNS_PRESENTATION_MODE {
        return false;
    }
    window_is_borderless_fullscreen(fg)
}

/// 当前前台是否激活了系统桌面或任务栏等壳层窗口（关闭全部窗口或 `Win+D` 显示桌面时为 `true`）。
#[must_use]
pub fn desktop_shell_active() -> bool {
    // SAFETY: 无参数，只返回当前前台窗口句柄。
    let fg = unsafe { wm::GetForegroundWindow() };
    !fg.is_invalid() && (is_shell_or_desktop_window(fg) || ancestor_is_shell(fg))
}

fn ancestor_is_shell(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` 非空；`GetAncestor` 对无效句柄安全返回空句柄。
    let root = unsafe { wm::GetAncestor(hwnd, wm::GA_ROOT) };
    !root.is_invalid() && root != hwnd && is_shell_or_desktop_window(root)
}

fn is_shell_or_desktop_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return true;
    }
    // SAFETY: 与桌面/壳层句柄比较并查找子视图 `SHELLDLL_DefView`，均无生命周期风险。
    unsafe {
        if hwnd == wm::GetDesktopWindow() || hwnd == wm::GetShellWindow() {
            return true;
        }
        if let Ok(def_view) = wm::FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None)
            && !def_view.is_invalid()
        {
            return true;
        }
    }
    let class = window_class_name(hwnd);
    is_shell_class_name(&class)
}

fn window_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: `buf` 是固定长度局部变量，传入其有效切片缓冲。
    let len = unsafe { wm::GetClassNameW(hwnd, &mut buf) };
    usize::try_from(len)
        .ok()
        .and_then(|n| buf.get(..n))
        .map(String::from_utf16_lossy)
        .unwrap_or_default()
}

fn is_window_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    let size = u32::try_from(std::mem::size_of::<u32>()).unwrap_or(4);
    // SAFETY: `cloaked` 为局部变量，大小与 `u32` 严格一致。
    let hr = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&raw mut cloaked).cast(), size) };
    hr.is_ok() && cloaked != 0
}

fn window_is_borderless_fullscreen(hwnd: HWND) -> bool {
    // SAFETY: 均为标准 Win32 查询调用，指针均指向栈上局部变量。
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mon_rect = monitor_bounds(monitor);
        let mut win_rect = RECT::default();
        if wm::GetWindowRect(hwnd, &raw mut win_rect).is_err()
            || !rect_covers_monitor(win_rect, mon_rect)
        {
            return false;
        }
        let style = u32::from_ne_bytes(wm::GetWindowLongW(hwnd, wm::GWL_STYLE).to_ne_bytes());
        if (style & wm::WS_CAPTION.0) == wm::WS_CAPTION.0 && wm::IsZoomed(hwnd).as_bool() {
            let mut client_top = POINT::default();
            if ClientToScreen(hwnd, &raw mut client_top).as_bool() && client_top.y > mon_rect.top {
                return false;
            }
        }
        true
    }
}

fn is_shell_class_name(class: &str) -> bool {
    matches!(
        class,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "Windows.UI.Core.CoreWindow"
            | "XamlExplorerHostIslandWindow"
            | "MultitaskingViewFrame"
            | "TaskListThumbnailWnd"
            | "ForegroundStaging"
    )
}

fn rect_covers_monitor(win: RECT, mon: RECT) -> bool {
    mon.right > mon.left
        && mon.bottom > mon.top
        && win.left <= mon.left
        && win.top <= mon.top
        && win.right >= mon.right
        && win.bottom >= mon.bottom
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

fn monitor_bounds(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> RECT {
    let mut info = MONITORINFO {
        cbSize: u32::try_from(std::mem::size_of::<MONITORINFO>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `info` 是局部变量且 cbSize 已设置。
    unsafe {
        let _ = GetMonitorInfoW(monitor, &raw mut info);
    }
    info.rcMonitor
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_class_names_are_filtered_from_fullscreen() {
        for cls in [
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "NotifyIconOverflowWindow",
        ] {
            assert!(is_shell_class_name(cls), "{cls} 应被识别为桌面/壳层窗口");
        }
        assert!(!is_shell_class_name("UnityWndClass"));
        assert!(!is_shell_class_name("Chrome_WidgetWin_1"));
    }

    #[test]
    fn rect_covers_monitor_handles_primary_and_negative_monitors() {
        let primary = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        assert!(rect_covers_monitor(primary, primary));
        assert!(!rect_covers_monitor(
            RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
            primary
        ));

        let secondary_left = RECT {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        assert!(rect_covers_monitor(secondary_left, secondary_left));
        assert!(!rect_covers_monitor(primary, secondary_left));
    }
}
