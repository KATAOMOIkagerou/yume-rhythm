use std::collections::HashMap;
use tiny_skia::{
    Color, FillRule, FilterQuality, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke,
    Transform,
};

use crate::PET_LOGICAL_WIDTH;

/// Interactive pill buttons in the floating top toolbar shown on hover when unlocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolbarButton {
    Console,
    Lock,
    AudioSync,
}

/// Logical hit-test rectangle (`540x590` coordinate space) for a toolbar pill button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarButtonRect {
    pub button: ToolbarButton,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ToolbarButtonRect {
    pub fn contains(&self, lx: f32, ly: f32) -> bool {
        lx >= self.x && lx <= self.x + self.width && ly >= self.y && ly <= self.y + self.height
    }
}

/// Visual interaction state for the floating hover toolbar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarState {
    /// Overall toolbar opacity (`0.0` = hidden, `1.0` = fully visible).
    pub opacity: f32,
    pub hovered: Option<ToolbarButton>,
    pub pressed: Option<ToolbarButton>,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            opacity: 0.0,
            hovered: None,
            pressed: None,
        }
    }
}

const TOOLBAR_TOP_Y: f32 = 10.0;
const TOOLBAR_HEIGHT: f32 = 30.0;
const TOOLBAR_GAP: f32 = 8.0;
const BTN_CONSOLE_W: f32 = 90.0;
const BTN_LOCK_W: f32 = 76.0;
const BTN_AUDIO_W: f32 = 106.0;

/// Computes the 3 centered pastel pill button rects in logical `540x590` coordinates.
pub fn toolbar_layout() -> [ToolbarButtonRect; 3] {
    let total_w = BTN_CONSOLE_W + BTN_LOCK_W + BTN_AUDIO_W + TOOLBAR_GAP * 2.0;
    let start_x = ((PET_LOGICAL_WIDTH as f32) - total_w) * 0.5;
    let x0 = start_x;
    let x1 = x0 + BTN_CONSOLE_W + TOOLBAR_GAP;
    let x2 = x1 + BTN_LOCK_W + TOOLBAR_GAP;
    [
        ToolbarButtonRect {
            button: ToolbarButton::Console,
            x: x0,
            y: TOOLBAR_TOP_Y,
            width: BTN_CONSOLE_W,
            height: TOOLBAR_HEIGHT,
        },
        ToolbarButtonRect {
            button: ToolbarButton::Lock,
            x: x1,
            y: TOOLBAR_TOP_Y,
            width: BTN_LOCK_W,
            height: TOOLBAR_HEIGHT,
        },
        ToolbarButtonRect {
            button: ToolbarButton::AudioSync,
            x: x2,
            y: TOOLBAR_TOP_Y,
            width: BTN_AUDIO_W,
            height: TOOLBAR_HEIGHT,
        },
    ]
}

/// Hit-tests logical `(lx, ly)` against the floating toolbar pill buttons.
pub fn toolbar_hit_test(lx: f32, ly: f32) -> Option<ToolbarButton> {
    toolbar_layout()
        .into_iter()
        .find(|r| r.contains(lx, ly))
        .map(|r| r.button)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextCacheKey {
    text: &'static str,
    rgb: [u8; 3],
    dpi_percent: u32,
}

/// Caches antialiased Chinese text labels rasterized into premultiplied RGBA `Pixmap`s.
#[derive(Default)]
pub struct TextLabelCache {
    entries: HashMap<TextCacheKey, Option<Pixmap>>,
}

impl TextLabelCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn get_or_rasterize(
        &mut self,
        text: &'static str,
        rgb: [u8; 3],
        scale: f32,
    ) -> Option<&Pixmap> {
        let dpi_percent = (scale * 100.0).round().clamp(50.0, 400.0) as u32;
        let key = TextCacheKey {
            text,
            rgb,
            dpi_percent,
        };
        let entry = self.entries.entry(key).or_insert_with(|| {
            rasterize_label_premul_rgba(text, rgb, (dpi_percent as f32) / 100.0)
        });
        entry.as_ref()
    }
}

/// Draws the floating pastel pill toolbar (`控制台`, `锁定`, `音频同步` / `停止同步`) onto `pixmap`.
pub fn draw_toolbar(
    pixmap: &mut Pixmap,
    state: ToolbarState,
    music_running: bool,
    elapsed_ms: u64,
    dpi_x: f32,
    dpi_y: f32,
    label_cache: &mut TextLabelCache,
) {
    let alpha = state.opacity.clamp(0.0, 1.0);
    if alpha <= 0.01 {
        return;
    }

    let scale = (dpi_x + dpi_y) * 0.5;
    let buttons = toolbar_layout();

    for rect in buttons {
        let is_hovered = state.hovered == Some(rect.button);
        let is_pressed = state.pressed == Some(rect.button);
        let is_audio_active = rect.button == ToolbarButton::AudioSync && music_running;

        let press_dy = if is_pressed { 1.0 } else { 0.0 };
        let bx = rect.x;
        let by = rect.y + press_dy;
        let bw = rect.width;
        let bh = rect.height;
        let radius = bh * 0.5;

        // 1) Soft pastel drop shadow beneath pill
        if let Some(shadow_path) = rounded_rect_path(
            bx * dpi_x,
            (by + 2.2) * dpi_y,
            bw * dpi_x,
            bh * dpi_y,
            radius * scale,
        ) {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.set_color(Color::from_rgba8(
                110,
                79,
                117,
                (44.0 * alpha).round() as u8,
            ));
            pixmap.fill_path(
                &shadow_path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        // 2) Pill body fill
        let (fill_r, fill_g, fill_b, fill_a) = if is_audio_active {
            if is_pressed {
                (255, 210, 232, 250)
            } else if is_hovered {
                (255, 224, 240, 252)
            } else {
                (255, 232, 244, 245)
            }
        } else if is_pressed {
            (255, 228, 240, 252)
        } else if is_hovered {
            (255, 240, 246, 252)
        } else {
            (255, 249, 252, 242)
        };

        if let Some(pill_path) = rounded_rect_path(
            bx * dpi_x,
            by * dpi_y,
            bw * dpi_x,
            bh * dpi_y,
            radius * scale,
        ) {
            let mut fill_paint = Paint::default();
            fill_paint.anti_alias = true;
            fill_paint.set_color(Color::from_rgba8(
                fill_r,
                fill_g,
                fill_b,
                ((fill_a as f32) * alpha).round() as u8,
            ));
            pixmap.fill_path(
                &pill_path,
                &fill_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );

            // 3) Crisp border
            let (br, bg, bb) = if is_audio_active {
                (204, 86, 146)
            } else if is_hovered {
                (148, 86, 152)
            } else {
                (110, 79, 117)
            };
            let mut stroke_paint = Paint::default();
            stroke_paint.anti_alias = true;
            stroke_paint.set_color(Color::from_rgba8(
                br,
                bg,
                bb,
                (235.0 * alpha).round() as u8,
            ));
            let stroke = Stroke {
                width: (1.5 * scale).max(1.0),
                ..Stroke::default()
            };
            pixmap.stroke_path(
                &pill_path,
                &stroke_paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }

        let fg_rgb = if is_audio_active {
            [176, 48, 114]
        } else {
            [90, 61, 99]
        };

        // 4) Cute vector icon on the left side of the pill
        let icon_cx = (bx + 17.0) * dpi_x;
        let icon_cy = (by + bh * 0.5) * dpi_y;
        draw_button_icon(
            pixmap,
            rect.button,
            music_running,
            elapsed_ms,
            icon_cx,
            icon_cy,
            scale,
            fg_rgb,
            alpha,
        );

        // 5) Cached crisp Chinese label
        let label_text = match rect.button {
            ToolbarButton::Console => "控制台",
            ToolbarButton::Lock => "锁定",
            ToolbarButton::AudioSync => {
                if music_running {
                    "停止同步"
                } else {
                    "音频同步"
                }
            }
        };

        if let Some(label_pm) = label_cache.get_or_rasterize(label_text, fg_rgb, scale) {
            let text_area_left = (bx + 26.0) * dpi_x;
            let text_area_w = (bw - 32.0) * dpi_x;
            let dst_x = (text_area_left + (text_area_w - label_pm.width() as f32) * 0.5).round()
                as i32;
            let dst_y = ((by + bh * 0.5) * dpi_y - (label_pm.height() as f32) * 0.5).round() as i32;
            let mut paint = PixmapPaint::default();
            paint.opacity = alpha;
            paint.quality = FilterQuality::Nearest;
            pixmap.draw_pixmap(dst_x, dst_y, label_pm.as_ref(), &paint, Transform::identity(), None);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_button_icon(
    pixmap: &mut Pixmap,
    button: ToolbarButton,
    music_running: bool,
    elapsed_ms: u64,
    cx: f32,
    cy: f32,
    scale: f32,
    rgb: [u8; 3],
    alpha: f32,
) {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.set_color(Color::from_rgba8(
        rgb[0],
        rgb[1],
        rgb[2],
        (240.0 * alpha).round() as u8,
    ));

    let stroke = Stroke {
        width: (1.5 * scale).max(1.0),
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Stroke::default()
    };

    match button {
        ToolbarButton::Console => {
            // 3 cute horizontal slider bars with tiny knobs
            let offsets = [(-3.6_f32, -1.6_f32), (0.0, 2.0), (3.6, -0.8)];
            for (dy, knob_dx) in offsets {
                let mut pb = PathBuilder::new();
                pb.move_to(cx - 5.2 * scale, cy + dy * scale);
                pb.line_to(cx + 5.2 * scale, cy + dy * scale);
                if let Some(p) = pb.finish() {
                    pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
                }
                if let Some(knob) =
                    PathBuilder::from_circle(cx + knob_dx * scale, cy + dy * scale, 1.7 * scale)
                {
                    pixmap.fill_path(
                        &knob,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        ToolbarButton::Lock => {
            // Cute miniature padlock
            if let Some(body) = rounded_rect_path(
                cx - 4.6 * scale,
                cy - 0.5 * scale,
                9.2 * scale,
                6.6 * scale,
                2.0 * scale,
            ) {
                pixmap.fill_path(&body, &paint, FillRule::Winding, Transform::identity(), None);
            }
            let mut shackle = PathBuilder::new();
            shackle.move_to(cx - 2.8 * scale, cy - 0.5 * scale);
            shackle.line_to(cx - 2.8 * scale, cy - 3.2 * scale);
            shackle.cubic_to(
                cx - 2.8 * scale,
                cy - 6.0 * scale,
                cx + 2.8 * scale,
                cy - 6.0 * scale,
                cx + 2.8 * scale,
                cy - 3.2 * scale,
            );
            shackle.line_to(cx + 2.8 * scale, cy - 0.5 * scale);
            if let Some(p) = shackle.finish() {
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }
        ToolbarButton::AudioSync => {
            // Musical eighth note + live pulsing dot when active
            if let Some(head) = PathBuilder::from_circle(
                cx - 2.2 * scale,
                cy + 3.2 * scale,
                2.5 * scale,
            ) {
                pixmap.fill_path(&head, &paint, FillRule::Winding, Transform::identity(), None);
            }
            let mut stem = PathBuilder::new();
            stem.move_to(cx - 0.2 * scale, cy + 3.0 * scale);
            stem.line_to(cx - 0.2 * scale, cy - 5.0 * scale);
            stem.quad_to(
                cx + 4.4 * scale,
                cy - 4.0 * scale,
                cx + 3.8 * scale,
                cy - 0.6 * scale,
            );
            if let Some(p) = stem.finish() {
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
            if music_running {
                let pulse = 0.55 + 0.45 * (((elapsed_ms as f32) / 360.0).sin());
                let mut dot_paint = Paint::default();
                dot_paint.anti_alias = true;
                dot_paint.set_color(Color::from_rgba8(
                    236,
                    72,
                    153,
                    (240.0 * pulse * alpha).round() as u8,
                ));
                if let Some(dot) = PathBuilder::from_circle(
                    cx + 5.0 * scale,
                    cy + 3.6 * scale,
                    (1.9 + 0.5 * pulse) * scale,
                ) {
                    pixmap.fill_path(
                        &dot,
                        &dot_paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
    }
}

/// Builds a rounded rectangle path.
pub fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = radius.clamp(0.0, (w * 0.5).min(h * 0.5));
    if r <= 0.01 {
        return Rect::from_xywh(x, y, w, h).map(PathBuilder::from_rect);
    }
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(
        x + w,
        y + h - r + k,
        x + w - r + k,
        y + h,
        x + w - r,
        y + h,
    );
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Rasterizes a UTF-8 Chinese/ASCII label via Windows GDI grayscale antialiasing (`ANTIALIASED_QUALITY`)
/// into a premultiplied RGBA8 `tiny_skia::Pixmap` with zero white halo (`r, g, b <= a`).
#[cfg(windows)]
fn rasterize_label_premul_rgba(text: &str, rgb: [u8; 3], scale: f32) -> Option<Pixmap> {
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{RECT, SIZE};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW,
        GetTextExtentPoint32W, SelectObject, SetBkMode, SetTextColor, ANTIALIASED_QUALITY,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH,
        DIB_RGB_COLORS, DT_CENTER, DT_NOCLIP, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD,
        OUT_TT_PRECIS, TRANSPARENT,
    };

    let wide_text: Vec<u16> = text.encode_utf16().collect();
    if wide_text.is_empty() {
        return None;
    }

    let font_height = -((13.0 * scale).round().max(9.0) as i32);
    let mut face_name = [0u16; 32];
    for (dst, src) in face_name
        .iter_mut()
        .zip("Microsoft YaHei UI".encode_utf16())
    {
        *dst = src;
    }

    unsafe {
        let hdc = CreateCompatibleDC(null_mut());
        if hdc.is_null() {
            return None;
        }

        let hfont = CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_BOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_TT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            ANTIALIASED_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            face_name.as_ptr(),
        );
        if hfont.is_null() {
            DeleteDC(hdc);
            return None;
        }

        let old_font = SelectObject(hdc, hfont as _);
        let mut ext = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, wide_text.as_ptr(), wide_text.len() as i32, &mut ext);

        let pad = (2.0 * scale).ceil() as i32;
        let width = (ext.cx + pad * 2).max(4) as u32;
        let height = (ext.cy + pad * 2).max(4) as u32;

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // top-down
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
        let hbm = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if hbm.is_null() || bits.is_null() {
            SelectObject(hdc, old_font);
            DeleteObject(hfont as _);
            DeleteDC(hdc);
            return None;
        }

        let old_bm = SelectObject(hdc, hbm as _);
        let byte_len = (width as usize) * (height as usize) * 4;
        std::ptr::write_bytes(bits as *mut u8, 0, byte_len);

        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, 0x00FF_FFFF);

        let mut rc = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        DrawTextW(
            hdc,
            wide_text.as_ptr(),
            wide_text.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_CENTER | DT_VCENTER | DT_NOCLIP,
        );

        let raw = std::slice::from_raw_parts(bits as *const u8, byte_len);
        let mut pixmap = Pixmap::new(width, height);
        if let Some(pm) = pixmap.as_mut() {
            let dst = pm.data_mut();
            let tr = rgb[0] as u16;
            let tg = rgb[1] as u16;
            let tb = rgb[2] as u16;
            for (s, d) in raw.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                // Grayscale AA on pure black background -> max(B,G,R) is exact coverage alpha.
                let cov = s[0].max(s[1]).max(s[2]);
                if cov == 0 {
                    d[0] = 0;
                    d[1] = 0;
                    d[2] = 0;
                    d[3] = 0;
                } else {
                    let a = cov as u16;
                    d[0] = ((tr * a + 127) / 255) as u8;
                    d[1] = ((tg * a + 127) / 255) as u8;
                    d[2] = ((tb * a + 127) / 255) as u8;
                    d[3] = cov;
                }
            }
        }

        SelectObject(hdc, old_bm);
        SelectObject(hdc, old_font);
        DeleteObject(hbm as _);
        DeleteObject(hfont as _);
        DeleteDC(hdc);

        pixmap
    }
}

#[cfg(not(windows))]
fn rasterize_label_premul_rgba(_text: &str, _rgb: [u8; 3], _scale: f32) -> Option<Pixmap> {
    None
}
