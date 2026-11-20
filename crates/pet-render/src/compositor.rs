use std::time::Instant;

use pet_core::{
    ActivityMode, AnimationPose, LoadedCharacterAssets, NoteParticleRender, TightSprite,
};
use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, LineCap, LineJoin, Paint, Path, PathBuilder,
    Pixmap, PixmapPaint, PixmapRef, Point, RadialGradient, SpreadMode, Stroke, Transform,
};

use crate::toolbar::{draw_toolbar, TextLabelCache, ToolbarButton, ToolbarState};
use crate::{PetRenderInput, PET_LOGICAL_HEIGHT, PET_LOGICAL_WIDTH};

/// Exact rounded division by 255 for `0..=65025` without a hardware division instruction.
#[inline(always)]
fn div255(x: u32) -> u32 {
    let t = x + 128;
    (t + (t >> 8)) >> 8
}

/// Blends a single premultiplied BGRA8 pixel `(sb, sg, sr, sa)` over `dst` (`[b, g, r, a]`),
/// preserving the strict `b, g, r <= a` premultiplied invariant.
#[inline(always)]
fn blend_premul_bgra_over(dst: &mut [u8], sb: u8, sg: u8, sr: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    if sa == 255 {
        dst[0] = sb;
        dst[1] = sg;
        dst[2] = sr;
        dst[3] = 255;
        return;
    }
    let inv_a = (255 - sa) as u32;
    let out_a = (sa as u32 + div255((dst[3] as u32) * inv_a)).min(255);
    dst[0] = (sb as u32 + div255((dst[0] as u32) * inv_a)).min(out_a) as u8;
    dst[1] = (sg as u32 + div255((dst[1] as u32) * inv_a)).min(out_a) as u8;
    dst[2] = (sr as u32 + div255((dst[2] as u32) * inv_a)).min(out_a) as u8;
    dst[3] = out_a as u8;
}

/// Tightly cropped physical-resolution premultiplied BGRA8 sprite with per-row active spans
/// for near-zero-overhead identity and fixed-point bilinear affine blitting.
struct CroppedBgra {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    /// Premultiplied BGRA8 pixels (`width * height * 4`).
    bgra: Vec<u8>,
    /// Per-row `[start_x, end_x_exclusive)` non-zero alpha spans to skip empty margins fast.
    row_spans: Vec<(u16, u16)>,
}

impl CroppedBgra {
    /// Crops non-zero alpha bounding box from a premultiplied RGBA8 `tiny-skia::Pixmap`
    /// and converts once into premultiplied BGRA8.
    fn from_rgba_pixmap(src: &Pixmap) -> Option<Self> {
        let w = src.width();
        let h = src.height();
        let data = src.data();

        let mut min_x = w;
        let mut min_y = h;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;

        for y in 0..h {
            let row = (y as usize) * (w as usize) * 4;
            for x in 0..w {
                if data[row + (x as usize) * 4 + 3] != 0 {
                    if !found {
                        min_x = x;
                        max_x = x;
                        min_y = y;
                        max_y = y;
                        found = true;
                    } else {
                        if x < min_x {
                            min_x = x;
                        }
                        if x > max_x {
                            max_x = x;
                        }
                        if y < min_y {
                            min_y = y;
                        }
                        if y > max_y {
                            max_y = y;
                        }
                    }
                }
            }
        }

        if !found {
            return None;
        }

        let cw = max_x - min_x + 1;
        let ch = max_y - min_y + 1;
        let mut bgra = vec![0u8; (cw as usize) * (ch as usize) * 4];
        let mut row_spans = Vec::with_capacity(ch as usize);

        for ry in 0..ch {
            let src_y = min_y + ry;
            let mut span_start = cw as u16;
            let mut span_end = 0u16;
            for rx in 0..cw {
                let src_idx = ((src_y as usize) * (w as usize) + (min_x + rx) as usize) * 4;
                let dst_idx = ((ry as usize) * (cw as usize) + rx as usize) * 4;
                let a = data[src_idx + 3];
                if a != 0 {
                    if (rx as u16) < span_start {
                        span_start = rx as u16;
                    }
                    span_end = (rx as u16) + 1;
                    bgra[dst_idx] = data[src_idx + 2].min(a);
                    bgra[dst_idx + 1] = data[src_idx + 1].min(a);
                    bgra[dst_idx + 2] = data[src_idx].min(a);
                    bgra[dst_idx + 3] = a;
                }
            }
            if span_start < span_end {
                row_spans.push((span_start, span_end));
            } else {
                row_spans.push((0, 0));
            }
        }

        Some(Self {
            x: min_x as i32,
            y: min_y as i32,
            width: cw,
            height: ch,
            bgra,
            row_spans,
        })
    }

    /// Creates a `CroppedBgra` directly from a `TightSprite` (in canvas coordinates).
    fn from_tight_sprite(sprite: &TightSprite) -> Self {
        let cw = sprite.width;
        let ch = sprite.height;
        let mut bgra = vec![0u8; (cw as usize) * (ch as usize) * 4];
        let mut row_spans = Vec::with_capacity(ch as usize);
        let src = &sprite.premul_rgba;
        for ry in 0..ch {
            let mut span_start = cw as u16;
            let mut span_end = 0u16;
            let row_off = (ry as usize) * (cw as usize) * 4;
            for rx in 0..cw {
                let idx = row_off + (rx as usize) * 4;
                let a = src[idx + 3];
                if a != 0 {
                    if (rx as u16) < span_start {
                        span_start = rx as u16;
                    }
                    span_end = (rx as u16) + 1;
                    bgra[idx] = src[idx + 2].min(a);
                    bgra[idx + 1] = src[idx + 1].min(a);
                    bgra[idx + 2] = src[idx].min(a);
                    bgra[idx + 3] = a;
                }
            }
            if span_start < span_end {
                row_spans.push((span_start, span_end));
            } else {
                row_spans.push((0, 0));
            }
        }
        Self {
            x: sprite.offset_x as i32,
            y: sprite.offset_y as i32,
            width: cw,
            height: ch,
            bgra,
            row_spans,
        }
    }

    /// Ultra-fast 1:1 premultiplied BGRA SourceOver blit using per-row active spans.
    fn blit_identity(&self, dst: &mut [u8], dst_w: u32, dst_h: u32) {
        let dw = dst_w as i32;
        let dh = dst_h as i32;
        let sw = self.width as usize;

        for (ry, &(sx0, sx1)) in self.row_spans.iter().enumerate() {
            if sx0 >= sx1 {
                continue;
            }
            let dy = self.y + (ry as i32);
            if dy < 0 || dy >= dh {
                continue;
            }
            let x_start = (self.x + sx0 as i32).max(0);
            let x_end = (self.x + sx1 as i32).min(dw);
            if x_start >= x_end {
                continue;
            }

            let src_rx_start = (x_start - self.x) as usize;
            let count = (x_end - x_start) as usize;
            let src_row_off = (ry * sw + src_rx_start) * 4;
            let dst_row_off = ((dy as usize) * (dst_w as usize) + (x_start as usize)) * 4;

            let src_slice = &self.bgra[src_row_off..src_row_off + count * 4];
            let dst_slice = &mut dst[dst_row_off..dst_row_off + count * 4];

            for (s, d) in src_slice.chunks_exact(4).zip(dst_slice.chunks_exact_mut(4)) {
                blend_premul_bgra_over(d, s[0], s[1], s[2], s[3]);
            }
        }
    }

    /// High-speed 16.16 fixed-point affine bilinear premultiplied BGRA blitter.
    fn blit_transformed(
        &self,
        dst: &mut [u8],
        dst_w: u32,
        dst_h: u32,
        extra_transform: Transform,
    ) {
        // Fast path: pure integer or near-integer translation with no scale/rotation
        if (extra_transform.sx - 1.0).abs() < 1e-4
            && (extra_transform.sy - 1.0).abs() < 1e-4
            && extra_transform.kx.abs() < 1e-4
            && extra_transform.ky.abs() < 1e-4
            && (extra_transform.tx - extra_transform.tx.round()).abs() < 0.02
            && (extra_transform.ty - extra_transform.ty.round()).abs() < 0.02
        {
            let shift_x = extra_transform.tx.round() as i32;
            let shift_y = extra_transform.ty.round() as i32;
            self.blit_offset(dst, dst_w, dst_h, shift_x, shift_y);
            return;
        }

        let fwd =
            Transform::from_translate(self.x as f32, self.y as f32).post_concat(extra_transform);
        blit_bgra_affine_bilinear(
            dst,
            dst_w,
            dst_h,
            &self.bgra,
            self.width,
            self.height,
            fwd,
        );
    }

    fn blit_offset(&self, dst: &mut [u8], dst_w: u32, dst_h: u32, dx_off: i32, dy_off: i32) {
        let dw = dst_w as i32;
        let dh = dst_h as i32;
        let sw = self.width as usize;
        let ox = self.x + dx_off;
        let oy = self.y + dy_off;

        for (ry, &(sx0, sx1)) in self.row_spans.iter().enumerate() {
            if sx0 >= sx1 {
                continue;
            }
            let dy = oy + (ry as i32);
            if dy < 0 || dy >= dh {
                continue;
            }
            let x_start = (ox + sx0 as i32).max(0);
            let x_end = (ox + sx1 as i32).min(dw);
            if x_start >= x_end {
                continue;
            }

            let src_rx_start = (x_start - ox) as usize;
            let count = (x_end - x_start) as usize;
            let src_row_off = (ry * sw + src_rx_start) * 4;
            let dst_row_off = ((dy as usize) * (dst_w as usize) + (x_start as usize)) * 4;

            let src_slice = &self.bgra[src_row_off..src_row_off + count * 4];
            let dst_slice = &mut dst[dst_row_off..dst_row_off + count * 4];

            for (s, d) in src_slice.chunks_exact(4).zip(dst_slice.chunks_exact_mut(4)) {
                blend_premul_bgra_over(d, s[0], s[1], s[2], s[3]);
            }
        }
    }
}

/// Core 16.16 fixed-point affine bilinear premultiplied BGRA blitter.
/// Samples pixel centers with transparent 0-padding outside `[0..sw) x [0..sh)` for smooth AA borders.
fn blit_bgra_affine_bilinear(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    src: &[u8],
    sw: u32,
    sh: u32,
    fwd: Transform,
) {
    if sw == 0 || sh == 0 || dst_w == 0 || dst_h == 0 {
        return;
    }
    let Some(inv) = fwd.invert() else {
        return;
    };

    // Transform padded source corners [-1, sw+1] x [-1, sh+1] to tight destination bbox
    let corners = [
        (-1.0_f32, -1.0_f32),
        (sw as f32 + 1.0, -1.0),
        (-1.0, sh as f32 + 1.0),
        (sw as f32 + 1.0, sh as f32 + 1.0),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (cx, cy) in corners {
        let tx = fwd.sx * cx + fwd.kx * cy + fwd.tx;
        let ty = fwd.ky * cx + fwd.sy * cy + fwd.ty;
        if tx < min_x {
            min_x = tx;
        }
        if tx > max_x {
            max_x = tx;
        }
        if ty < min_y {
            min_y = ty;
        }
        if ty > max_y {
            max_y = ty;
        }
    }

    let x0 = (min_x.floor() as i32).max(0);
    let y0 = (min_y.floor() as i32).max(0);
    let x1 = (max_x.ceil() as i32).min(dst_w as i32 - 1);
    let y1 = (max_y.ceil() as i32).min(dst_h as i32 - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }

    let du_dx = (inv.sx * 65536.0).round() as i32;
    let dv_dx = (inv.ky * 65536.0).round() as i32;
    let sw_i = sw as i32;
    let sh_i = sh as i32;
    let sw_usize = sw as usize;
    let src_row_bytes = sw_usize * 4;
    let dst_row_bytes = (dst_w as usize) * 4;

    for dy in y0..=y1 {
        let py = dy as f32 + 0.5;
        let px0 = x0 as f32 + 0.5;
        let u_start = (inv.sx * px0 + inv.kx * py + inv.tx - 0.5) * 65536.0;
        let v_start = (inv.ky * px0 + inv.sy * py + inv.ty - 0.5) * 65536.0;
        let mut u = u_start.round() as i32;
        let mut v = v_start.round() as i32;

        let dst_row = &mut dst[(dy as usize) * dst_row_bytes..];

        for dx in x0..=x1 {
            let ix = u >> 16;
            let iy = v >> 16;

            if ix >= 0 && ix < sw_i - 1 && iy >= 0 && iy < sh_i - 1 {
                // Fast interior path: all 4 taps guaranteed in-bounds!
                let base = (iy as usize) * src_row_bytes + (ix as usize) * 4;
                let a00 = src[base + 3];
                let a10 = src[base + 7];
                let base1 = base + src_row_bytes;
                let a01 = src[base1 + 3];
                let a11 = src[base1 + 7];

                if (a00 | a10 | a01 | a11) != 0 {
                    let fx = ((u >> 8) & 0xFF) as u32;
                    let fy = ((v >> 8) & 0xFF) as u32;
                    let ifx = 256 - fx;
                    let ify = 256 - fy;
                    let w00 = ifx * ify;
                    let w10 = fx * ify;
                    let w01 = ifx * fy;
                    let w11 = fx * fy;

                    let sa = ((a00 as u32 * w00
                        + a10 as u32 * w10
                        + a01 as u32 * w01
                        + a11 as u32 * w11
                        + 32768)
                        >> 16) as u8;
                    if sa != 0 {
                        let sb = ((src[base] as u32 * w00
                            + src[base + 4] as u32 * w10
                            + src[base1] as u32 * w01
                            + src[base1 + 4] as u32 * w11
                            + 32768)
                            >> 16)
                            .min(sa as u32) as u8;
                        let sg = ((src[base + 1] as u32 * w00
                            + src[base + 5] as u32 * w10
                            + src[base1 + 1] as u32 * w01
                            + src[base1 + 5] as u32 * w11
                            + 32768)
                            >> 16)
                            .min(sa as u32) as u8;
                        let sr = ((src[base + 2] as u32 * w00
                            + src[base + 6] as u32 * w10
                            + src[base1 + 2] as u32 * w01
                            + src[base1 + 6] as u32 * w11
                            + 32768)
                            >> 16)
                            .min(sa as u32) as u8;

                        let doff = (dx as usize) * 4;
                        blend_premul_bgra_over(&mut dst_row[doff..doff + 4], sb, sg, sr, sa);
                    }
                }
            } else if ix >= -1 && ix < sw_i && iy >= -1 && iy < sh_i {
                // Smooth 1px edge path with zero-alpha border taps
                let sample = |sx: i32, sy: i32| -> [u32; 4] {
                    if sx >= 0 && sx < sw_i && sy >= 0 && sy < sh_i {
                        let idx = (sy as usize) * src_row_bytes + (sx as usize) * 4;
                        [
                            src[idx] as u32,
                            src[idx + 1] as u32,
                            src[idx + 2] as u32,
                            src[idx + 3] as u32,
                        ]
                    } else {
                        [0, 0, 0, 0]
                    }
                };
                let p00 = sample(ix, iy);
                let p10 = sample(ix + 1, iy);
                let p01 = sample(ix, iy + 1);
                let p11 = sample(ix + 1, iy + 1);
                if (p00[3] | p10[3] | p01[3] | p11[3]) != 0 {
                    let fx = ((u >> 8) & 0xFF) as u32;
                    let fy = ((v >> 8) & 0xFF) as u32;
                    let ifx = 256 - fx;
                    let ify = 256 - fy;
                    let w00 = ifx * ify;
                    let w10 = fx * ify;
                    let w01 = ifx * fy;
                    let w11 = fx * fy;

                    let sa = ((p00[3] * w00 + p10[3] * w10 + p01[3] * w01 + p11[3] * w11 + 32768)
                        >> 16) as u8;
                    if sa != 0 {
                        let sb = ((p00[0] * w00
                            + p10[0] * w10
                            + p01[0] * w01
                            + p11[0] * w11
                            + 32768)
                            >> 16)
                            .min(sa as u32) as u8;
                        let sg = ((p00[1] * w00
                            + p10[1] * w10
                            + p01[1] * w01
                            + p11[1] * w11
                            + 32768)
                            >> 16)
                            .min(sa as u32) as u8;
                        let sr = ((p00[2] * w00
                            + p10[2] * w10
                            + p01[2] * w01
                            + p11[2] * w11
                            + 32768)
                            >> 16)
                            .min(sa as u32) as u8;

                        let doff = (dx as usize) * 4;
                        blend_premul_bgra_over(&mut dst_row[doff..doff + 4], sb, sg, sr, sa);
                    }
                }
            }

            u = u.wrapping_add(du_dx);
            v = v.wrapping_add(dv_dx);
        }
    }
}

/// Pre-baked musical note glyph alpha template (`halo_alpha`, `fill_alpha`) rasterized once via `tiny-skia`.
struct NoteGlyphTemplate {
    size: u32,
    center: f32,
    /// Interleaved `(halo_alpha, fill_alpha)` pairs per pixel.
    masks: Vec<(u8, u8)>,
}

struct NoteGlyphAtlas {
    eighth: NoteGlyphTemplate,
    quarter: NoteGlyphTemplate,
    beamed_eighth: NoteGlyphTemplate,
    beamed_sixteenth: NoteGlyphTemplate,
    sparkle: NoteGlyphTemplate,
}

impl NoteGlyphAtlas {
    fn new() -> Self {
        Self {
            eighth: bake_note_template('♪'),
            quarter: bake_note_template('♩'),
            beamed_eighth: bake_note_template('♫'),
            beamed_sixteenth: bake_note_template('♬'),
            sparkle: bake_note_template('✦'),
        }
    }

    fn get(&self, glyph: char) -> &NoteGlyphTemplate {
        match glyph {
            '♪' => &self.eighth,
            '♩' => &self.quarter,
            '♫' => &self.beamed_eighth,
            '♬' => &self.beamed_sixteenth,
            _ => &self.sparkle,
        }
    }
}

fn bake_note_template(glyph: char) -> NoteGlyphTemplate {
    let size = 32u32;
    let center = (size as f32) * 0.5;
    let mut halo_pm = Pixmap::new(size, size).expect("note halo pixmap");
    let mut fill_pm = Pixmap::new(size, size).expect("note fill pixmap");

    if let Some(path) = build_note_glyph_path(glyph) {
        let tf = Transform::from_translate(center, center);

        let mut halo_paint = Paint::default();
        halo_paint.anti_alias = true;
        halo_paint.set_color(Color::WHITE);
        let halo_stroke = Stroke {
            width: 3.4,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        halo_pm.stroke_path(&path, &halo_paint, &halo_stroke, tf, None);

        let mut fill_paint = Paint::default();
        fill_paint.anti_alias = true;
        fill_paint.set_color(Color::WHITE);
        fill_pm.fill_path(&path, &fill_paint, FillRule::Winding, tf, None);
        let inner_stroke = Stroke {
            width: 1.9,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        fill_pm.stroke_path(&path, &fill_paint, &inner_stroke, tf, None);
    }

    let mut masks = Vec::with_capacity((size * size) as usize);
    for (h, f) in halo_pm
        .data()
        .chunks_exact(4)
        .zip(fill_pm.data().chunks_exact(4))
    {
        masks.push((h[3], f[3]));
    }

    NoteGlyphTemplate {
        size,
        center,
        masks,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticCacheKey {
    activity_mode: ActivityMode,
    asset_generation: u64,
    asset_fingerprint: u64,
    width: u32,
    height: u32,
}

struct StaticLayerCache {
    key: StaticCacheKey,
    static_back: Option<CroppedBgra>,
    static_back_dynamic: bool,
    static_desk: Option<CroppedBgra>,
    static_desk_dynamic: bool,
    static_arms: Option<CroppedBgra>,
    static_arms_dynamic: bool,
    static_front_combined: Option<CroppedBgra>,
    body_base: Option<CroppedBgra>,
    body_dynamic: bool,
    timer_clock: Option<CroppedBgra>,
    timer_dynamic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeadCacheKey {
    eyes: String,
    mouth: String,
    show_headphones: bool,
    asset_generation: u64,
    asset_fingerprint: u64,
    width: u32,
    height: u32,
}

struct HeadRigCache {
    key: HeadCacheKey,
    pixmap: Option<CroppedBgra>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OverlayCacheKey {
    name: String,
    frame_ptr: usize,
    asset_generation: u64,
    asset_fingerprint: u64,
    width: u32,
    height: u32,
}

struct OverlayCache {
    key: OverlayCacheKey,
    pixmap: Option<CroppedBgra>,
}

#[derive(Debug, Clone, Copy)]
struct StageGeometry {
    stage_x: f32,
    stage_y: f32,
    stage_w: f32,
    stage_h: f32,
    fit_x: f32,
    fit_y: f32,
    fit_scale: f32,
    dpi_x: f32,
    dpi_y: f32,
}

impl StageGeometry {
    fn compute(out_w: u32, out_h: u32, canvas_w: u32, canvas_h: u32) -> Self {
        let logical_w = PET_LOGICAL_WIDTH as f32;
        let logical_h = PET_LOGICAL_HEIGHT as f32;
        let dpi_x = (out_w as f32) / logical_w;
        let dpi_y = (out_h as f32) / logical_h;

        // Square stage anchored to bottom of 540x590 window, leaving 50 logical px at top for toolbar.
        let stage_size = logical_w.min(logical_h);
        let stage_w = stage_size;
        let stage_h = stage_size;
        let stage_x = (logical_w - stage_w) * 0.5;
        let stage_y = logical_h - stage_h;

        let cw = (canvas_w.max(1)) as f32;
        let ch = (canvas_h.max(1)) as f32;
        let fit_scale = (stage_w / cw).min(stage_h / ch);
        let fit_x = stage_x + (stage_w - cw * fit_scale) * 0.5;
        let fit_y = stage_y + (stage_h - ch * fit_scale) * 0.5;

        Self {
            stage_x,
            stage_y,
            stage_w,
            stage_h,
            fit_x,
            fit_y,
            fit_scale,
            dpi_x,
            dpi_y,
        }
    }

    fn sprite_to_physical(&self, sprite: &TightSprite) -> Transform {
        Transform::from_translate(sprite.offset_x as f32, sprite.offset_y as f32)
            .post_scale(self.fit_scale * self.dpi_x, self.fit_scale * self.dpi_y)
            .post_translate(self.fit_x * self.dpi_x, self.fit_y * self.dpi_y)
    }

    fn dpi_avg(&self) -> f32 {
        (self.dpi_x + self.dpi_y) * 0.5
    }
}

/// CPU `tiny-skia` tight-bbox compositor producing premultiplied BGRA8 frames directly for Win32 `UpdateLayeredWindow`.
pub struct PetCompositor {
    width: u32,
    height: u32,
    scale_factor: f32,
    bgra_buffer: Vec<u8>,
    toolbar_strip: Pixmap,
    note_atlas: NoteGlyphAtlas,
    asset_generation: u64,
    static_cache: Option<StaticLayerCache>,
    head_cache: Option<HeadRigCache>,
    overlay_cache: Option<OverlayCache>,
    label_cache: TextLabelCache,
    toolbar_state: ToolbarState,
    static_cache_hits: u64,
    static_cache_misses: u64,
    last_render_ms: f64,
}

impl PetCompositor {
    pub fn new(width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let scale_factor = (width as f32) / (PET_LOGICAL_WIDTH as f32);
        let strip_h = ((56.0 * scale_factor).ceil() as u32).clamp(1, height);
        let toolbar_strip = Pixmap::new(width, strip_h).expect("valid toolbar strip dimensions");
        Self {
            width,
            height,
            scale_factor,
            bgra_buffer: vec![0u8; (width as usize) * (height as usize) * 4],
            toolbar_strip,
            note_atlas: NoteGlyphAtlas::new(),
            asset_generation: 0,
            static_cache: None,
            head_cache: None,
            overlay_cache: None,
            label_cache: TextLabelCache::default(),
            toolbar_state: ToolbarState::default(),
            static_cache_hits: 0,
            static_cache_misses: 0,
            last_render_ms: 0.0,
        }
    }

    /// Creates a compositor sized for the given DPI scale factor (e.g., `1.0`, `1.25`, `1.5`, `2.0`).
    pub fn with_scale_factor(scale_factor: f32) -> Self {
        let clamped = scale_factor.clamp(0.5, 4.0);
        let w = ((PET_LOGICAL_WIDTH as f32) * clamped).round().max(1.0) as u32;
        let h = ((PET_LOGICAL_HEIGHT as f32) * clamped).round().max(1.0) as u32;
        let mut comp = Self::new(w, h);
        comp.scale_factor = clamped;
        comp
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.scale_factor = (width as f32) / (PET_LOGICAL_WIDTH as f32);
            self.bgra_buffer
                .resize((width as usize) * (height as usize) * 4, 0);
            let strip_h = ((56.0 * self.scale_factor).ceil() as u32).clamp(1, height);
            self.toolbar_strip =
                Pixmap::new(width, strip_h).expect("valid toolbar strip dimensions");
            self.static_cache = None;
            self.head_cache = None;
            self.overlay_cache = None;
            self.label_cache.clear();
        }
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        let clamped = scale_factor.clamp(0.5, 4.0);
        let w = ((PET_LOGICAL_WIDTH as f32) * clamped).round().max(1.0) as u32;
        let h = ((PET_LOGICAL_HEIGHT as f32) * clamped).round().max(1.0) as u32;
        self.scale_factor = clamped;
        self.resize(w, h);
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn invalidate_static_cache(&mut self) {
        self.asset_generation = self.asset_generation.wrapping_add(1);
        self.static_cache = None;
        self.head_cache = None;
        self.overlay_cache = None;
    }

    pub fn set_asset_generation(&mut self, generation: u64) {
        if self.asset_generation != generation {
            self.asset_generation = generation;
            self.static_cache = None;
            self.head_cache = None;
            self.overlay_cache = None;
        }
    }

    pub fn set_toolbar_state(&mut self, state: ToolbarState) {
        self.toolbar_state = state;
    }

    pub fn set_toolbar_visible(
        &mut self,
        visible: bool,
        hovered: Option<ToolbarButton>,
        pressed: Option<ToolbarButton>,
    ) {
        self.toolbar_state = ToolbarState {
            opacity: if visible { 1.0 } else { 0.0 },
            hovered,
            pressed,
        };
    }

    pub fn toolbar_state(&self) -> ToolbarState {
        self.toolbar_state
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn last_render_ms(&self) -> f64 {
        self.last_render_ms
    }

    pub fn static_cache_hits(&self) -> u64 {
        self.static_cache_hits
    }

    pub fn static_cache_misses(&self) -> u64 {
        self.static_cache_misses
    }

    pub fn bgra_buffer(&self) -> &[u8] {
        &self.bgra_buffer
    }

    /// Renders a single pet frame directly into the internal premultiplied BGRA8 buffer
    /// ready for Win32 `UpdateLayeredWindow(ULW_ALPHA)`.
    pub fn render_frame(&mut self, input: &PetRenderInput<'_>) -> &[u8] {
        let t0 = Instant::now();

        let manifest = &input.assets.manifest;
        let geom = StageGeometry::compute(
            self.width,
            self.height,
            manifest.canvas_width,
            manifest.canvas_height,
        );
        let fingerprint = fingerprint_assets(input.assets);

        // Clear framebuffer to fully transparent premultiplied black (0, 0, 0, 0).
        self.bgra_buffer.fill(0);

        // 1) Beat pulse ring behind character (analytical antialiased annulus directly in BGRA)
        if input.pose.ring_opacity > 0.005 {
            draw_beat_ring_bgra(&mut self.bgra_buffer, self.width, self.height, &geom, input.pose);
        }

        // Physical-space head rig transform around manifest headPivot
        let head_ox_phys = (geom.fit_x + manifest.head_pivot.x * geom.fit_scale) * geom.dpi_x;
        let head_oy_phys = (geom.fit_y + manifest.head_pivot.y * geom.fit_scale) * geom.dpi_y;
        let head_transform_phys = Transform::from_translate(-head_ox_phys, -head_oy_phys)
            .post_scale(input.pose.head_scale_x, input.pose.head_scale_y)
            .post_rotate(input.pose.head_rot_deg)
            .post_translate(
                head_ox_phys,
                head_oy_phys + input.pose.head_y * geom.dpi_y,
            );

        if let Some(base_anim_file) = input.layers.base_animation.as_deref() {
            // Full-character base animation replacement expression (moves with head rig)
            if let Some(sprite) = input.assets.get_sprite(base_anim_file) {
                let frame = sprite.frame_at(input.elapsed_ms);
                let cropped = CroppedBgra::from_tight_sprite(frame);
                let fwd = geom
                    .sprite_to_physical(frame)
                    .post_concat(head_transform_phys);
                blit_bgra_affine_bilinear(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    &cropped.bgra,
                    cropped.width,
                    cropped.height,
                    fwd,
                );
            }
        } else {
            // Standard layered character pipeline with pre-baked static back/front + head rig caches
            self.ensure_static_cache(input.assets, input.activity_mode, fingerprint, &geom);

            let layers = &manifest.layers;

            // 2) Static back layer (chair_back)
            let back_dynamic = self
                .static_cache
                .as_ref()
                .map(|c| c.static_back_dynamic)
                .unwrap_or(true);
            if back_dynamic {
                draw_dynamic_named_sprite(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    input.assets,
                    &layers.chair_back,
                    input.elapsed_ms,
                    &geom,
                    Transform::identity(),
                );
            } else if let Some(cp) = self
                .static_cache
                .as_ref()
                .and_then(|c| c.static_back.as_ref())
            {
                cp.blit_identity(&mut self.bgra_buffer, self.width, self.height);
            }

            // 3) Body rig (subtle breathing + soft counter-recoil around 50% 88% of stage)
            let body_ox_phys = (geom.stage_x + geom.stage_w * 0.5) * geom.dpi_x;
            let body_oy_phys = (geom.stage_y + geom.stage_h * 0.88) * geom.dpi_y;
            let body_transform_phys = Transform::from_translate(-body_ox_phys, -body_oy_phys)
                .post_scale(input.pose.body_scale_x, input.pose.body_scale_y)
                .post_translate(
                    body_ox_phys + input.pose.body_x * geom.dpi_x,
                    body_oy_phys + input.pose.body_y * geom.dpi_y,
                );

            let body_dynamic = self
                .static_cache
                .as_ref()
                .map(|c| c.body_dynamic)
                .unwrap_or(true);
            if body_dynamic {
                draw_dynamic_named_sprite(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    input.assets,
                    &layers.body_base,
                    input.elapsed_ms,
                    &geom,
                    body_transform_phys,
                );
            } else if let Some(cp) = self
                .static_cache
                .as_ref()
                .and_then(|c| c.body_base.as_ref())
            {
                cp.blit_transformed(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    body_transform_phys,
                );
            }

            // 4) Unified Head Rig (hair_back, head_base, blush_base, eyes, mouth, hair_front, optional headphones)
            let show_headphones = input.music_running && manifest.show_music_headphones;
            self.draw_head_rig(
                input.assets,
                &input.layers.eyes,
                &input.layers.mouth,
                show_headphones,
                input.elapsed_ms,
                fingerprint,
                &geom,
                head_transform_phys,
            );

            // 5-9) Static front layers (desk + computer + books + optional timer_clock + arms)
            let computer_file = match input.activity_mode {
                ActivityMode::Study => &layers.computer_on,
                ActivityMode::Chill => &layers.computer_off,
            };
            let arms_file = match input.activity_mode {
                ActivityMode::Study => &layers.arms_study,
                ActivityMode::Chill => &layers.arms_chill,
            };

            if !input.timer_active {
                let can_use_combined = self
                    .static_cache
                    .as_ref()
                    .map(|c| !c.static_desk_dynamic && !c.static_arms_dynamic)
                    .unwrap_or(false);
                if can_use_combined {
                    if let Some(cp) = self
                        .static_cache
                        .as_ref()
                        .and_then(|c| c.static_front_combined.as_ref())
                    {
                        cp.blit_identity(&mut self.bgra_buffer, self.width, self.height);
                    }
                } else {
                    self.draw_static_front_split(input, computer_file, arms_file, &geom);
                }
            } else {
                self.draw_static_front_split(input, computer_file, arms_file, &geom);
            }
        }

        // 10) Head-attached accessory FX overlay (with gentle floating animation)
        if let Some(overlay_file) = input.layers.overlay.as_deref() {
            self.draw_overlay(
                input.assets,
                overlay_file,
                input.elapsed_ms,
                fingerprint,
                &geom,
                input.pose,
                head_transform_phys,
            );
        }

        // 11) Floating beat musical note particles (using pre-baked `tiny-skia` glyph templates)
        if !input.pose.notes.is_empty() {
            for note in &input.pose.notes {
                draw_note_particle_fast(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    &geom,
                    &self.note_atlas,
                    note,
                );
            }
        }

        // 12) Floating pastel pet toolbar pill buttons (only when unlocked & visible, rendered on compact top strip)
        if !input.pet_locked && self.toolbar_state.opacity > 0.01 {
            self.toolbar_strip.data_mut().fill(0);
            draw_toolbar(
                &mut self.toolbar_strip,
                self.toolbar_state,
                input.music_running,
                input.elapsed_ms,
                geom.dpi_x,
                geom.dpi_y,
                &mut self.label_cache,
            );
            blend_rgba_strip_over_bgra(
                &mut self.bgra_buffer,
                self.width,
                self.toolbar_strip.data(),
                self.toolbar_strip.width(),
                self.toolbar_strip.height(),
            );
        }

        self.last_render_ms = t0.elapsed().as_secs_f64() * 1000.0;
        &self.bgra_buffer
    }

    fn draw_static_front_split(
        &mut self,
        input: &PetRenderInput<'_>,
        computer_file: &str,
        arms_file: &str,
        geom: &StageGeometry,
    ) {
        let layers = &input.assets.manifest.layers;

        // Desk + Computer + Books
        let desk_dynamic = self
            .static_cache
            .as_ref()
            .map(|c| c.static_desk_dynamic)
            .unwrap_or(true);
        if desk_dynamic {
            draw_dynamic_named_sprite(
                &mut self.bgra_buffer,
                self.width,
                self.height,
                input.assets,
                &layers.desk_surface,
                input.elapsed_ms,
                geom,
                Transform::identity(),
            );
            draw_dynamic_named_sprite(
                &mut self.bgra_buffer,
                self.width,
                self.height,
                input.assets,
                computer_file,
                input.elapsed_ms,
                geom,
                Transform::identity(),
            );
            draw_dynamic_named_sprite(
                &mut self.bgra_buffer,
                self.width,
                self.height,
                input.assets,
                &layers.books,
                input.elapsed_ms,
                geom,
                Transform::identity(),
            );
        } else if let Some(cp) = self
            .static_cache
            .as_ref()
            .and_then(|c| c.static_desk.as_ref())
        {
            cp.blit_identity(&mut self.bgra_buffer, self.width, self.height);
        }

        // Timer clock with gentle pastel pink breathing glow when active
        if input.timer_active {
            let timer_dynamic = self
                .static_cache
                .as_ref()
                .map(|c| c.timer_dynamic)
                .unwrap_or(true);
            if !timer_dynamic {
                if let Some(cp) = self
                    .static_cache
                    .as_ref()
                    .and_then(|c| c.timer_clock.as_ref())
                {
                    let cx = cp.x as f32 + (cp.width as f32) * 0.5;
                    let cy = cp.y as f32 + (cp.height as f32) * 0.5;
                    let r = ((cp.width.max(cp.height) as f32) * 0.68).max(18.0 * geom.dpi_avg());
                    draw_timer_glow_bgra(
                        &mut self.bgra_buffer,
                        self.width,
                        self.height,
                        cx,
                        cy,
                        r,
                        input.pose.timer_glow_strength,
                    );
                    cp.blit_identity(&mut self.bgra_buffer, self.width, self.height);
                }
            } else {
                draw_dynamic_named_sprite(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    input.assets,
                    &layers.timer_clock,
                    input.elapsed_ms,
                    geom,
                    Transform::identity(),
                );
            }
        }

        // Arms foreground (study vs chill)
        let arms_dynamic = self
            .static_cache
            .as_ref()
            .map(|c| c.static_arms_dynamic)
            .unwrap_or(true);
        if arms_dynamic {
            draw_dynamic_named_sprite(
                &mut self.bgra_buffer,
                self.width,
                self.height,
                input.assets,
                arms_file,
                input.elapsed_ms,
                geom,
                Transform::identity(),
            );
        } else if let Some(cp) = self
            .static_cache
            .as_ref()
            .and_then(|c| c.static_arms.as_ref())
        {
            cp.blit_identity(&mut self.bgra_buffer, self.width, self.height);
        }
    }

    fn ensure_static_cache(
        &mut self,
        assets: &LoadedCharacterAssets,
        activity_mode: ActivityMode,
        asset_fingerprint: u64,
        geom: &StageGeometry,
    ) {
        let key = StaticCacheKey {
            activity_mode,
            asset_generation: self.asset_generation,
            asset_fingerprint,
            width: self.width,
            height: self.height,
        };

        if self
            .static_cache
            .as_ref()
            .map(|c| c.key == key)
            .unwrap_or(false)
        {
            self.static_cache_hits += 1;
            return;
        }

        self.static_cache_misses += 1;
        let layers = &assets.manifest.layers;
        let computer_file = match activity_mode {
            ActivityMode::Study => &layers.computer_on,
            ActivityMode::Chill => &layers.computer_off,
        };
        let arms_file = match activity_mode {
            ActivityMode::Study => &layers.arms_study,
            ActivityMode::Chill => &layers.arms_chill,
        };

        let is_anim = |name: &str| {
            assets
                .get_sprite(name)
                .map(|s| s.is_animated())
                .unwrap_or(false)
        };

        let static_back_dynamic = is_anim(&layers.chair_back);
        let static_desk_dynamic =
            is_anim(&layers.desk_surface) || is_anim(computer_file) || is_anim(&layers.books);
        let static_arms_dynamic = is_anim(arms_file);
        let body_dynamic = is_anim(&layers.body_base);
        let timer_dynamic = is_anim(&layers.timer_clock);

        let mut scratch = Pixmap::new(self.width, self.height);

        let static_back = if !static_back_dynamic {
            bake_layers_cropped_bgra(scratch.as_mut(), assets, &[&layers.chair_back], geom)
        } else {
            None
        };

        let static_desk = if !static_desk_dynamic {
            bake_layers_cropped_bgra(
                scratch.as_mut(),
                assets,
                &[&layers.desk_surface, computer_file, &layers.books],
                geom,
            )
        } else {
            None
        };

        let static_arms = if !static_arms_dynamic {
            bake_layers_cropped_bgra(scratch.as_mut(), assets, &[arms_file], geom)
        } else {
            None
        };

        let static_front_combined = if !static_desk_dynamic && !static_arms_dynamic {
            bake_layers_cropped_bgra(
                scratch.as_mut(),
                assets,
                &[&layers.desk_surface, computer_file, &layers.books, arms_file],
                geom,
            )
        } else {
            None
        };

        let body_base = if !body_dynamic {
            bake_layers_cropped_bgra(scratch.as_mut(), assets, &[&layers.body_base], geom)
        } else {
            None
        };

        let timer_clock = if !timer_dynamic {
            bake_layers_cropped_bgra(scratch.as_mut(), assets, &[&layers.timer_clock], geom)
        } else {
            None
        };

        self.static_cache = Some(StaticLayerCache {
            key,
            static_back,
            static_back_dynamic,
            static_desk,
            static_desk_dynamic,
            static_arms,
            static_arms_dynamic,
            static_front_combined,
            body_base,
            body_dynamic,
            timer_clock,
            timer_dynamic,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_head_rig(
        &mut self,
        assets: &LoadedCharacterAssets,
        eyes: &str,
        mouth: &str,
        show_headphones: bool,
        elapsed_ms: u64,
        asset_fingerprint: u64,
        geom: &StageGeometry,
        head_transform_phys: Transform,
    ) {
        let layers = &assets.manifest.layers;
        let head_list: [&str; 7] = [
            &layers.hair_back,
            &layers.head_base,
            &layers.blush_base,
            eyes,
            mouth,
            &layers.hair_front,
            &layers.headphones,
        ];
        let count = if show_headphones { 7 } else { 6 };
        let active_layers = &head_list[..count];

        let any_animated = active_layers.iter().any(|name| {
            assets
                .get_sprite(name)
                .map(|s| s.is_animated())
                .unwrap_or(false)
        });

        if any_animated {
            for name in active_layers {
                draw_dynamic_named_sprite(
                    &mut self.bgra_buffer,
                    self.width,
                    self.height,
                    assets,
                    name,
                    elapsed_ms,
                    geom,
                    head_transform_phys,
                );
            }
            return;
        }

        let cache_hit = self
            .head_cache
            .as_ref()
            .map(|c| {
                c.key.show_headphones == show_headphones
                    && c.key.asset_generation == self.asset_generation
                    && c.key.asset_fingerprint == asset_fingerprint
                    && c.key.width == self.width
                    && c.key.height == self.height
                    && c.key.eyes == eyes
                    && c.key.mouth == mouth
            })
            .unwrap_or(false);

        if !cache_hit {
            let mut scratch = Pixmap::new(self.width, self.height);
            let baked = bake_layers_cropped_bgra(scratch.as_mut(), assets, active_layers, geom);
            self.head_cache = Some(HeadRigCache {
                key: HeadCacheKey {
                    eyes: eyes.to_string(),
                    mouth: mouth.to_string(),
                    show_headphones,
                    asset_generation: self.asset_generation,
                    asset_fingerprint,
                    width: self.width,
                    height: self.height,
                },
                pixmap: baked,
            });
        }

        if let Some(cp) = self.head_cache.as_ref().and_then(|c| c.pixmap.as_ref()) {
            cp.blit_transformed(
                &mut self.bgra_buffer,
                self.width,
                self.height,
                head_transform_phys,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_overlay(
        &mut self,
        assets: &LoadedCharacterAssets,
        overlay_file: &str,
        elapsed_ms: u64,
        asset_fingerprint: u64,
        geom: &StageGeometry,
        pose: &AnimationPose,
        head_transform_phys: Transform,
    ) {
        let Some(sprite) = assets.get_sprite(overlay_file) else {
            return;
        };
        let frame = sprite.frame_at(elapsed_ms);
        let frame_ptr = frame as *const TightSprite as usize;

        let stage_cx_phys = (geom.stage_x + geom.stage_w * 0.5) * geom.dpi_x;
        let stage_cy_phys = (geom.stage_y + geom.stage_h * 0.5) * geom.dpi_y;
        let fx_float_phys = Transform::from_translate(-stage_cx_phys, -stage_cy_phys)
            .post_scale(pose.fx_float_scale, pose.fx_float_scale)
            .post_translate(stage_cx_phys, stage_cy_phys + pose.fx_float_y * geom.dpi_y);
        let combined_phys = fx_float_phys.post_concat(head_transform_phys);

        let cache_hit = self
            .overlay_cache
            .as_ref()
            .map(|c| {
                c.key.frame_ptr == frame_ptr
                    && c.key.asset_generation == self.asset_generation
                    && c.key.asset_fingerprint == asset_fingerprint
                    && c.key.width == self.width
                    && c.key.height == self.height
                    && c.key.name == overlay_file
            })
            .unwrap_or(false);

        if !cache_hit {
            let mut scratch = Pixmap::new(self.width, self.height);
            let baked = if let Some(buf) = scratch.as_mut() {
                buf.data_mut().fill(0);
                draw_tight_sprite_to_pixmap(buf, frame, geom.sprite_to_physical(frame));
                CroppedBgra::from_rgba_pixmap(buf)
            } else {
                None
            };
            self.overlay_cache = Some(OverlayCache {
                key: OverlayCacheKey {
                    name: overlay_file.to_string(),
                    frame_ptr,
                    asset_generation: self.asset_generation,
                    asset_fingerprint,
                    width: self.width,
                    height: self.height,
                },
                pixmap: baked,
            });
        }

        if let Some(cp) = self.overlay_cache.as_ref().and_then(|c| c.pixmap.as_ref()) {
            cp.blit_transformed(
                &mut self.bgra_buffer,
                self.width,
                self.height,
                combined_phys,
            );
        }
    }
}

fn bake_layers_cropped_bgra(
    scratch: Option<&mut Pixmap>,
    assets: &LoadedCharacterAssets,
    layer_names: &[&str],
    geom: &StageGeometry,
) -> Option<CroppedBgra> {
    let buf = scratch?;
    buf.data_mut().fill(0);
    let mut drew_any = false;
    for name in layer_names {
        if let Some(sprite) = assets.get_sprite(name) {
            let frame = sprite.frame_at(0);
            draw_tight_sprite_to_pixmap(buf, frame, geom.sprite_to_physical(frame));
            drew_any = true;
        }
    }
    if !drew_any {
        return None;
    }
    CroppedBgra::from_rgba_pixmap(buf)
}

#[allow(clippy::too_many_arguments)]
fn draw_dynamic_named_sprite(
    dst_bgra: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    assets: &LoadedCharacterAssets,
    name: &str,
    elapsed_ms: u64,
    geom: &StageGeometry,
    extra_transform_phys: Transform,
) {
    if let Some(sprite) = assets.get_sprite(name) {
        let frame = sprite.frame_at(elapsed_ms);
        let cropped = CroppedBgra::from_tight_sprite(frame);
        let fwd = geom
            .sprite_to_physical(frame)
            .post_concat(extra_transform_phys);
        blit_bgra_affine_bilinear(
            dst_bgra,
            dst_w,
            dst_h,
            &cropped.bgra,
            cropped.width,
            cropped.height,
            fwd,
        );
    }
}

fn draw_tight_sprite_to_pixmap(dst: &mut Pixmap, sprite: &TightSprite, transform: Transform) {
    if sprite.width == 0 || sprite.height == 0 {
        return;
    }
    if let Some(src) = PixmapRef::from_bytes(&sprite.premul_rgba, sprite.width, sprite.height) {
        let mut paint = PixmapPaint::default();
        paint.quality = FilterQuality::Bilinear;
        dst.draw_pixmap(0, 0, src, &paint, transform, None);
    }
}

/// Blends a premultiplied RGBA8 top strip (toolbar) directly over the premultiplied BGRA8 framebuffer.
fn blend_rgba_strip_over_bgra(
    dst_bgra: &mut [u8],
    dst_w: u32,
    src_rgba: &[u8],
    strip_w: u32,
    strip_h: u32,
) {
    let w = dst_w.min(strip_w) as usize;
    let dst_row_bytes = (dst_w as usize) * 4;
    let src_row_bytes = (strip_w as usize) * 4;
    for y in 0..strip_h as usize {
        let src_row = &src_rgba[y * src_row_bytes..y * src_row_bytes + w * 4];
        let dst_row = &mut dst_bgra[y * dst_row_bytes..y * dst_row_bytes + w * 4];
        for (s, d) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            let a = s[3];
            if a != 0 {
                blend_premul_bgra_over(d, s[2].min(a), s[1].min(a), s[0].min(a), a);
            }
        }
    }
}

/// Analytical antialiased beat pulse ring (`#c084fc`) touching only the narrow annulus pixels.
fn draw_beat_ring_bgra(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    geom: &StageGeometry,
    pose: &AnimationPose,
) {
    let opacity = pose.ring_opacity.clamp(0.0, 1.0);
    if opacity <= 0.005 {
        return;
    }

    let cx = (geom.fit_x + 400.0 * geom.fit_scale) * geom.dpi_x;
    let cy = (geom.fit_y + 340.0 * geom.fit_scale) * geom.dpi_y;
    let radius = (pose.ring_radius * geom.fit_scale * geom.dpi_avg()).max(1.0);
    let half_w = ((pose.ring_stroke_width * geom.fit_scale * geom.dpi_avg()).max(1.0)) * 0.5;

    let r_out = radius + half_w + 1.0;
    let r_in = (radius - half_w - 1.0).max(0.0);
    let r_out_sq = r_out * r_out;
    let r_in_sq = r_in * r_in;

    let y0 = ((cy - r_out).floor() as i32).max(0);
    let y1 = ((cy + r_out).ceil() as i32).min(dst_h as i32 - 1);
    let max_x = dst_w as i32 - 1;
    let row_bytes = (dst_w as usize) * 4;

    // Color #c084fc -> RGB(192, 132, 252) -> BGRA(252, 132, 192)
    let cb = 252u32;
    let cg = 132u32;
    let cr = 192u32;

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5) - cy;
        let dy2 = dy * dy;
        if dy2 >= r_out_sq {
            continue;
        }
        let dx_out = (r_out_sq - dy2).sqrt();
        let row = &mut dst[(y as usize) * row_bytes..];

        let mut draw_span = |sx0: i32, sx1: i32| {
            let lx0 = sx0.max(0);
            let lx1 = sx1.min(max_x);
            if lx0 > lx1 {
                return;
            }
            for x in lx0..=lx1 {
                let dx = (x as f32 + 0.5) - cx;
                let dist = (dx * dx + dy2).sqrt();
                let cov = (half_w + 0.5 - (dist - radius).abs()).clamp(0.0, 1.0);
                if cov > 0.004 {
                    let a = (cov * opacity * 255.0).round().clamp(0.0, 255.0) as u32;
                    if a != 0 {
                        let sb = div255(cb * a) as u8;
                        let sg = div255(cg * a) as u8;
                        let sr = div255(cr * a) as u8;
                        let off = (x as usize) * 4;
                        blend_premul_bgra_over(&mut row[off..off + 4], sb, sg, sr, a as u8);
                    }
                }
            }
        };

        if dy2 < r_in_sq {
            let dx_in = (r_in_sq - dy2).sqrt();
            draw_span((cx - dx_out).floor() as i32, (cx - dx_in).ceil() as i32);
            draw_span((cx + dx_in).floor() as i32, (cx + dx_out).ceil() as i32);
        } else {
            draw_span((cx - dx_out).floor() as i32, (cx + dx_out).ceil() as i32);
        }
    }
}

/// Draws a soft pastel pink radial breathing glow behind the timer clock using `tiny-skia` on a local bbox.
fn draw_timer_glow_bgra(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    glow_strength: f32,
) {
    let s = glow_strength.clamp(0.0, 1.0);
    let inner_alpha = ((0.22 + 0.34 * s) * 255.0).round() as u8;
    let mid_alpha = ((0.10 + 0.18 * s) * 255.0).round() as u8;
    let r = (radius * (0.92 + 0.22 * s)).max(2.0);
    let dim = (r.ceil() as u32) * 2 + 2;
    let Some(mut pm) = Pixmap::new(dim, dim) else {
        return;
    };
    let local_c = (dim as f32) * 0.5;
    if let Some(shader) = RadialGradient::new(
        Point::from_xy(local_c, local_c),
        Point::from_xy(local_c, local_c),
        r,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(255, 132, 184, inner_alpha)),
            GradientStop::new(0.55, Color::from_rgba8(255, 162, 198, mid_alpha)),
            GradientStop::new(1.0, Color::from_rgba8(255, 162, 198, 0)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        if let Some(circle) = PathBuilder::from_circle(local_c, local_c, r) {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.shader = shader;
            pm.fill_path(&circle, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    let ox = (cx - local_c).round() as i32;
    let oy = (cy - local_c).round() as i32;
    let data = pm.data();
    let dst_row_bytes = (dst_w as usize) * 4;
    for ry in 0..dim as i32 {
        let dy = oy + ry;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        let dst_row = &mut dst[(dy as usize) * dst_row_bytes..];
        for rx in 0..dim as i32 {
            let dx = ox + rx;
            if dx < 0 || dx >= dst_w as i32 {
                continue;
            }
            let sidx = ((ry as usize) * (dim as usize) + (rx as usize)) * 4;
            let a = data[sidx + 3];
            if a != 0 {
                let doff = (dx as usize) * 4;
                blend_premul_bgra_over(
                    &mut dst_row[doff..doff + 4],
                    data[sidx + 2].min(a),
                    data[sidx + 1].min(a),
                    data[sidx].min(a),
                    a,
                );
            }
        }
    }
}

/// Blits a pre-baked `tiny-skia` musical note glyph template tinted to the particle's HSL RGB & opacity.
fn draw_note_particle_fast(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    geom: &StageGeometry,
    atlas: &NoteGlyphAtlas,
    note: &NoteParticleRender,
) {
    let alpha = note.opacity.clamp(0.0, 1.0);
    if alpha <= 0.01 {
        return;
    }

    let tpl = atlas.get(note.glyph);
    let size = tpl.size;
    let x_phys = (geom.stage_x + geom.stage_w * (note.x_pct / 100.0) + note.offset_x) * geom.dpi_x;
    let y_phys = (geom.stage_y + geom.stage_h * (note.y_pct / 100.0) + note.offset_y) * geom.dpi_y;
    let scale = note.scale.max(0.1) * geom.dpi_avg();

    let fwd = Transform::from_translate(-tpl.center, -tpl.center)
        .post_scale(scale, scale)
        .post_rotate(note.rot_deg)
        .post_translate(x_phys, y_phys);

    let Some(inv) = fwd.invert() else {
        return;
    };

    let half_extent = tpl.center * scale * 1.42 + 2.0;
    let x0 = ((x_phys - half_extent).floor() as i32).max(0);
    let y0 = ((y_phys - half_extent).floor() as i32).max(0);
    let x1 = ((x_phys + half_extent).ceil() as i32).min(dst_w as i32 - 1);
    let y1 = ((y_phys + half_extent).ceil() as i32).min(dst_h as i32 - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }

    let alpha_q = (alpha * 255.0).round() as u32;
    let halo_q = (alpha * 215.0).round() as u32;
    let nb = note.rgb[2] as u32;
    let ng = note.rgb[1] as u32;
    let nr = note.rgb[0] as u32;

    let du_dx = (inv.sx * 65536.0).round() as i32;
    let dv_dx = (inv.ky * 65536.0).round() as i32;
    let size_i = size as i32;
    let size_u = size as usize;
    let dst_row_bytes = (dst_w as usize) * 4;

    for dy in y0..=y1 {
        let py = dy as f32 + 0.5;
        let px0 = x0 as f32 + 0.5;
        let mut u = ((inv.sx * px0 + inv.kx * py + inv.tx - 0.5) * 65536.0).round() as i32;
        let mut v = ((inv.ky * px0 + inv.sy * py + inv.ty - 0.5) * 65536.0).round() as i32;
        let dst_row = &mut dst[(dy as usize) * dst_row_bytes..];

        for dx in x0..=x1 {
            let ix = u >> 16;
            let iy = v >> 16;
            if ix >= 0 && ix < size_i - 1 && iy >= 0 && iy < size_i - 1 {
                let idx0 = (iy as usize) * size_u + (ix as usize);
                let idx1 = idx0 + size_u;
                let (h00, f00) = tpl.masks[idx0];
                let (h10, f10) = tpl.masks[idx0 + 1];
                let (h01, f01) = tpl.masks[idx1];
                let (h11, f11) = tpl.masks[idx1 + 1];

                if (h00 | h10 | h01 | h11 | f00 | f10 | f01 | f11) != 0 {
                    let fx = ((u >> 8) & 0xFF) as u32;
                    let fy = ((v >> 8) & 0xFF) as u32;
                    let ifx = 256 - fx;
                    let ify = 256 - fy;
                    let w00 = ifx * ify;
                    let w10 = fx * ify;
                    let w01 = ifx * fy;
                    let w11 = fx * fy;

                    let ha = div255(
                        ((h00 as u32 * w00 + h10 as u32 * w10 + h01 as u32 * w01 + h11 as u32 * w11 + 32768)
                            >> 16)
                            * halo_q,
                    );
                    let fa = div255(
                        ((f00 as u32 * w00 + f10 as u32 * w10 + f01 as u32 * w01 + f11 as u32 * w11 + 32768)
                            >> 16)
                            * alpha_q,
                    );

                    // Composite colored note fill over white halo
                    let inv_fa = 255 - fa;
                    let out_a = (fa + div255(ha * inv_fa)).min(255);
                    if out_a != 0 {
                        let ha_rem = div255(ha * inv_fa);
                        let sb = (div255(nb * fa) + ha_rem).min(out_a) as u8;
                        let sg = (div255(ng * fa) + ha_rem).min(out_a) as u8;
                        let sr = (div255(nr * fa) + ha_rem).min(out_a) as u8;
                        let doff = (dx as usize) * 4;
                        blend_premul_bgra_over(&mut dst_row[doff..doff + 4], sb, sg, sr, out_a as u8);
                    }
                }
            }
            u = u.wrapping_add(du_dx);
            v = v.wrapping_add(dv_dx);
        }
    }
}

fn build_note_glyph_path(glyph: char) -> Option<Path> {
    let mut pb = PathBuilder::new();
    match glyph {
        '♪' => {
            add_oval(&mut pb, -3.0, 5.5, 4.2, 3.0);
            pb.move_to(0.6, 5.0);
            pb.line_to(0.6, -8.5);
            pb.cubic_to(6.2, -7.2, 8.2, -2.5, 4.8, 1.4);
        }
        '♩' => {
            add_oval(&mut pb, -2.5, 5.5, 4.2, 3.0);
            pb.move_to(1.1, 5.0);
            pb.line_to(1.1, -9.0);
        }
        '♫' => {
            add_oval(&mut pb, -6.5, 6.2, 3.6, 2.7);
            add_oval(&mut pb, 3.8, 4.4, 3.6, 2.7);
            pb.move_to(-3.4, 5.8);
            pb.line_to(-3.4, -7.2);
            pb.line_to(6.9, -9.2);
            pb.line_to(6.9, 4.0);
        }
        '♬' => {
            add_oval(&mut pb, -6.8, 6.4, 3.5, 2.6);
            add_oval(&mut pb, 3.8, 4.6, 3.5, 2.6);
            pb.move_to(-3.8, 6.0);
            pb.line_to(-3.8, -7.4);
            pb.line_to(6.8, -9.4);
            pb.line_to(6.8, 4.2);
            pb.move_to(-3.8, -3.6);
            pb.line_to(6.8, -5.6);
        }
        _ => {
            let outer = 9.0_f32;
            let ctrl = 2.0_f32;
            pb.move_to(0.0, -outer);
            pb.quad_to(ctrl, -ctrl, outer, 0.0);
            pb.quad_to(ctrl, ctrl, 0.0, outer);
            pb.quad_to(-ctrl, ctrl, -outer, 0.0);
            pb.quad_to(-ctrl, -ctrl, 0.0, -outer);
            pb.close();
        }
    }
    pb.finish()
}

fn add_oval(pb: &mut PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32) {
    let kx = 0.552_284_8 * rx;
    let ky = 0.552_284_8 * ry;
    pb.move_to(cx, cy - ry);
    pb.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
    pb.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
    pb.close();
}

/// Fast deterministic FNV-1a fingerprint to detect reloaded/replaced character assets automatically.
fn fingerprint_assets(assets: &LoadedCharacterAssets) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    let mix_u64 = |h: &mut u64, v: u64| {
        *h ^= v;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    let mix_str = |h: &mut u64, s: &str| {
        for &b in s.as_bytes() {
            *h ^= u64::from(b);
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        *h ^= 0xff;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };

    mix_str(&mut h, &assets.source_hint);
    mix_str(&mut h, &assets.manifest.pack_name);
    mix_str(&mut h, &assets.manifest.version);
    mix_u64(&mut h, u64::from(assets.manifest.canvas_width));
    mix_u64(&mut h, u64::from(assets.manifest.canvas_height));
    mix_u64(&mut h, assets.total_decoded_bytes as u64);
    mix_u64(&mut h, assets.sprites.len() as u64);
    mix_str(&mut h, &assets.manifest.layers.chair_back);
    mix_str(&mut h, &assets.manifest.layers.body_base);
    mix_str(&mut h, &assets.manifest.layers.desk_surface);
    mix_str(&mut h, &assets.manifest.layers.arms_chill);
    mix_str(&mut h, &assets.manifest.layers.arms_study);
    h
}
