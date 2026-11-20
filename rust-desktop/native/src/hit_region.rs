//! Input-only alpha mask. Never use SetWindowRgn: it clips the compositor and
//! can expose white non-client edges or cut off the next animation frame.
use serde_json::Value;
use tao::{platform::windows::WindowExtWindows, window::Window};
use windows_sys::Win32::{
    Foundation::POINT,
    UI::WindowsAndMessaging::{GetCursorPos, GetWindowLongW, GWL_EXSTYLE, WS_EX_TRANSPARENT},
};

#[derive(Default)]
pub struct HitRegion {
    rows: Vec<Vec<(u32, u32)>>,
    width: u32,
    height: u32,
    pub rect_count: usize,
}

impl HitRegion {
    pub fn update(&mut self, window: &Window, request: &Value) -> Result<(), String> {
        let width = request["width"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 4096)
            .ok_or("无效窗口宽度")? as u32;
        let height = request["height"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 4096)
            .ok_or("无效窗口高度")? as u32;
        let rects = request["rects"]
            .as_array()
            .filter(|r| r.len() <= 32768)
            .ok_or("无效透明区域")?;
        let scale = window.scale_factor();
        let size = window.inner_size();
        if (size.width as f64 / scale - width as f64).abs() > 1.
            || (size.height as f64 / scale - height as f64).abs() > 1.
        {
            return Err("窗口尺寸已变化".into());
        }
        let mut rows = vec![Vec::new(); height as usize];
        for rect in rects {
            let values = rect
                .as_array()
                .filter(|r| r.len() == 4)
                .ok_or("无效命中矩形")?;
            let mut coords = [0u32; 4];
            for (index, value) in values.iter().enumerate() {
                coords[index] = value
                    .as_u64()
                    .filter(|n| *n <= 4096)
                    .ok_or("无效命中坐标")? as u32;
            }
            let [left, top, right, bottom] = coords;
            if left >= right || top >= bottom || right > width || bottom > height {
                return Err("命中矩形超出窗口".into());
            }
            for row in &mut rows[top as usize..bottom as usize] {
                row.push((left, right));
            }
        }
        self.rows = rows;
        self.width = width;
        self.height = height;
        self.rect_count = rects.len();
        Ok(())
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        if x < 0. || y < 0. || x >= self.width as f64 || y >= self.height as f64 {
            return false;
        }
        let x = x.floor() as u32;
        self.rows
            .get(y.floor() as usize)
            .is_some_and(|row| row.iter().any(|&(left, right)| left <= x && x < right))
    }

    /// Native cursor checks only; no browser round trips, screen readback or
    /// image copies. Update the OS style only when hit state actually changes.
    pub fn sync_pointer(&self, window: &Window, locked: bool) -> Result<(), String> {
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) } == 0 {
            return Ok(());
        }
        self.sync_point(window, locked, point)
    }

    pub fn sync_point(&self, window: &Window, locked: bool, point: POINT) -> Result<(), String> {
        let origin = window.inner_position().map_err(|e| e.to_string())?;
        let scale = window.scale_factor();
        let opaque = self.contains(
            (point.x - origin.x) as f64 / scale,
            (point.y - origin.y) as f64 / scale,
        );
        let ignore = locked || !opaque;
        let current = unsafe { GetWindowLongW(window.hwnd() as _, GWL_EXSTYLE) } as u32
            & WS_EX_TRANSPARENT
            != 0;
        if current != ignore {
            window
                .set_ignore_cursor_events(ignore)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
