use crate::{AnchorRect, CONSOLE_LOGICAL_HEIGHT, CONSOLE_LOGICAL_WIDTH, CONSOLE_MIN_HEIGHT, CONSOLE_MIN_WIDTH};
use egui::{CursorIcon, Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, Vec2};
use pet_core::is_supported_audio_file;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EndPaint,
    GetDC, GetMonitorInfoW, InvalidateRect, MonitorFromPoint, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
    GetWindowLongPtrW, LoadCursorW, PeekMessageW, RegisterClassExW, SetCursor, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, CS_DBLCLKS, CS_HREDRAW,
    CS_VREDRAW, GWLP_USERDATA, HCURSOR, HTCLIENT, IDC_ARROW, IDC_HAND, IDC_IBEAM, IDC_NO,
    IDC_SIZEALL, IDC_SIZENS, IDC_SIZEWE, MINMAXINFO, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOZORDER,
    SW_SHOWNORMAL, WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_DPICHANGED, WM_DROPFILES, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WNDCLASSEXW, WS_CAPTION, WS_EX_ACCEPTFILES, WS_EX_APPWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU, WS_THICKFRAME,
};

const CF_UNICODETEXT: u32 = 13;
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Shortcut key pressed while console window had focus (evaluated only if egui text box is unfocused).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleShortcut {
    ToggleAudio,
    StopAudio,
    HideConsole,
}

/// 32-bit top-down BGRA Win32 DIBSection surface for zero-GPU `BitBlt` presentation.
pub struct DibSurface {
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    bits: *mut u8,
    width: u32,
    height: u32,
}

impl DibSurface {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let w = width.max(1);
        let h = height.max(1);
        unsafe {
            let screen_dc = GetDC(null_mut());
            let mem_dc = CreateCompatibleDC(screen_dc);
            ReleaseDC(null_mut(), screen_dc);
            if mem_dc.is_null() {
                return Err("创建兼容 DC 失败".into());
            }

            let mut bmi: BITMAPINFO = zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // Top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };

            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(mem_dc);
                return Err("创建 32-bit DIBSection 失败".into());
            }

            let old_bitmap = SelectObject(mem_dc, bitmap);
            Ok(Self {
                mem_dc,
                bitmap,
                old_bitmap,
                bits: bits as *mut u8,
                width: w,
                height: h,
            })
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = (self.width as usize) * (self.height as usize) * 4;
        unsafe { std::slice::from_raw_parts_mut(self.bits, len) }
    }

    pub fn present_to_dc(&self, target_dc: HDC) {
        unsafe {
            BitBlt(
                target_dc,
                0,
                0,
                self.width as i32,
                self.height as i32,
                self.mem_dc,
                0,
                0,
                SRCCOPY,
            );
        }
    }
}

impl Drop for DibSurface {
    fn drop(&mut self) {
        unsafe {
            if !self.mem_dc.is_null() {
                if !self.old_bitmap.is_null() {
                    SelectObject(self.mem_dc, self.old_bitmap);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                DeleteDC(self.mem_dc);
            }
        }
    }
}

struct SharedWindowState {
    dib: Option<DibSurface>,
    client_width: u32,
    client_height: u32,
    scale_factor: f32,
    pending_events: Vec<Event>,
    pending_dropped_audio: Vec<PathBuf>,
    pending_shortcuts: Vec<ConsoleShortcut>,
    close_requested: bool,
    dirty: bool,
    tracking_mouse_leave: bool,
    high_surrogate: Option<u16>,
    cursor_icon: CursorIcon,
}

impl SharedWindowState {
    fn new(width: u32, height: u32, scale_factor: f32) -> Self {
        Self {
            dib: None,
            client_width: width.max(1),
            client_height: height.max(1),
            scale_factor: scale_factor.clamp(0.75, 4.0),
            pending_events: Vec::with_capacity(32),
            pending_dropped_audio: Vec::new(),
            pending_shortcuts: Vec::new(),
            close_requested: false,
            dirty: true,
            tracking_mouse_leave: false,
            high_surrogate: None,
            cursor_icon: CursorIcon::Default,
        }
    }

    fn ensure_dib(&mut self) -> Result<&mut DibSurface, String> {
        let need_new = match &self.dib {
            Some(d) => d.width() != self.client_width || d.height() != self.client_height,
            None => true,
        };
        if need_new {
            self.dib = Some(DibSurface::new(self.client_width, self.client_height)?);
        }
        Ok(self.dib.as_mut().expect("dib initialized"))
    }
}

/// Native Win32 resizable window backed by a 32-bit DIBSection (zero GPU, zero WebView2).
pub struct NativeConsoleWindow {
    hwnd: HWND,
    state: Box<SharedWindowState>,
}

impl NativeConsoleWindow {
    pub fn create_near(anchor: AnchorRect) -> Result<Self, String> {
        register_window_class()?;

        let system_dpi = unsafe { GetDpiForSystem() }.max(96);
        let scale = if anchor.scale_factor > 0.5 && anchor.scale_factor < 4.5 {
            anchor.scale_factor
        } else {
            system_dpi as f32 / 96.0
        };
        let dpi = (scale * 96.0).round() as u32;

        let style = WS_OVERLAPPED
            | WS_CAPTION
            | WS_SYSMENU
            | WS_THICKFRAME
            | WS_MINIMIZEBOX
            | WS_MAXIMIZEBOX;
        let ex_style = WS_EX_APPWINDOW | WS_EX_ACCEPTFILES;

        let client_w = ((CONSOLE_LOGICAL_WIDTH as f32) * scale).round() as i32;
        let client_h = ((CONSOLE_LOGICAL_HEIGHT as f32) * scale).round() as i32;

        let mut win_rect = RECT {
            left: 0,
            top: 0,
            right: client_w,
            bottom: client_h,
        };
        unsafe {
            AdjustWindowRectExForDpi(&mut win_rect, style, 0, ex_style, dpi);
        }
        let outer_w = (win_rect.right - win_rect.left).max(200);
        let outer_h = (win_rect.bottom - win_rect.top).max(200);

        let (pos_x, pos_y) = compute_window_position(anchor, outer_w, outer_h);

        let class_name = to_wide("YumeRhythmConsoleWindowClass");
        let title = to_wide("桌宠节奏师 · 控制台");

        let mut state = Box::new(SharedWindowState::new(
            client_w.max(1) as u32,
            client_h.max(1) as u32,
            scale,
        ));
        let state_ptr = (&mut *state) as *mut SharedWindowState as isize;

        let hwnd = unsafe {
            let hinst = GetModuleHandleW(null());
            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                title.as_ptr(),
                style,
                pos_x,
                pos_y,
                outer_w,
                outer_h,
                null_mut(),
                null_mut(),
                hinst,
                null_mut(),
            );
            if hwnd.is_null() {
                return Err(format!("创建控制台窗口失败（Win32 错误码 {}）", GetLastError()));
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);
            DragAcceptFiles(hwnd, 1);

            // Sync actual DPI and client rect after creation
            let actual_dpi = GetDpiForWindow(hwnd);
            if actual_dpi >= 96 {
                state.scale_factor = actual_dpi as f32 / 96.0;
            }
            let mut rc: RECT = zeroed();
            if GetClientRect(hwnd, &mut rc) != 0 {
                state.client_width = (rc.right - rc.left).max(1) as u32;
                state.client_height = (rc.bottom - rc.top).max(1) as u32;
            }

            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            hwnd
        };

        Ok(Self { hwnd, state })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn bring_to_front(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(self.hwnd);
        }
    }

    pub fn client_size_px(&self) -> (u32, u32) {
        (self.state.client_width, self.state.client_height)
    }

    pub fn scale_factor(&self) -> f32 {
        self.state.scale_factor
    }

    /// Pumps any queued messages specifically targeting this console window.
    pub fn pump_messages(&mut self) {
        unsafe {
            let mut msg: MSG = zeroed();
            while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub fn close_requested(&self) -> bool {
        self.state.close_requested
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.state.dirty, false)
    }

    pub fn is_dirty(&self) -> bool {
        self.state.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.state.dirty = true;
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.state.pending_events)
    }

    pub fn take_dropped_audio_files(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.state.pending_dropped_audio)
    }

    pub fn take_shortcuts(&mut self) -> Vec<ConsoleShortcut> {
        std::mem::take(&mut self.state.pending_shortcuts)
    }

    pub fn set_cursor_icon(&mut self, icon: CursorIcon) {
        if self.state.cursor_icon != icon {
            self.state.cursor_icon = icon;
            unsafe {
                SetCursor(cursor_for_icon(icon));
            }
        }
    }

    /// Ensures the DIB matches current client size and invokes `draw_fn` to rasterize directly into it,
    /// then blits immediately to the window client DC.
    pub fn draw_and_present<F>(&mut self, draw_fn: F) -> Result<(), String>
    where
        F: FnOnce(&mut [u8], usize, usize, f32),
    {
        let scale = self.state.scale_factor;
        let dib = self.state.ensure_dib()?;
        let w = dib.width() as usize;
        let h = dib.height() as usize;
        draw_fn(dib.as_mut_slice(), w, h, scale);

        unsafe {
            let dc = GetDC(self.hwnd);
            if !dc.is_null() {
                dib.present_to_dc(dc);
                ReleaseDC(self.hwnd, dc);
            } else {
                InvalidateRect(self.hwnd, null(), 0);
            }
        }
        Ok(())
    }
}

impl Drop for NativeConsoleWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_null() {
                // Detach pointer BEFORE destroying HWND so WndProc never touches freed state.
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                DestroyWindow(self.hwnd);
                self.hwnd = null_mut();
            }
        }
        // Drop DIB explicitly (also happens when Box<SharedWindowState> drops).
        self.state.dib = None;
    }
}

fn register_window_class() -> Result<(), String> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| unsafe {
            let class_name = to_wide("YumeRhythmConsoleWindowClass");
            let hinst = GetModuleHandleW(null());
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(console_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: null_mut(),
            };
            if RegisterClassExW(&wc) == 0 {
                let err = GetLastError();
                // ERROR_CLASS_ALREADY_EXISTS = 1410
                if err != 1410 {
                    return Err(format!("注册控制台窗口类失败（错误码 {err}）"));
                }
            }
            Ok(())
        })
        .clone()
}

fn compute_window_position(anchor: AnchorRect, outer_w: i32, outer_h: i32) -> (i32, i32) {
    let ref_pt = if anchor.width > 0 && anchor.height > 0 {
        POINT {
            x: anchor.x + anchor.width / 2,
            y: anchor.y + anchor.height / 2,
        }
    } else {
        POINT { x: 0, y: 0 }
    };

    let work = unsafe {
        let mon = MonitorFromPoint(ref_pt, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if !mon.is_null() && GetMonitorInfoW(mon, &mut info) != 0 {
            info.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            }
        }
    };

    let gap = 16;
    let raw_x = if anchor.width > 0 {
        let right_candidate = anchor.x + anchor.width + gap;
        if right_candidate + outer_w <= work.right {
            right_candidate
        } else {
            anchor.x - outer_w - gap
        }
    } else {
        work.left + (work.right - work.left - outer_w) / 2
    };

    let raw_y = if anchor.height > 0 {
        anchor.y + (anchor.height - outer_h) / 2
    } else {
        work.top + (work.bottom - work.top - outer_h) / 2
    };

    let max_x = (work.right - outer_w).max(work.left);
    let max_y = (work.bottom - outer_h).max(work.top);
    (raw_x.clamp(work.left, max_x), raw_y.clamp(work.top, max_y))
}

unsafe extern "system" fn console_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SharedWindowState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state = &mut *ptr;

    match msg {
        WM_CLOSE => {
            state.close_requested = true;
            state.dirty = true;
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_null() {
                if let Some(dib) = &state.dib {
                    dib.present_to_dc(hdc);
                }
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_SIZE => {
            let w = (lparam & 0xFFFF) as u32;
            let h = ((lparam >> 16) & 0xFFFF) as u32;
            if w > 0 && h > 0 {
                state.client_width = w;
                state.client_height = h;
                state.dirty = true;
            }
            0
        }
        WM_GETMINMAXINFO => {
            let mmi = &mut *(lparam as *mut MINMAXINFO);
            mmi.ptMinTrackSize.x = ((CONSOLE_MIN_WIDTH as f32) * state.scale_factor).round() as i32;
            mmi.ptMinTrackSize.y = ((CONSOLE_MIN_HEIGHT as f32) * state.scale_factor).round() as i32;
            0
        }
        WM_DPICHANGED => {
            let new_dpi = (wparam & 0xFFFF) as u32;
            if new_dpi >= 96 {
                state.scale_factor = new_dpi as f32 / 96.0;
            }
            let suggested = &*(lparam as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            state.dirty = true;
            0
        }
        WM_SETCURSOR => {
            if (lparam & 0xFFFF) as u32 == HTCLIENT {
                SetCursor(cursor_for_icon(state.cursor_icon));
                return 1;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEMOVE => {
            if !state.tracking_mouse_leave {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme) != 0 {
                    state.tracking_mouse_leave = true;
                }
            }
            let pos = lparam_to_logical_pos(lparam, state.scale_factor);
            state.pending_events.push(Event::PointerMoved(pos));
            state.dirty = true;
            0
        }
        WM_MOUSELEAVE => {
            state.tracking_mouse_leave = false;
            state.pending_events.push(Event::PointerGone);
            state.dirty = true;
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            SetCapture(hwnd);
            push_pointer_button(state, lparam, PointerButton::Primary, true);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            push_pointer_button(state, lparam, PointerButton::Primary, false);
            0
        }
        WM_RBUTTONDOWN => {
            push_pointer_button(state, lparam, PointerButton::Secondary, true);
            0
        }
        WM_RBUTTONUP => {
            push_pointer_button(state, lparam, PointerButton::Secondary, false);
            0
        }
        WM_MBUTTONDOWN => {
            push_pointer_button(state, lparam, PointerButton::Middle, true);
            0
        }
        WM_MBUTTONUP => {
            push_pointer_button(state, lparam, PointerButton::Middle, false);
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as i16) as f32 / (WHEEL_DELTA as f32);
            state.pending_events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: Vec2::new(0.0, delta * 48.0),
                modifiers: current_modifiers(),
            });
            state.dirty = true;
            0
        }
        WM_MOUSEHWHEEL => {
            let delta = ((wparam >> 16) as i16) as f32 / (WHEEL_DELTA as f32);
            state.pending_events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: Vec2::new(-delta * 48.0, 0.0),
                modifiers: current_modifiers(),
            });
            state.dirty = true;
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = wparam as VIRTUAL_KEY;
            let repeat = (lparam & (1 << 30)) != 0;
            let modifiers = current_modifiers();

            if modifiers.ctrl && !modifiers.alt {
                match vk {
                    0x43 => {
                        state.pending_events.push(Event::Copy);
                        state.dirty = true;
                        return 0;
                    }
                    0x58 => {
                        state.pending_events.push(Event::Cut);
                        state.dirty = true;
                        return 0;
                    }
                    0x56 => {
                        if let Some(text) = read_clipboard_text(hwnd) {
                            state.pending_events.push(Event::Paste(text));
                            state.dirty = true;
                        }
                        return 0;
                    }
                    _ => {}
                }
            }

            if !repeat && !modifiers.ctrl && !modifiers.alt {
                match vk {
                    VK_SPACE => state.pending_shortcuts.push(ConsoleShortcut::ToggleAudio),
                    0x51 => state.pending_shortcuts.push(ConsoleShortcut::StopAudio), // Q
                    0x48 => state.pending_shortcuts.push(ConsoleShortcut::HideConsole), // H
                    _ => {}
                }
            }

            if let Some(key) = vk_to_egui_key(vk) {
                state.pending_events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat,
                    modifiers,
                });
                state.dirty = true;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KEYUP | WM_SYSKEYUP => {
            let vk = wparam as VIRTUAL_KEY;
            if let Some(key) = vk_to_egui_key(vk) {
                state.pending_events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: current_modifiers(),
                });
                state.dirty = true;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CHAR => {
            let unit = (wparam & 0xFFFF) as u16;
            // Handle UTF-16 surrogate pairs for emoji / supplementary CJK planes
            if (0xD800..=0xDBFF).contains(&unit) {
                state.high_surrogate = Some(unit);
                return 0;
            }
            let ch_opt = if (0xDC00..=0xDFFF).contains(&unit) {
                state
                    .high_surrogate
                    .take()
                    .and_then(|hi| char::decode_utf16([hi, unit]).next())
                    .and_then(Result::ok)
            } else {
                state.high_surrogate = None;
                char::from_u32(unit as u32)
            };

            if let Some(ch) = ch_opt {
                // Skip ASCII control characters (handled via WM_KEYDOWN)
                if !ch.is_control() {
                    state.pending_events.push(Event::Text(ch.to_string()));
                    state.dirty = true;
                }
            }
            0
        }
        WM_DROPFILES => {
            let hdrop = wparam as HDROP;
            let count = DragQueryFileW(hdrop, u32::MAX, null_mut(), 0);
            for idx in 0..count {
                let len = DragQueryFileW(hdrop, idx, null_mut(), 0);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    let written = DragQueryFileW(hdrop, idx, buf.as_mut_ptr(), len + 1);
                    if written > 0 {
                        let s = String::from_utf16_lossy(&buf[..written as usize]);
                        let path = PathBuf::from(s);
                        if is_supported_audio_file(&path) {
                            state.pending_dropped_audio.push(path);
                            state.dirty = true;
                        }
                    }
                }
            }
            DragFinish(hdrop);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn push_pointer_button(
    state: &mut SharedWindowState,
    lparam: LPARAM,
    button: PointerButton,
    pressed: bool,
) {
    let pos = lparam_to_logical_pos(lparam, state.scale_factor);
    state.pending_events.push(Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: current_modifiers(),
    });
    state.dirty = true;
}

fn lparam_to_logical_pos(lparam: LPARAM, scale_factor: f32) -> Pos2 {
    let x = (lparam & 0xFFFF) as i16 as f32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as f32;
    let inv = 1.0 / scale_factor.max(0.5);
    Pos2::new(x * inv, y * inv)
}

fn current_modifiers() -> Modifiers {
    unsafe {
        let ctrl = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
        let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
        let alt = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
        Modifiers {
            alt,
            ctrl,
            shift,
            mac_cmd: false,
            command: ctrl,
        }
    }
}

fn vk_to_egui_key(vk: VIRTUAL_KEY) -> Option<Key> {
    match vk {
        VK_BACK => Some(Key::Backspace),
        VK_TAB => Some(Key::Tab),
        VK_RETURN => Some(Key::Enter),
        VK_ESCAPE => Some(Key::Escape),
        VK_SPACE => Some(Key::Space),
        VK_PRIOR => Some(Key::PageUp),
        VK_NEXT => Some(Key::PageDown),
        VK_END => Some(Key::End),
        VK_HOME => Some(Key::Home),
        VK_LEFT => Some(Key::ArrowLeft),
        VK_UP => Some(Key::ArrowUp),
        VK_RIGHT => Some(Key::ArrowRight),
        VK_DOWN => Some(Key::ArrowDown),
        VK_DELETE => Some(Key::Delete),
        0x30 => Some(Key::Num0),
        0x31 => Some(Key::Num1),
        0x32 => Some(Key::Num2),
        0x33 => Some(Key::Num3),
        0x34 => Some(Key::Num4),
        0x35 => Some(Key::Num5),
        0x36 => Some(Key::Num6),
        0x37 => Some(Key::Num7),
        0x38 => Some(Key::Num8),
        0x39 => Some(Key::Num9),
        0x41 => Some(Key::A),
        0x42 => Some(Key::B),
        0x43 => Some(Key::C),
        0x44 => Some(Key::D),
        0x45 => Some(Key::E),
        0x46 => Some(Key::F),
        0x47 => Some(Key::G),
        0x48 => Some(Key::H),
        0x49 => Some(Key::I),
        0x4A => Some(Key::J),
        0x4B => Some(Key::K),
        0x4C => Some(Key::L),
        0x4D => Some(Key::M),
        0x4E => Some(Key::N),
        0x4F => Some(Key::O),
        0x50 => Some(Key::P),
        0x51 => Some(Key::Q),
        0x52 => Some(Key::R),
        0x53 => Some(Key::S),
        0x54 => Some(Key::T),
        0x55 => Some(Key::U),
        0x56 => Some(Key::V),
        0x57 => Some(Key::W),
        0x58 => Some(Key::X),
        0x59 => Some(Key::Y),
        0x5A => Some(Key::Z),
        _ => None,
    }
}

unsafe fn cursor_for_icon(icon: CursorIcon) -> HCURSOR {
    let id = match icon {
        CursorIcon::PointingHand => IDC_HAND,
        CursorIcon::Text => IDC_IBEAM,
        CursorIcon::ResizeHorizontal | CursorIcon::ResizeColumn => IDC_SIZEWE,
        CursorIcon::ResizeVertical | CursorIcon::ResizeRow => IDC_SIZENS,
        CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move => IDC_SIZEALL,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
        _ => IDC_ARROW,
    };
    LoadCursorW(null_mut(), id)
}

pub fn write_clipboard_text(hwnd: HWND, text: &str) {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return;
        }
        EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if !hmem.is_null() {
            let dst = GlobalLock(hmem) as *mut u16;
            if !dst.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
                GlobalUnlock(hmem);
                SetClipboardData(CF_UNICODETEXT, hmem);
            }
        }
        CloseClipboard();
    }
}

fn read_clipboard_text(hwnd: HWND) -> Option<String> {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        let result = if !handle.is_null() {
            let ptr = GlobalLock(handle) as *const u16;
            if !ptr.is_null() {
                let mut len = 0usize;
                while *ptr.add(len) != 0 && len < 65_536 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                let text = String::from_utf16_lossy(slice);
                GlobalUnlock(handle);
                Some(text)
            } else {
                None
            }
        } else {
            None
        };
        CloseClipboard();
        result
    }
}

/// Queries the current process private memory usage in MiB via Win32 `K32GetProcessMemoryInfo`.
pub fn query_process_private_mib() -> f32 {
    unsafe {
        let mut pmc: PROCESS_MEMORY_COUNTERS_EX = zeroed();
        pmc.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let ok = K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut pmc as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
            pmc.cb,
        );
        if ok != 0 {
            (pmc.PrivateUsage as f64 / (1024.0 * 1024.0)) as f32
        } else {
            0.0
        }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
