//! 桌宠窗口：Win32 分层窗口，用 `UpdateLayeredWindow` 提交逐像素 alpha 画面。
//!
//! 系统按像素做命中测试：alpha 为 0 的像素点击会穿透到下面的窗口，半透明边缘照常响应。
//! 不需要自己计算点击掩码。
//!
//! 窗口不抢焦点（`WS_EX_NOACTIVATE`），不在任务栏和 Alt+Tab 中出现（`WS_EX_TOOLWINDOW`）。
//! 拖动自己实现，不进入系统的模态移动循环，所以拖动时动画不会停。
//! 本类型只能在创建它的线程上使用。

use crate::surface::Surface;
use std::cell::RefCell;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging as wm;
use windows::core::{HSTRING, PCWSTR, Result, w};

const CLASS_NAME: PCWSTR = w!("YunMengZhuoBanPet");
/// 第二个实例启动时广播这条消息，请已运行的实例显示控制台。
pub const ACTIVATE_MESSAGE: PCWSTR = w!("YunMengZhuoBan.Activate");
/// 按下后移动超过这么多像素才算拖动，否则算点击。
const DRAG_THRESHOLD: i32 = 3;

/// 窗口事件，由 `PetWindow::take_events` 取出。坐标均为虚拟桌面物理像素。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PetEvent {
    /// 拖动结束，窗口左上角的新位置。
    Moved {
        x: i32,
        y: i32,
    },
    Clicked,
    /// 右键，光标位置。
    ContextMenu {
        x: i32,
        y: i32,
    },
    FilesDropped(Vec<String>),
    /// 另一个实例请求激活。
    ActivateRequested,
}

#[derive(Default)]
struct Drag {
    active: bool,
    moved: bool,
    cursor: POINT,
    window: POINT,
}

struct Shared {
    events: Vec<PetEvent>,
    drag: Drag,
    activate_message: u32,
}

pub struct PetWindow {
    hwnd: HWND,
    shared: Box<RefCell<Shared>>,
    surface: Option<Surface>,
    visible: bool,
    shown: bool,
    opacity: u8,
}

impl PetWindow {
    /// 创建窗口，左上角位于 `(x, y)`。第一次 `present` 之后才显示。
    ///
    /// # Errors
    /// 系统拒绝创建窗口时返回错误。
    pub fn create(title: &str, x: i32, y: i32) -> Result<Self> {
        // SAFETY: 以下均为标准的 Win32 窗口创建调用，参数都是有效值；
        // `shared` 装箱后地址固定，并且活得比窗口久（见 Drop）。
        unsafe {
            let instance = GetModuleHandleW(None)?;
            register_class(instance.into())?;
            let ex_style =
                wm::WS_EX_LAYERED | wm::WS_EX_TOOLWINDOW | wm::WS_EX_TOPMOST | wm::WS_EX_NOACTIVATE;
            let hwnd = wm::CreateWindowExW(
                ex_style,
                CLASS_NAME,
                &HSTRING::from(title),
                wm::WS_POPUP,
                x,
                y,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let activate_message = wm::RegisterWindowMessageW(ACTIVATE_MESSAGE);
            let shared = Box::new(RefCell::new(Shared {
                events: Vec::new(),
                drag: Drag::default(),
                activate_message,
            }));
            let ptr: *const RefCell<Shared> = &*shared;
            wm::SetWindowLongPtrW(hwnd, wm::GWLP_USERDATA, ptr as isize);
            DragAcceptFiles(hwnd, true);
            Ok(Self {
                hwnd,
                shared,
                surface: None,
                visible: true,
                shown: false,
                opacity: 255,
            })
        }
    }

    /// 提交一帧画面（预乘 BGRA）。尺寸变化时保持窗口底边中点不动。
    ///
    /// # Errors
    /// 创建位图或更新分层窗口失败。
    pub fn present(&mut self, bgra: &[u8], width: u32, height: u32) -> Result<()> {
        let (w, h) = (to_i32(width), to_i32(height));
        let mut position = None;
        if self.surface.as_ref().is_none_or(|s| s.size() != (w, h)) {
            if let Some(old) = &self.surface {
                let rect = self.rect();
                let (ow, oh) = old.size();
                position = Some(POINT {
                    x: rect.left + (ow - w) / 2,
                    y: rect.top + (oh - h),
                });
            }
            self.surface = Some(Surface::new(w, h)?);
        }
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        surface.write(bgra);
        let size = SIZE { cx: w, cy: h };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: self.opacity,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: 窗口与内存 DC 都有效；所有指针指向本函数内的局部变量。
        unsafe {
            wm::UpdateLayeredWindow(
                self.hwnd,
                None,
                position.as_ref().map(std::ptr::from_ref),
                Some(&raw const size),
                Some(surface.dc()),
                Some(&raw const origin),
                COLORREF(0),
                Some(&raw const blend),
                wm::ULW_ALPHA,
            )?;
        }
        if !self.shown && self.visible {
            self.show(true);
        }
        Ok(())
    }

    /// 锁定：整窗点击穿透。
    pub fn set_click_through(&self, on: bool) {
        // SAFETY: 读写本窗口的扩展样式。
        unsafe {
            let ex = wm::GetWindowLongPtrW(self.hwnd, wm::GWL_EXSTYLE);
            let flag = wm::WS_EX_TRANSPARENT.0 as isize;
            let next = if on { ex | flag } else { ex & !flag };
            wm::SetWindowLongPtrW(self.hwnd, wm::GWL_EXSTYLE, next);
        }
        if on {
            self.shared.borrow_mut().drag = Drag::default();
        }
    }

    /// 整体透明度（0–255），与逐像素 alpha 相乘。
    pub fn set_opacity(&mut self, alpha: u8) {
        if self.opacity == alpha {
            return;
        }
        self.opacity = alpha;
        let Some(surface) = &self.surface else {
            return;
        };
        let (w, h) = surface.size();
        let size = SIZE { cx: w, cy: h };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: self.opacity,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: 窗口与内存 DC 均有效，所有指针指向本函数局部变量。
        unsafe {
            let _ = wm::UpdateLayeredWindow(
                self.hwnd,
                None,
                None,
                Some(&raw const size),
                Some(surface.dc()),
                Some(&raw const origin),
                COLORREF(0),
                Some(&raw const blend),
                wm::ULW_ALPHA,
            );
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if self.surface.is_some() {
            self.show(visible);
        }
    }

    /// 重新声明置顶而不激活窗口，防止显示桌面或关闭全部窗口时被 `WorkerW` 遮挡。
    pub fn ensure_topmost(&self) {
        if !self.shown || !self.visible {
            return;
        }
        let flags = wm::SWP_NOMOVE | wm::SWP_NOSIZE | wm::SWP_NOACTIVATE;
        // SAFETY: 窗口有效，仅维护 TOPMOST Z 序，不改变焦点、尺寸或位置。
        unsafe {
            let _ = wm::SetWindowPos(self.hwnd, Some(wm::HWND_TOPMOST), 0, 0, 0, 0, flags);
        }
    }

    fn show(&mut self, visible: bool) {
        let cmd = if visible {
            wm::SW_SHOWNOACTIVATE
        } else {
            wm::SW_HIDE
        };
        // SAFETY: 窗口有效。返回值只表示之前是否可见，无需处理。
        unsafe {
            let _ = wm::ShowWindow(self.hwnd, cmd);
        }
        self.shown = visible;
    }

    pub fn set_position(&self, x: i32, y: i32) {
        let flags = wm::SWP_NOSIZE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE;
        // SAFETY: 窗口有效。移动失败无副作用，忽略。
        unsafe {
            let _ = wm::SetWindowPos(self.hwnd, None, x, y, 0, 0, flags);
        }
    }

    /// 窗口左上角（物理像素）。
    #[must_use]
    pub fn position(&self) -> (i32, i32) {
        let rect = self.rect();
        (rect.left, rect.top)
    }

    /// 所在显示器的缩放比例（1.0 = 96 DPI）。
    #[must_use]
    pub fn dpi_scale(&self) -> f32 {
        // SAFETY: 窗口有效。
        let dpi = unsafe { GetDpiForWindow(self.hwnd) };
        #[allow(clippy::cast_precision_loss, reason = "DPI 很小")]
        if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
    }

    /// 取出自上次调用以来的事件。
    pub fn take_events(&self) -> Vec<PetEvent> {
        std::mem::take(&mut self.shared.borrow_mut().events)
    }

    /// 在光标处弹出菜单，返回选中项的 id。
    #[must_use]
    pub fn show_menu(&self, items: &[crate::MenuItem], x: i32, y: i32) -> Option<u32> {
        crate::menu::track(self.hwnd, items, x, y)
    }

    fn rect(&self) -> RECT {
        let mut rect = RECT::default();
        // SAFETY: 窗口有效，`rect` 是局部变量。
        unsafe {
            let _ = wm::GetWindowRect(self.hwnd, &raw mut rect);
        }
        rect
    }
}

impl Drop for PetWindow {
    fn drop(&mut self) {
        // SAFETY: 先销毁窗口（期间窗口过程仍可访问 `shared`），再释放其余字段。
        unsafe {
            wm::SetWindowLongPtrW(self.hwnd, wm::GWLP_USERDATA, 0);
            let _ = wm::DestroyWindow(self.hwnd);
        }
    }
}

fn register_class(instance: windows::Win32::Foundation::HINSTANCE) -> Result<()> {
    static REGISTER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let ok = *REGISTER.get_or_init(|| {
        // SAFETY: 结构体字段均为有效值，类名是静态字符串。
        unsafe {
            let class = wm::WNDCLASSEXW {
                cbSize: u32::try_from(std::mem::size_of::<wm::WNDCLASSEXW>()).unwrap_or(0),
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: wm::LoadCursorW(None, wm::IDC_HAND).unwrap_or_default(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            wm::RegisterClassExW(&raw const class) != 0
        }
    });
    if ok {
        Ok(())
    } else {
        Err(windows::core::Error::from_thread())
    }
}

fn cursor() -> POINT {
    let mut p = POINT::default();
    // SAFETY: `p` 是局部变量。
    unsafe {
        let _ = wm::GetCursorPos(&raw mut p);
    }
    p
}

/// 窗口过程。注意：调用可能重入窗口过程的 Win32 函数前，必须先释放 `shared` 的借用。
extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA 由 `create` 写入，指向与窗口同生命周期的 `RefCell<Shared>`；为 0 时不使用。
    let shared = unsafe {
        (wm::GetWindowLongPtrW(hwnd, wm::GWLP_USERDATA) as *const RefCell<Shared>).as_ref()
    };
    let Some(shared) = shared else {
        // SAFETY: 标准默认处理。
        return unsafe { wm::DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    match msg {
        wm::WM_MOUSEACTIVATE => LRESULT(wm::MA_NOACTIVATE as isize),
        wm::WM_LBUTTONDOWN => {
            let mut window = RECT::default();
            // SAFETY: 窗口有效，`window` 是局部变量。
            unsafe {
                let _ = wm::GetWindowRect(hwnd, &raw mut window);
            }
            shared.borrow_mut().drag = Drag {
                active: true,
                moved: false,
                cursor: cursor(),
                window: POINT {
                    x: window.left,
                    y: window.top,
                },
            };
            // SAFETY: 窗口有效；借用已释放。
            unsafe {
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        wm::WM_MOUSEMOVE => {
            let target = drag_target(&mut shared.borrow_mut().drag);
            if let Some((x, y)) = target {
                let flags = wm::SWP_NOSIZE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE;
                // SAFETY: 窗口有效；借用已释放。
                unsafe {
                    let _ = wm::SetWindowPos(hwnd, None, x, y, 0, 0, flags);
                }
            }
            LRESULT(0)
        }
        wm::WM_LBUTTONUP => {
            finish_drag(shared, hwnd, true);
            // SAFETY: 释放捕获；借用已释放。
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        wm::WM_CAPTURECHANGED => {
            finish_drag(shared, hwnd, false);
            LRESULT(0)
        }
        wm::WM_RBUTTONUP => {
            let p = cursor();
            shared
                .borrow_mut()
                .events
                .push(PetEvent::ContextMenu { x: p.x, y: p.y });
            LRESULT(0)
        }
        wm::WM_DROPFILES => {
            let files = dropped_files(HDROP(wparam.0 as *mut _));
            shared
                .borrow_mut()
                .events
                .push(PetEvent::FilesDropped(files));
            LRESULT(0)
        }
        _ if msg == shared.borrow().activate_message && msg != 0 => {
            shared.borrow_mut().events.push(PetEvent::ActivateRequested);
            LRESULT(0)
        }
        // SAFETY: 标准默认处理。
        _ => unsafe { wm::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn drag_target(drag: &mut Drag) -> Option<(i32, i32)> {
    if !drag.active {
        return None;
    }
    let now = cursor();
    let (dx, dy) = (now.x - drag.cursor.x, now.y - drag.cursor.y);
    if !drag.moved && dx.abs() < DRAG_THRESHOLD && dy.abs() < DRAG_THRESHOLD {
        return None;
    }
    drag.moved = true;
    Some((drag.window.x + dx, drag.window.y + dy))
}

fn finish_drag(shared: &RefCell<Shared>, hwnd: HWND, released: bool) {
    let mut s = shared.borrow_mut();
    if !s.drag.active {
        return;
    }
    let moved = s.drag.moved;
    s.drag = Drag::default();
    if moved {
        let mut rect = RECT::default();
        // SAFETY: 窗口有效，`rect` 是局部变量；GetWindowRect 不会重入窗口过程。
        unsafe {
            let _ = wm::GetWindowRect(hwnd, &raw mut rect);
        }
        s.events.push(PetEvent::Moved {
            x: rect.left,
            y: rect.top,
        });
    } else if released {
        s.events.push(PetEvent::Clicked);
    }
}

fn dropped_files(drop: HDROP) -> Vec<String> {
    let mut files = Vec::new();
    // SAFETY: `drop` 来自 WM_DROPFILES，使用完调用 DragFinish 释放。
    unsafe {
        let count = DragQueryFileW(drop, u32::MAX, None);
        for i in 0..count {
            let len = DragQueryFileW(drop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            let written = DragQueryFileW(drop, i, Some(&mut buf)) as usize;
            files.push(String::from_utf16_lossy(&buf[..written.min(len)]));
        }
        DragFinish(drop);
    }
    files
}

fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}
