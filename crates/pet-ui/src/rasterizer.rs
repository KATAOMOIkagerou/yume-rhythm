use egui::epaint::{ClippedPrimitive, ImageData, Mesh, Primitive, TextureId, Vertex};
use egui::TexturesDelta;
use std::collections::HashMap;

/// Stored texture in premultiplied sRGBA8 (`[r, g, b, a]` per texel).
struct CpuTexture {
    width: usize,
    height: usize,
    /// Premultiplied sRGBA8 pixels (`width * height * 4`).
    pixels: Vec<u8>,
}

impl CpuTexture {
    fn from_image_data(image: &ImageData) -> Self {
        let [width, height] = image.size();
        let mut pixels = Vec::with_capacity(width * height * 4);
        match image {
            ImageData::Color(color) => {
                for c in &color.pixels {
                    pixels.extend_from_slice(&c.to_array());
                }
            }
            ImageData::Font(font) => {
                for c in font.srgba_pixels(None) {
                    pixels.extend_from_slice(&c.to_array());
                }
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn update_subregion(&mut self, pos: [usize; 2], image: &ImageData) {
        let [w, h] = image.size();
        let [px, py] = pos;
        if px + w > self.width || py + h > self.height {
            return;
        }
        match image {
            ImageData::Color(color) => {
                for row in 0..h {
                    let dst_off = ((py + row) * self.width + px) * 4;
                    let src_off = row * w;
                    for col in 0..w {
                        let c = color.pixels[src_off + col].to_array();
                        let d = dst_off + col * 4;
                        self.pixels[d..d + 4].copy_from_slice(&c);
                    }
                }
            }
            ImageData::Font(font) => {
                let mut iter = font.srgba_pixels(None);
                for row in 0..h {
                    let dst_off = ((py + row) * self.width + px) * 4;
                    for col in 0..w {
                        if let Some(c) = iter.next() {
                            let d = dst_off + col * 4;
                            self.pixels[d..d + 4].copy_from_slice(&c.to_array());
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn sample_nearest(&self, u: f32, v: f32) -> [u8; 4] {
        if self.width == 0 || self.height == 0 {
            return [255, 255, 255, 255];
        }
        let x = ((u * self.width as f32).floor() as isize).clamp(0, self.width as isize - 1) as usize;
        let y = ((v * self.height as f32).floor() as isize).clamp(0, self.height as isize - 1) as usize;
        let idx = (y * self.width + x) * 4;
        [
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ]
    }

    #[inline(always)]
    fn sample_bilinear(&self, u: f32, v: f32) -> [u8; 4] {
        if self.width == 0 || self.height == 0 {
            return [255, 255, 255, 255];
        }
        let fx = (u * self.width as f32 - 0.5).clamp(0.0, (self.width - 1) as f32);
        let fy = (v * self.height as f32 - 0.5).clamp(0.0, (self.height - 1) as f32);
        let x0 = fx as usize;
        let y0 = fy as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let wx = ((fx - x0 as f32) * 256.0) as u32;
        let wy = ((fy - y0 as f32) * 256.0) as u32;
        let inv_wx = 256 - wx;
        let inv_wy = 256 - wy;

        let i00 = (y0 * self.width + x0) * 4;
        let i10 = (y0 * self.width + x1) * 4;
        let i01 = (y1 * self.width + x0) * 4;
        let i11 = (y1 * self.width + x1) * 4;

        let w00 = inv_wx * inv_wy;
        let w10 = wx * inv_wy;
        let w01 = inv_wx * wy;
        let w11 = wx * wy;

        let mut out = [0u8; 4];
        for c in 0..4 {
            let val = self.pixels[i00 + c] as u32 * w00
                + self.pixels[i10 + c] as u32 * w10
                + self.pixels[i01 + c] as u32 * w01
                + self.pixels[i11 + c] as u32 * w11;
            out[c] = ((val + 32_768) >> 16).min(255) as u8;
        }
        out
    }
}

/// Pure-Rust zero-GPU CPU software rasterizer for `egui` tessellated primitives.
/// Writes directly into a 32-bit top-down BGRA framebuffer (compatible with Win32 `DIBSection` + `BitBlt`).
pub struct SoftwareRasterizer {
    textures: HashMap<TextureId, CpuTexture>,
}

impl SoftwareRasterizer {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Applies `egui::TexturesDelta::set` updates before rendering a frame.
    pub fn apply_textures_set(&mut self, delta: &TexturesDelta) {
        for (id, image_delta) in &delta.set {
            match image_delta.pos {
                Some(pos) => {
                    if let Some(tex) = self.textures.get_mut(id) {
                        tex.update_subregion(pos, &image_delta.image);
                    } else {
                        self.textures
                            .insert(*id, CpuTexture::from_image_data(&image_delta.image));
                    }
                }
                None => {
                    self.textures
                        .insert(*id, CpuTexture::from_image_data(&image_delta.image));
                }
            }
        }
    }

    /// Applies `egui::TexturesDelta::free` releases after rendering a frame.
    pub fn apply_textures_free(&mut self, delta: &TexturesDelta) {
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    /// Total CPU memory occupied by cached textures (font atlas, etc.).
    pub fn texture_bytes(&self) -> usize {
        self.textures.values().map(|t| t.pixels.len()).sum()
    }

    /// Clears `bgra_buf` with `clear_rgba` and rasterizes all `primitives` into it.
    pub fn paint(
        &self,
        bgra_buf: &mut [u8],
        width: usize,
        height: usize,
        pixels_per_point: f32,
        clear_rgba: [u8; 4],
        primitives: &[ClippedPrimitive],
    ) {
        if width == 0 || height == 0 || bgra_buf.len() < width * height * 4 {
            return;
        }

        // Fast fill background in BGRA order.
        let clear_bgra = [clear_rgba[2], clear_rgba[1], clear_rgba[0], clear_rgba[3]];
        for px in bgra_buf[..width * height * 4].chunks_exact_mut(4) {
            px.copy_from_slice(&clear_bgra);
        }

        for clipped in primitives {
            let Primitive::Mesh(mesh) = &clipped.primitive else {
                continue;
            };
            if mesh.indices.len() < 3 || mesh.vertices.is_empty() {
                continue;
            }

            let clip_min_x = (clipped.clip_rect.min.x * pixels_per_point)
                .floor()
                .max(0.0) as i32;
            let clip_min_y = (clipped.clip_rect.min.y * pixels_per_point)
                .floor()
                .max(0.0) as i32;
            let clip_max_x = (clipped.clip_rect.max.x * pixels_per_point)
                .ceil()
                .min(width as f32) as i32;
            let clip_max_y = (clipped.clip_rect.max.y * pixels_per_point)
                .ceil()
                .min(height as f32) as i32;

            if clip_min_x >= clip_max_x || clip_min_y >= clip_max_y {
                continue;
            }

            let clip = [clip_min_x, clip_min_y, clip_max_x, clip_max_y];
            let tex = self.textures.get(&mesh.texture_id);
            self.rasterize_mesh(bgra_buf, width, pixels_per_point, clip, mesh, tex);
        }
    }

    fn rasterize_mesh(
        &self,
        buf: &mut [u8],
        fb_width: usize,
        ppp: f32,
        clip: [i32; 4],
        mesh: &Mesh,
        tex: Option<&CpuTexture>,
    ) {
        let verts = &mesh.vertices;
        let idx = &mesh.indices;
        let mut i = 0usize;

        while i + 2 < idx.len() {
            // Fast-path check: axis-aligned quad made of two triangles (6 indices).
            if i + 5 < idx.len() {
                if let Some(quad) = detect_axis_aligned_quad(verts, &idx[i..i + 6], ppp) {
                    if quad.is_solid_untextured {
                        fill_solid_rect(buf, fb_width, clip, quad.rect, quad.color);
                    } else if let Some(t) = tex {
                        fill_textured_rect(buf, fb_width, clip, quad.rect, quad.uv_rect, quad.color, t);
                    } else {
                        fill_solid_rect(buf, fb_width, clip, quad.rect, quad.color);
                    }
                    i += 6;
                    continue;
                }
            }

            let i0 = idx[i] as usize;
            let i1 = idx[i + 1] as usize;
            let i2 = idx[i + 2] as usize;
            i += 3;

            if i0 >= verts.len() || i1 >= verts.len() || i2 >= verts.len() {
                continue;
            }
            rasterize_triangle(buf, fb_width, ppp, clip, &verts[i0], &verts[i1], &verts[i2], tex);
        }
    }
}

impl Default for SoftwareRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

struct AxisAlignedQuad {
    rect: [f32; 4],    // [x0, y0, x1, y1] in physical px
    uv_rect: [f32; 4], // [u0, v0, u1, v1]
    color: [u8; 4],    // premultiplied RGBA
    is_solid_untextured: bool,
}

/// Detects whether 2 consecutive triangles (`6` indices) form a constant-color axis-aligned
/// rectangle (covers 95%+ of `egui` background panels, buttons, bars, and text glyph quads!).
#[inline]
fn detect_axis_aligned_quad(verts: &[Vertex], tri_pair: &[u32], ppp: f32) -> Option<AxisAlignedQuad> {
    let i0 = tri_pair[0] as usize;
    let i1 = tri_pair[1] as usize;
    let i2 = tri_pair[2] as usize;
    let i3 = tri_pair[3] as usize;
    let i4 = tri_pair[4] as usize;
    let i5 = tri_pair[5] as usize;

    let n = verts.len();
    if i0 >= n || i1 >= n || i2 >= n || i3 >= n || i4 >= n || i5 >= n {
        return None;
    }

    // Standard egui quad winding uses 4 distinct vertices in pattern (a, b, c, c, b, d) or (a, b, c, a, c, d).
    let mut uniq = [0usize; 4];
    let mut count = 0usize;
    for &id in &[i0, i1, i2, i3, i4, i5] {
        if !uniq[..count].contains(&id) {
            if count == 4 {
                return None;
            }
            uniq[count] = id;
            count += 1;
        }
    }
    if count != 4 {
        return None;
    }

    let v = [verts[uniq[0]], verts[uniq[1]], verts[uniq[2]], verts[uniq[3]]];
    let c0 = v[0].color.to_array();
    if v[1].color.to_array() != c0 || v[2].color.to_array() != c0 || v[3].color.to_array() != c0 {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for vert in &v {
        min_x = min_x.min(vert.pos.x);
        max_x = max_x.max(vert.pos.x);
        min_y = min_y.min(vert.pos.y);
        max_y = max_y.max(vert.pos.y);
    }

    let eps = 1e-3;
    if (max_x - min_x) < eps || (max_y - min_y) < eps {
        return None;
    }

    // Verify all 4 vertices sit on the 4 distinct corners of the bounding box.
    let mut corners = [None; 4]; // 0: TL, 1: TR, 2: BL, 3: BR
    for vert in &v {
        let on_left = (vert.pos.x - min_x).abs() <= eps;
        let on_right = (vert.pos.x - max_x).abs() <= eps;
        let on_top = (vert.pos.y - min_y).abs() <= eps;
        let on_bottom = (vert.pos.y - max_y).abs() <= eps;
        let slot = match (on_left, on_right, on_top, on_bottom) {
            (true, false, true, false) => 0,
            (false, true, true, false) => 1,
            (true, false, false, true) => 2,
            (false, true, false, true) => 3,
            _ => return None,
        };
        if corners[slot].is_some() {
            return None;
        }
        corners[slot] = Some(vert);
    }

    let tl = corners[0]?;
    let tr = corners[1]?;
    let bl = corners[2]?;
    let br = corners[3]?;

    // Check untextured white-pixel UV (egui uses uv == (0, 0) for solid shapes).
    let is_solid_untextured = is_zero_uv(tl) && is_zero_uv(tr) && is_zero_uv(bl) && is_zero_uv(br);
    if !is_solid_untextured {
        // Verify UVs are axis-aligned (no rotation/flip mismatch).
        let uv_eps = 1e-5;
        if (tl.uv.x - bl.uv.x).abs() > uv_eps
            || (tr.uv.x - br.uv.x).abs() > uv_eps
            || (tl.uv.y - tr.uv.y).abs() > uv_eps
            || (bl.uv.y - br.uv.y).abs() > uv_eps
        {
            return None;
        }
    }

    Some(AxisAlignedQuad {
        rect: [min_x * ppp, min_y * ppp, max_x * ppp, max_y * ppp],
        uv_rect: [tl.uv.x, tl.uv.y, br.uv.x, br.uv.y],
        color: c0,
        is_solid_untextured,
    })
}

#[inline(always)]
fn is_zero_uv(v: &Vertex) -> bool {
    v.uv.x == 0.0 && v.uv.y == 0.0
}

#[inline(always)]
fn div255(x: u32) -> u8 {
    (((x + 128) + ((x + 128) >> 8)) >> 8) as u8
}

/// Blends premultiplied sRGBA `src` over opaque BGRA pixel slice `dst[0..4]`.
#[inline(always)]
fn blend_premul_bgra(dst: &mut [u8], src_r: u8, src_g: u8, src_b: u8, src_a: u8) {
    if src_a == 255 {
        dst[0] = src_b;
        dst[1] = src_g;
        dst[2] = src_r;
        dst[3] = 255;
    } else if src_a != 0 {
        let inv_a = (255 - src_a) as u32;
        dst[0] = src_b.saturating_add(div255(dst[0] as u32 * inv_a));
        dst[1] = src_g.saturating_add(div255(dst[1] as u32 * inv_a));
        dst[2] = src_r.saturating_add(div255(dst[2] as u32 * inv_a));
        dst[3] = 255;
    }
}

fn fill_solid_rect(buf: &mut [u8], fb_width: usize, clip: [i32; 4], rect: [f32; 4], color: [u8; 4]) {
    let [sr, sg, sb, sa] = color;
    if sa == 0 && sr == 0 && sg == 0 && sb == 0 {
        return;
    }

    let x0 = ((rect[0] - 0.5).ceil() as i32).max(clip[0]);
    let y0 = ((rect[1] - 0.5).ceil() as i32).max(clip[1]);
    let x1 = ((rect[2] - 0.5).ceil() as i32).min(clip[2]);
    let y1 = ((rect[3] - 0.5).ceil() as i32).min(clip[3]);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    if sa == 255 {
        let bgra = [sb, sg, sr, 255];
        for y in y0..y1 {
            let row_start = (y as usize * fb_width + x0 as usize) * 4;
            let row_end = (y as usize * fb_width + x1 as usize) * 4;
            for px in buf[row_start..row_end].chunks_exact_mut(4) {
                px.copy_from_slice(&bgra);
            }
        }
    } else {
        let inv_a = (255 - sa) as u32;
        for y in y0..y1 {
            let row_start = (y as usize * fb_width + x0 as usize) * 4;
            let row_end = (y as usize * fb_width + x1 as usize) * 4;
            for px in buf[row_start..row_end].chunks_exact_mut(4) {
                px[0] = sb.saturating_add(div255(px[0] as u32 * inv_a));
                px[1] = sg.saturating_add(div255(px[1] as u32 * inv_a));
                px[2] = sr.saturating_add(div255(px[2] as u32 * inv_a));
                px[3] = 255;
            }
        }
    }
}

fn fill_textured_rect(
    buf: &mut [u8],
    fb_width: usize,
    clip: [i32; 4],
    rect: [f32; 4],
    uv: [f32; 4],
    color: [u8; 4],
    tex: &CpuTexture,
) {
    let [vr, vg, vb, va] = color;
    if va == 0 && vr == 0 && vg == 0 && vb == 0 {
        return;
    }

    let x0 = (rect[0].floor() as i32).max(clip[0]);
    let y0 = (rect[1].floor() as i32).max(clip[1]);
    let x1 = (rect[2].ceil() as i32).min(clip[2]);
    let y1 = (rect[3].ceil() as i32).min(clip[3]);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let rw = rect[2] - rect[0];
    let rh = rect[3] - rect[1];
    if rw <= 1e-6 || rh <= 1e-6 {
        return;
    }
    let inv_w = 1.0 / rw;
    let inv_h = 1.0 / rh;
    let du = uv[2] - uv[0];
    let dv = uv[3] - uv[1];

    // Check if texel grid aligns almost 1:1 with physical pixels (typical for crisp font glyphs).
    let tex_span_x = (du.abs() * tex.width as f32) - rw;
    let tex_span_y = (dv.abs() * tex.height as f32) - rh;
    let use_nearest = tex_span_x.abs() < 0.25 && tex_span_y.abs() < 0.25;

    for y in y0..y1 {
        let ty = ((y as f32 + 0.5) - rect[1]) * inv_h;
        if !(0.0..=1.0).contains(&ty) {
            continue;
        }
        let v = uv[1] + dv * ty;
        let row_base = y as usize * fb_width;

        for x in x0..x1 {
            let tx = ((x as f32 + 0.5) - rect[0]) * inv_w;
            if !(0.0..=1.0).contains(&tx) {
                continue;
            }
            let u = uv[0] + du * tx;
            let t = if use_nearest {
                tex.sample_nearest(u, v)
            } else {
                tex.sample_bilinear(u, v)
            };
            if t[3] == 0 && t[0] == 0 && t[1] == 0 && t[2] == 0 {
                continue;
            }
            let sr = div255(vr as u32 * t[0] as u32);
            let sg = div255(vg as u32 * t[1] as u32);
            let sb = div255(vb as u32 * t[2] as u32);
            let sa = div255(va as u32 * t[3] as u32);
            let off = (row_base + x as usize) * 4;
            blend_premul_bgra(&mut buf[off..off + 4], sr, sg, sb, sa);
        }
    }
}

#[inline(always)]
fn edge_fn(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}

#[inline(always)]
fn is_top_left(ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    (ay == by && bx < ax) || (by > ay)
}

#[allow(clippy::too_many_arguments)]
fn rasterize_triangle(
    buf: &mut [u8],
    fb_width: usize,
    ppp: f32,
    clip: [i32; 4],
    v0: &Vertex,
    v1: &Vertex,
    v2: &Vertex,
    tex: Option<&CpuTexture>,
) {
    let x0 = v0.pos.x * ppp;
    let y0 = v0.pos.y * ppp;
    let mut x1 = v1.pos.x * ppp;
    let mut y1 = v1.pos.y * ppp;
    let mut x2 = v2.pos.x * ppp;
    let mut y2 = v2.pos.y * ppp;

    let c0 = v0.color.to_array();
    let mut c1 = v1.color.to_array();
    let mut c2 = v2.color.to_array();
    let u0 = v0.uv.x;
    let w0_v = v0.uv.y;
    let mut u1 = v1.uv.x;
    let mut w1_v = v1.uv.y;
    let mut u2 = v2.uv.x;
    let mut w2_v = v2.uv.y;

    let mut area = edge_fn(x0, y0, x1, y1, x2, y2);
    if area.abs() < 1e-5 {
        return;
    }
    // Ensure positive winding (area > 0) so edge_fn >= 0 is always inside.
    if area < 0.0 {
        std::mem::swap(&mut x1, &mut x2);
        std::mem::swap(&mut y1, &mut y2);
        std::mem::swap(&mut c1, &mut c2);
        std::mem::swap(&mut u1, &mut u2);
        std::mem::swap(&mut w1_v, &mut w2_v);
        area = -area;
    }

    let min_x = (x0.min(x1).min(x2).floor() as i32).max(clip[0]);
    let max_x = (x0.max(x1).max(x2).ceil() as i32).min(clip[2]);
    let min_y = (y0.min(y1).min(y2).floor() as i32).max(clip[1]);
    let max_y = (y0.max(y1).max(y2).ceil() as i32).min(clip[3]);
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let flat_color = c0 == c1 && c1 == c2;
    let untextured = tex.is_none()
        || (u0 == 0.0 && w0_v == 0.0 && u1 == 0.0 && w1_v == 0.0 && u2 == 0.0 && w2_v == 0.0);

    // Top-left bias prevents double-blending shared internal edges of translucent polygons.
    let bias0 = if is_top_left(x1, y1, x2, y2) { 0.0 } else { -1e-5 };
    let bias1 = if is_top_left(x2, y2, x0, y0) { 0.0 } else { -1e-5 };
    let bias2 = if is_top_left(x0, y0, x1, y1) { 0.0 } else { -1e-5 };

    let inv_area = 1.0 / area;

    // Edge incremental steps
    let dx0 = y2 - y1;
    let dy0 = x1 - x2;
    let dx1 = y0 - y2;
    let dy1 = x2 - x0;
    let dx2 = y1 - y0;
    let dy2 = x0 - x1;

    let start_x = min_x as f32 + 0.5;
    let start_y = min_y as f32 + 0.5;
    let mut row_e0 = edge_fn(x1, y1, x2, y2, start_x, start_y);
    let mut row_e1 = edge_fn(x2, y2, x0, y0, start_x, start_y);
    let mut row_e2 = edge_fn(x0, y0, x1, y1, start_x, start_y);

    if flat_color && untextured {
        let [sr, sg, sb, sa] = c0;
        if sa == 0 && sr == 0 && sg == 0 && sb == 0 {
            return;
        }
        let inv_dx0 = if dx0.abs() > 1e-6 { Some(1.0 / dx0) } else { None };
        let inv_dx1 = if dx1.abs() > 1e-6 { Some(1.0 / dx1) } else { None };
        let inv_dx2 = if dx2.abs() > 1e-6 { Some(1.0 / dx2) } else { None };
        let bgra = [sb, sg, sr, 255];
        let inv_a = (255 - sa) as u32;
        let span_max = (max_x - min_x) as f32;

        for y in min_y..max_y {
            let mut t_lo = 0.0_f32;
            let mut t_hi = span_max;

            // Intersect the 3 half-planes `row_e + t * dx >= bias` on this scanline in O(1).
            let edges = [
                (row_e0, dx0, inv_dx0, bias0),
                (row_e1, dx1, inv_dx1, bias1),
                (row_e2, dx2, inv_dx2, bias2),
            ];
            let mut visible = true;
            for (re, dx, inv_dx, bias) in edges {
                if let Some(inv) = inv_dx {
                    let t = (bias - re) * inv;
                    if dx > 0.0 {
                        if t > t_lo {
                            t_lo = t;
                        }
                    } else if t < t_hi {
                        t_hi = t;
                    }
                } else if re < bias {
                    visible = false;
                    break;
                }
            }

            if visible {
                let sx0 = (min_x + t_lo.ceil() as i32).max(min_x);
                let sx1 = (min_x + t_hi.floor() as i32 + 1).min(max_x);
                if sx0 < sx1 {
                    let row_start = (y as usize * fb_width + sx0 as usize) * 4;
                    let row_end = (y as usize * fb_width + sx1 as usize) * 4;
                    if sa == 255 {
                        for px in buf[row_start..row_end].chunks_exact_mut(4) {
                            px.copy_from_slice(&bgra);
                        }
                    } else {
                        for px in buf[row_start..row_end].chunks_exact_mut(4) {
                            px[0] = sb.saturating_add(div255(px[0] as u32 * inv_a));
                            px[1] = sg.saturating_add(div255(px[1] as u32 * inv_a));
                            px[2] = sr.saturating_add(div255(px[2] as u32 * inv_a));
                            px[3] = 255;
                        }
                    }
                }
            }

            row_e0 += dy0;
            row_e1 += dy1;
            row_e2 += dy2;
        }
        return;
    }

    for y in min_y..max_y {
        let mut e0 = row_e0;
        let mut e1 = row_e1;
        let mut e2 = row_e2;
        let row_base = y as usize * fb_width;
        for x in min_x..max_x {
            if e0 >= bias0 && e1 >= bias1 && e2 >= bias2 {
                let b0 = e0 * inv_area;
                let b1 = e1 * inv_area;
                let b2 = e2 * inv_area;

                let (vr, vg, vb, va) = if flat_color {
                    (c0[0], c0[1], c0[2], c0[3])
                } else {
                    (
                        (b0 * c0[0] as f32 + b1 * c1[0] as f32 + b2 * c2[0] as f32).clamp(0.0, 255.0) as u8,
                        (b0 * c0[1] as f32 + b1 * c1[1] as f32 + b2 * c2[1] as f32).clamp(0.0, 255.0) as u8,
                        (b0 * c0[2] as f32 + b1 * c1[2] as f32 + b2 * c2[2] as f32).clamp(0.0, 255.0) as u8,
                        (b0 * c0[3] as f32 + b1 * c1[3] as f32 + b2 * c2[3] as f32).clamp(0.0, 255.0) as u8,
                    )
                };

                let (sr, sg, sb, sa) = if untextured {
                    (vr, vg, vb, va)
                } else if let Some(t) = tex {
                    let u = b0 * u0 + b1 * u1 + b2 * u2;
                    let v = b0 * w0_v + b1 * w1_v + b2 * w2_v;
                    let tx = t.sample_bilinear(u, v);
                    (
                        div255(vr as u32 * tx[0] as u32),
                        div255(vg as u32 * tx[1] as u32),
                        div255(vb as u32 * tx[2] as u32),
                        div255(va as u32 * tx[3] as u32),
                    )
                } else {
                    (vr, vg, vb, va)
                };

                let off = (row_base + x as usize) * 4;
                blend_premul_bgra(&mut buf[off..off + 4], sr, sg, sb, sa);
            }
            e0 += dx0;
            e1 += dx1;
            e2 += dx2;
        }
        row_e0 += dy0;
        row_e1 += dy1;
        row_e2 += dy2;
    }
}
