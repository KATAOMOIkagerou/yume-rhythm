//! 右键弹出菜单。

use crate::MenuItem;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging as wm;
use windows::core::HSTRING;

pub fn track(hwnd: HWND, items: &[MenuItem], x: i32, y: i32) -> Option<u32> {
    // SAFETY: 菜单在本函数内创建并销毁；窗口有效。
    unsafe {
        let menu = wm::CreatePopupMenu().ok()?;
        for item in items {
            match item {
                MenuItem::Separator => {
                    let _ = wm::AppendMenuW(menu, wm::MF_SEPARATOR, 0, None);
                }
                MenuItem::Action { id, label, checked } => {
                    let mut flags = wm::MF_STRING;
                    if *checked {
                        flags |= wm::MF_CHECKED;
                    }
                    let _ =
                        wm::AppendMenuW(menu, flags, *id as usize, &HSTRING::from(label.as_str()));
                }
            }
        }
        // 弹出菜单前必须把窗口设为前台，否则点菜单外不会关闭。
        let _ = wm::SetForegroundWindow(hwnd);
        let flags = wm::TPM_RETURNCMD | wm::TPM_RIGHTBUTTON | wm::TPM_NONOTIFY;
        let chosen = wm::TrackPopupMenu(menu, flags, x, y, None, hwnd, None);
        let _ = wm::PostMessageW(Some(hwnd), wm::WM_NULL, WPARAM(0), LPARAM(0));
        let _ = wm::DestroyMenu(menu);
        u32::try_from(chosen.0).ok().filter(|id| *id != 0)
    }
}
