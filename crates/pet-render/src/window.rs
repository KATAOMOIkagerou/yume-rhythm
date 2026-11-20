#[cfg(not(windows))]
use crate::{
    PetCompositor, PetRenderInput, PetWindowEvent, WindowRect, PET_LOGICAL_HEIGHT,
    PET_LOGICAL_WIDTH,
};

/// Clamps a rectangle of size `(width, height)` inside the work-area bounds `[left, right] x [top, bottom]`.
pub fn clamp_rect_to_bounds(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> (i32, i32) {
    let max_x = (right - width).max(left);
    let max_y = (bottom - height).max(top);
    (x.clamp(left, max_x), y.clamp(top, max_y))
}

#[cfg(windows)]
mod win32_impl {
    use super::clamp_rect_to_bounds;
    use crate::toolbar::{toolbar_hit_test, ToolbarButton, ToolbarState};
    use crate::{
        PetCompositor, PetRenderInput, PetWindowEvent, WindowRect, PET_LOGICAL_HEIGHT,
        PET_LOGICAL_WIDTH,
    };
    use pet_core::is_supported_audio_file;
    use std::cell::RefCell;
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::ptr::{null, null_mut};
    use std::sync::Once;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetMonitorInfoW,
        MonitorFromPoint, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, MONITORINFO,
        MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_SPACE,
    };
    use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
        GetWindowLongPtrW, LoadCursorW, PeekMessageW, RegisterClassExW, SetCursor,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage,
        UpdateLayeredWindow, CS_DBLCLKS, GWLP_USERDATA, GWL_EXSTYLE, HTCLIENT, HWND_TOPMOST,
        IDC_ARROW, IDC_HAND, MSG, PM_REMOVE, SPI_GETWORKAREA, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_CAPTURECHANGED,
        WM_CLOSE, WM_DPICHANGED, WM_DROPFILES, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDBLCLK,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST, WM_RBUTTONUP, WM_SETCURSOR,
        WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        WS_VISIBLE,
    };

    const WM_MOUSELEAVE: u32 = 0x02A3;

    const TOOLBAR_LINGER_MS: f32 = 1350.0;
    const TOOLBAR_FADE_MS: f32 = 250.0;

    static REGISTER_CLASS_ONCE: Once = Once::new();

    fn class_name_wide() -> Vec<u16> {
        "YumeRhythmPetWindowClass\0".encode_utf16().collect()
    }

    fn window_title_wide() -> Vec<u16> {
        "桌宠节奏师\0".encode_utf16().collect()
    }

    /// Persistent 32-bit top-down premultiplied BGRA DIBSection for `UpdateLayeredWindow`.
    struct LayeredSurface {
        mem_dc: HDC,
        bitmap: HBITMAP,
        old_obj: HGDIOBJ,
        bits: *mut u8,
        width: u32,
        height: u32,
    }

    impl LayeredSurface {
        fn new(width: u32, height: u32) -> Result<Self, String> {
            let width = width.max(1);
            let height = height.max(1);
            unsafe {
                let mem_dc = CreateCompatibleDC(null_mut());
                if mem_dc.is_null() {
                    return Err("创建分层窗口内存 DC 失败".into());
                }

                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width as i32,
                        biHeight: -(height as i32), // top-down DIB
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [std::mem::zeroed(); 1],
                };

                let mut bits: *mut c_void = null_mut();
                let bitmap = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
                if bitmap.is_null() || bits.is_null() {
                    DeleteDC(mem_dc);
                    return Err("创建 32 位预乘 BGRA DIBSection 失败".into());
                }

                let old_obj = SelectObject(mem_dc, bitmap as HGDIOBJ);
                std::ptr::write_bytes(bits as *mut u8, 0, (width as usize) * (height as usize) * 4);

                Ok(Self {
                    mem_dc,
                    bitmap,
                    old_obj,
                    bits: bits as *mut u8,
                    width,
                    height,
                })
            }
        }

        fn as_slice(&self) -> &[u8] {
            let len = (self.width as usize) * (self.height as usize) * 4;
            if self.bits.is_null() {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(self.bits, len) }
            }
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            let len = (self.width as usize) * (self.height as usize) * 4;
            if self.bits.is_null() {
                &mut []
            } else {
                unsafe { std::slice::from_raw_parts_mut(self.bits, len) }
            }
        }
    }

    impl Drop for LayeredSurface {
        fn drop(&mut self) {
            unsafe {
                if !self.mem_dc.is_null() {
                    if !self.old_obj.is_null() {
                        SelectObject(self.mem_dc, self.old_obj);
                    }
                    if !self.bitmap.is_null() {
                        DeleteObject(self.bitmap as HGDIOBJ);
                    }
                    DeleteDC(self.mem_dc);
                }
            }
        }
    }

    struct WindowInner {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        scale_factor: f32,
        locked: bool,
        tracking_mouse: bool,
        cursor_inside: bool,
        left_at: Option<Instant>,
        hovered_button: Option<ToolbarButton>,
        pressed_button: Option<ToolbarButton>,
        dragging: bool,
        drag_offset_x: i32,
        drag_offset_y: i32,
        needs_surface_resize: bool,
        events: Vec<PetWindowEvent>,
    }

    impl WindowInner {
        fn toolbar_opacity(&mut self) -> f32 {
            if self.locked {
                self.left_at = None;
                return 0.0;
            }
            if self.cursor_inside || self.pressed_button.is_some() {
                return 1.0;
            }
            if let Some(left) = self.left_at {
                let elapsed_ms = left.elapsed().as_secs_f32() * 1000.0;
                if elapsed_ms <= TOOLBAR_LINGER_MS {
                    return 1.0;
                }
                let fade = 1.0 - (elapsed_ms - TOOLBAR_LINGER_MS) / TOOLBAR_FADE_MS;
                if fade > 0.01 {
                    return fade.clamp(0.0, 1.0);
                }
                self.left_at = None;
            }
            0.0
        }

        fn toolbar_interactive(&self) -> bool {
            if self.locked {
                return false;
            }
            if self.cursor_inside || self.pressed_button.is_some() {
                return true;
            }
            if let Some(left) = self.left_at {
                let elapsed_ms = left.elapsed().as_secs_f32() * 1000.0;
                return elapsed_ms < TOOLBAR_LINGER_MS + TOOLBAR_FADE_MS;
            }
            false
        }
    }

    /// Native Win32 transparent, borderless, always-on-top layered pet window.
    pub struct PetWindow {
        hwnd: HWND,
        compositor: PetCompositor,
        surface: LayeredSurface,
        inner: Box<RefCell<WindowInner>>,
    }

    impl PetWindow {
        pub fn create() -> Result<Self, String> {
            unsafe {
                let hinstance = GetModuleHandleW(null());
                let class_name = class_name_wide();
                let title = window_title_wide();

                REGISTER_CLASS_ONCE.call_once(|| {
                    let wc = WNDCLASSEXW {
                        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                        style: CS_DBLCLKS,
                        lpfnWndProc: Some(pet_wnd_proc),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hInstance: hinstance,
                        hIcon: null_mut(),
                        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                        hbrBackground: null_mut(),
                        lpszMenuName: null(),
                        lpszClassName: class_name.as_ptr(),
                        hIconSm: null_mut(),
                    };
                    RegisterClassExW(&wc);
                });

                let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
                let style = WS_POPUP | WS_VISIBLE;

                let hwnd = CreateWindowExW(
                    ex_style,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    style,
                    0,
                    0,
                    PET_LOGICAL_WIDTH as i32,
                    PET_LOGICAL_HEIGHT as i32,
                    null_mut(),
                    null_mut(),
                    hinstance,
                    null(),
                );
                if hwnd.is_null() {
                    return Err("创建 Win32 分层桌宠窗口失败".into());
                }

                let dpi = GetDpiForWindow(hwnd);
                let scale_factor = if dpi > 0 {
                    (dpi as f32 / 96.0).clamp(0.75, 4.0)
                } else {
                    1.0
                };
                let width = ((PET_LOGICAL_WIDTH as f32) * scale_factor).round().max(1.0) as i32;
                let height = ((PET_LOGICAL_HEIGHT as f32) * scale_factor).round().max(1.0) as i32;

                let work = primary_work_area();
                let desired_x = work.right - width - ((36.0 * scale_factor).round() as i32);
                let desired_y = work.bottom - height - ((24.0 * scale_factor).round() as i32);
                let (x, y) = clamp_rect_to_bounds(
                    desired_x,
                    desired_y,
                    width,
                    height,
                    work.left,
                    work.top,
                    work.right,
                    work.bottom,
                );

                let compositor = PetCompositor::new(width as u32, height as u32);
                let surface = match LayeredSurface::new(width as u32, height as u32) {
                    Ok(s) => s,
                    Err(err) => {
                        DestroyWindow(hwnd);
                        return Err(err);
                    }
                };

                let inner = Box::new(RefCell::new(WindowInner {
                    x,
                    y,
                    width,
                    height,
                    scale_factor,
                    locked: false,
                    tracking_mouse: false,
                    cursor_inside: false,
                    left_at: None,
                    hovered_button: None,
                    pressed_button: None,
                    dragging: false,
                    drag_offset_x: 0,
                    drag_offset_y: 0,
                    needs_surface_resize: false,
                    events: Vec::new(),
                }));

                let raw_inner: *const RefCell<WindowInner> = &*inner;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw_inner as isize);

                DragAcceptFiles(hwnd, 1);

                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );

                Ok(Self {
                    hwnd,
                    compositor,
                    surface,
                    inner,
                })
            }
        }

        pub fn hwnd(&self) -> isize {
            self.hwnd as isize
        }

        pub fn set_locked(&mut self, locked: bool) -> Result<(), String> {
            let was_dragging;
            {
                let mut st = self.inner.borrow_mut();
                st.locked = locked;
                was_dragging = st.dragging;
                if locked {
                    st.dragging = false;
                    st.cursor_inside = false;
                    st.left_at = None;
                    st.hovered_button = None;
                    st.pressed_button = None;
                }
            }

            unsafe {
                if locked && was_dragging {
                    ReleaseCapture();
                }
                let mut ex_style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
                if locked {
                    ex_style |= WS_EX_TRANSPARENT as isize;
                } else {
                    ex_style &= !(WS_EX_TRANSPARENT as isize);
                }
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex_style);
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
            Ok(())
        }

        pub fn is_locked(&self) -> bool {
            self.inner.borrow().locked
        }

        pub fn invalidate_cache(&mut self) {
            self.compositor.invalidate_static_cache();
        }

        pub fn last_render_ms(&self) -> f64 {
            self.compositor.last_render_ms()
        }

        pub fn last_frame_bgra(&self) -> (&[u8], u32, u32) {
            (self.surface.as_slice(), self.surface.width, self.surface.height)
        }

        pub fn set_position(&mut self, x: i32, y: i32) {
            let (cx, cy, w, h) = {
                let mut st = self.inner.borrow_mut();
                let (nx, ny) = clamp_to_nearest_monitor_work_area(x, y, st.width, st.height);
                st.x = nx;
                st.y = ny;
                (nx, ny, st.width, st.height)
            };
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    cx,
                    cy,
                    w,
                    h,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }

        pub fn show(&mut self) {
            unsafe {
                ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }

        pub fn hide(&mut self) {
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
            }
        }

        pub fn present(&mut self, input: &PetRenderInput<'_>) -> Result<(), String> {
            let (x, y, width, height, needs_resize, toolbar_state) = {
                let mut st = self.inner.borrow_mut();
                if st.locked != input.pet_locked {
                    // Keep internal hover/toolbar consistent with input lock state
                    st.locked = input.pet_locked;
                    if input.pet_locked {
                        st.hovered_button = None;
                        st.pressed_button = None;
                        st.left_at = None;
                    }
                }
                let opacity = st.toolbar_opacity();
                let tb = ToolbarState {
                    opacity,
                    hovered: st.hovered_button,
                    pressed: st.pressed_button,
                };
                let nr = st.needs_surface_resize;
                st.needs_surface_resize = false;
                (st.x, st.y, st.width, st.height, nr, tb)
            };

            let target_w = width.max(1) as u32;
            let target_h = height.max(1) as u32;
            if needs_resize
                || self.surface.width != target_w
                || self.surface.height != target_h
            {
                self.compositor.resize(target_w, target_h);
                self.surface = LayeredSurface::new(target_w, target_h)?;
            }

            self.compositor.set_toolbar_state(toolbar_state);
            self.compositor
                .render_frame_into(input, self.surface.as_mut_slice());

            unsafe {
                let pt_dst = POINT { x, y };
                let size = SIZE {
                    cx: target_w as i32,
                    cy: target_h as i32,
                };
                let pt_src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };

                let ok = UpdateLayeredWindow(
                    self.hwnd,
                    null_mut(),
                    &pt_dst,
                    &size,
                    self.surface.mem_dc,
                    &pt_src,
                    0,
                    &blend,
                    ULW_ALPHA,
                );
                if ok == 0 {
                    return Err("UpdateLayeredWindow 呈现帧失败".into());
                }
            }
            Ok(())
        }

        pub fn poll_events(&mut self) -> Vec<PetWindowEvent> {
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::mem::take(&mut self.inner.borrow_mut().events)
        }

        pub fn window_rect(&self) -> WindowRect {
            let st = self.inner.borrow();
            WindowRect {
                x: st.x,
                y: st.y,
                width: st.width,
                height: st.height,
                scale_factor: st.scale_factor,
            }
        }

        /// Returns the current pet window screen rect & DPI scale so `pet-ui` can anchor the console cleanly beside it.
        pub fn anchor_rect(&self) -> WindowRect {
            self.window_rect()
        }
    }

    impl Drop for PetWindow {
        fn drop(&mut self) {
            unsafe {
                if !self.hwnd.is_null() {
                    // Detach user data pointer BEFORE DestroyWindow so WndProc never sees freed state.
                    SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                    DestroyWindow(self.hwnd);
                    self.hwnd = null_mut();
                }
            }
        }
    }

    fn primary_work_area() -> RECT {
        unsafe {
            let mut rc: RECT = std::mem::zeroed();
            if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rc as *mut RECT as *mut c_void, 0)
                != 0
            {
                return rc;
            }
        }
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        }
    }

    fn clamp_to_nearest_monitor_work_area(x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
        let center = POINT {
            x: x + width / 2,
            y: y + height / 2,
        };
        let work = unsafe {
            let hmon = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
            if !hmon.is_null() {
                let mut mi: MONITORINFO = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(hmon, &mut mi) != 0 {
                    mi.rcWork
                } else {
                    primary_work_area()
                }
            } else {
                primary_work_area()
            }
        };
        clamp_rect_to_bounds(
            x,
            y,
            width,
            height,
            work.left,
            work.top,
            work.right,
            work.bottom,
        )
    }

    #[inline]
    fn lparam_client_xy(lparam: LPARAM) -> (i32, i32) {
        let x = (lparam as u32 & 0xFFFF) as u16 as i16 as i32;
        let y = ((lparam as u32 >> 16) & 0xFFFF) as u16 as i16 as i32;
        (x, y)
    }

    unsafe extern "system" fn pet_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<WindowInner>;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let cell = &*ptr;

        match msg {
            WM_NCHITTEST => HTCLIENT as LRESULT,
            WM_ERASEBKGND => 1,
            WM_MOUSEMOVE => {
                let (cx, cy) = lparam_client_xy(lparam);
                let mut drag_move: Option<(i32, i32)> = None;
                let mut need_track = false;

                if let Ok(mut st) = cell.try_borrow_mut() {
                    if !st.tracking_mouse {
                        st.tracking_mouse = true;
                        need_track = true;
                    }
                    st.cursor_inside = true;
                    st.left_at = None;

                    if st.dragging && !st.locked {
                        let mut pt = POINT { x: 0, y: 0 };
                        if GetCursorPos(&mut pt) != 0 {
                            let raw_x = pt.x - st.drag_offset_x;
                            let raw_y = pt.y - st.drag_offset_y;
                            let (nx, ny) = clamp_to_nearest_monitor_work_area(
                                raw_x, raw_y, st.width, st.height,
                            );
                            if nx != st.x || ny != st.y {
                                st.x = nx;
                                st.y = ny;
                                drag_move = Some((nx, ny));
                            }
                        }
                    } else if !st.locked {
                        let inv_scale = 1.0 / st.scale_factor.max(0.5);
                        let lx = (cx as f32) * inv_scale;
                        let ly = (cy as f32) * inv_scale;
                        st.hovered_button = toolbar_hit_test(lx, ly);
                    } else {
                        st.hovered_button = None;
                    }
                }

                if need_track {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut tme);
                }

                if let Some((nx, ny)) = drag_move {
                    SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        nx,
                        ny,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                0
            }
            WM_MOUSELEAVE => {
                if let Ok(mut st) = cell.try_borrow_mut() {
                    st.tracking_mouse = false;
                    st.cursor_inside = false;
                    st.hovered_button = None;
                    if !st.locked {
                        st.left_at = Some(Instant::now());
                    }
                }
                0
            }
            WM_SETCURSOR => {
                if (lparam as u32 & 0xFFFF) == HTCLIENT {
                    let is_btn = cell
                        .try_borrow()
                        .map(|st| !st.locked && st.hovered_button.is_some())
                        .unwrap_or(false);
                    let cursor_id = if is_btn { IDC_HAND } else { IDC_ARROW };
                    SetCursor(LoadCursorW(null_mut(), cursor_id));
                    return 1;
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_LBUTTONDOWN => {
                let (cx, cy) = lparam_client_xy(lparam);
                let mut should_capture = false;

                if let Ok(mut st) = cell.try_borrow_mut() {
                    if !st.locked {
                        st.cursor_inside = true;
                        st.left_at = None;
                        let inv_scale = 1.0 / st.scale_factor.max(0.5);
                        let lx = (cx as f32) * inv_scale;
                        let ly = (cy as f32) * inv_scale;
                        let hit = if st.toolbar_interactive() {
                            toolbar_hit_test(lx, ly)
                        } else {
                            None
                        };

                        if let Some(btn) = hit {
                            st.hovered_button = Some(btn);
                            st.pressed_button = Some(btn);
                            st.dragging = false;
                            should_capture = true;
                        } else {
                            let mut pt = POINT { x: 0, y: 0 };
                            if GetCursorPos(&mut pt) != 0 {
                                st.dragging = true;
                                st.drag_offset_x = pt.x - st.x;
                                st.drag_offset_y = pt.y - st.y;
                                should_capture = true;
                            }
                        }
                    }
                }

                if should_capture {
                    SetCapture(hwnd);
                }
                0
            }
            WM_LBUTTONUP => {
                let (cx, cy) = lparam_client_xy(lparam);
                let mut should_release = false;

                if let Ok(mut st) = cell.try_borrow_mut() {
                    if let Some(pressed) = st.pressed_button.take() {
                        should_release = true;
                        let inv_scale = 1.0 / st.scale_factor.max(0.5);
                        let lx = (cx as f32) * inv_scale;
                        let ly = (cy as f32) * inv_scale;
                        if toolbar_hit_test(lx, ly) == Some(pressed) && !st.locked {
                            match pressed {
                                ToolbarButton::Console => {
                                    st.events.push(PetWindowEvent::OpenConsoleClicked);
                                }
                                ToolbarButton::Lock => {
                                    st.events.push(PetWindowEvent::ToggleLockClicked);
                                }
                                ToolbarButton::AudioSync => {
                                    st.events.push(PetWindowEvent::ToggleAudioClicked);
                                }
                            }
                        }
                    }
                    if st.dragging {
                        st.dragging = false;
                        should_release = true;
                    }
                }

                if should_release {
                    ReleaseCapture();
                }
                0
            }
            WM_CAPTURECHANGED => {
                if let Ok(mut st) = cell.try_borrow_mut() {
                    st.dragging = false;
                    st.pressed_button = None;
                }
                0
            }
            WM_LBUTTONDBLCLK => {
                let (cx, cy) = lparam_client_xy(lparam);
                if let Ok(mut st) = cell.try_borrow_mut() {
                    if !st.locked {
                        let inv_scale = 1.0 / st.scale_factor.max(0.5);
                        let lx = (cx as f32) * inv_scale;
                        let ly = (cy as f32) * inv_scale;
                        if toolbar_hit_test(lx, ly).is_none() {
                            st.dragging = false;
                            st.events.push(PetWindowEvent::OpenConsoleClicked);
                        }
                    }
                }
                0
            }
            WM_RBUTTONUP => {
                if let Ok(mut st) = cell.try_borrow_mut() {
                    if !st.locked {
                        let mut pt = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut pt);
                        st.events.push(PetWindowEvent::RightClicked {
                            screen_x: pt.x,
                            screen_y: pt.y,
                        });
                    }
                }
                0
            }
            WM_KEYDOWN => {
                if let Ok(mut st) = cell.try_borrow_mut() {
                    let vk = wparam as u16;
                    if vk == VK_SPACE {
                        st.events.push(PetWindowEvent::KeySpace);
                    } else if vk == b'Q' as u16 {
                        st.events.push(PetWindowEvent::KeyQ);
                    } else if vk == b'H' as u16 {
                        st.events.push(PetWindowEvent::KeyH);
                    }
                }
                0
            }
            WM_DROPFILES => {
                let hdrop = wparam as HDROP;
                let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, null_mut(), 0);
                let mut dropped = Vec::new();
                for idx in 0..count {
                    let len = DragQueryFileW(hdrop, idx, null_mut(), 0);
                    if len > 0 {
                        let mut buf = vec![0u16; (len as usize) + 1];
                        let copied = DragQueryFileW(hdrop, idx, buf.as_mut_ptr(), buf.len() as u32);
                        if copied > 0 {
                            let os = OsString::from_wide(&buf[..copied as usize]);
                            let path = PathBuf::from(os);
                            if is_supported_audio_file(&path) {
                                dropped.push(path);
                            }
                        }
                    }
                }
                DragFinish(hdrop);

                if !dropped.is_empty() {
                    if let Ok(mut st) = cell.try_borrow_mut() {
                        for path in dropped {
                            st.events.push(PetWindowEvent::DroppedAudioFile(path));
                        }
                    }
                }
                0
            }
            WM_DPICHANGED => {
                let new_dpi = (wparam & 0xFFFF) as u32;
                let new_scale = if new_dpi > 0 {
                    (new_dpi as f32 / 96.0).clamp(0.75, 4.0)
                } else {
                    1.0
                };
                let new_w = ((PET_LOGICAL_WIDTH as f32) * new_scale).round().max(1.0) as i32;
                let new_h = ((PET_LOGICAL_HEIGHT as f32) * new_scale).round().max(1.0) as i32;
                let suggested = lparam as *const RECT;
                let (sx, sy) = if !suggested.is_null() {
                    ((*suggested).left, (*suggested).top)
                } else {
                    (120, 120)
                };
                let (cx, cy) = clamp_to_nearest_monitor_work_area(sx, sy, new_w, new_h);

                if let Ok(mut st) = cell.try_borrow_mut() {
                    st.scale_factor = new_scale;
                    st.width = new_w;
                    st.height = new_h;
                    st.x = cx;
                    st.y = cy;
                    st.needs_surface_resize = true;
                }

                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    cx,
                    cy,
                    new_w,
                    new_h,
                    SWP_NOACTIVATE,
                );
                0
            }
            WM_CLOSE => {
                if let Ok(mut st) = cell.try_borrow_mut() {
                    st.events.push(PetWindowEvent::CloseRequested);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(windows)]
pub use win32_impl::PetWindow;

#[cfg(not(windows))]
pub struct PetWindow {
    locked: bool,
    compositor: PetCompositor,
}

#[cfg(not(windows))]
impl PetWindow {
    pub fn create() -> Result<Self, String> {
        Ok(Self {
            locked: false,
            compositor: PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT),
        })
    }

    pub fn hwnd(&self) -> isize {
        0
    }

    pub fn set_locked(&mut self, locked: bool) -> Result<(), String> {
        self.locked = locked;
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn invalidate_cache(&mut self) {
        self.compositor.invalidate_static_cache();
    }

    pub fn last_render_ms(&self) -> f64 {
        self.compositor.last_render_ms()
    }

    pub fn last_frame_bgra(&self) -> (&[u8], u32, u32) {
        let (w, h) = self.compositor.dimensions();
        (self.compositor.bgra_buffer(), w, h)
    }

    pub fn set_position(&mut self, _x: i32, _y: i32) {}

    pub fn show(&mut self) {}

    pub fn hide(&mut self) {}

    pub fn present(&mut self, input: &PetRenderInput<'_>) -> Result<(), String> {
        let _ = self.compositor.render_frame(input);
        Ok(())
    }

    pub fn poll_events(&mut self) -> Vec<PetWindowEvent> {
        Vec::new()
    }

    pub fn window_rect(&self) -> WindowRect {
        WindowRect {
            x: 120,
            y: 120,
            width: PET_LOGICAL_WIDTH as i32,
            height: PET_LOGICAL_HEIGHT as i32,
            scale_factor: 1.0,
        }
    }

    pub fn anchor_rect(&self) -> WindowRect {
        self.window_rect()
    }
}
