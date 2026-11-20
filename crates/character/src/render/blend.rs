//! 每帧都要做的两种贴图：整数位置贴图、带仿射变换的双线性贴图。
//!
//! tiny-skia 的通用管线在这两种场景下约 7–15 ns/像素，200% 缩放时一帧要 8 ms 以上，
//! 所以这里用整数运算单独实现：全透明像素直接跳过，不透明像素直接拷贝，
//! 其余用 SWAR（一个 u32 同时算两个通道）做预乘 alpha 混合。
//! 像素格式与 `sprite` 一致：预乘 BGRA，按小端读成 u32 时 alpha 在最高字节。

use tiny_skia::{Pixmap, Transform};

const LOW: u32 = 0x00FF_00FF;

#[inline]
fn load(data: &[u8], index: usize) -> u32 {
    let i = index * 4;
    u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
}

#[inline]
fn store(data: &mut [u8], index: usize, px: u32) {
    data[index * 4..index * 4 + 4].copy_from_slice(&px.to_le_bytes());
}

/// 各通道乘以 `w / 256`（`w` ∈ 0..=256）。
#[inline]
fn scale(px: u32, w: u32) -> u32 {
    let rb = (((px & LOW) * w) >> 8) & LOW;
    let ag = ((((px >> 8) & LOW) * w) >> 8) & LOW;
    rb | (ag << 8)
}

/// `p` 与 `q` 按 `w / 256` 插值。
#[inline]
fn lerp(p: u32, q: u32, w: u32) -> u32 {
    let iw = 256 - w;
    let rb = (((p & LOW) * iw + (q & LOW) * w) >> 8) & LOW;
    let ag = ((((p >> 8) & LOW) * iw + ((q >> 8) & LOW) * w) >> 8) & LOW;
    rb | (ag << 8)
}

/// 预乘 src-over。
#[inline]
fn over(src: u32, dst: u32) -> u32 {
    let sa = src >> 24;
    match sa {
        255 => src,
        0 => dst,
        _ => src + scale(dst, 256 - sa),
    }
}

/// 把 `src` 以左上角 `(x, y)` 贴到 `dst` 上。
pub fn blit_over(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32) {
    let (dw, dh) = (to_i32(dst.width()), to_i32(dst.height()));
    let (sw, sh) = (to_i32(src.width()), to_i32(src.height()));
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + sw).min(dw), (y + sh).min(dh));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let src_data = src.data();
    let dst_data = dst.data_mut();
    for row in y0..y1 {
        let s_row = to_usize(row - y) * to_usize(sw);
        let d_row = to_usize(row) * to_usize(dw);
        for col in x0..x1 {
            let s = load(src_data, s_row + to_usize(col - x));
            let sa = s >> 24;
            if sa == 0 {
                continue;
            }
            let di = d_row + to_usize(col);
            let d = if sa == 255 {
                s
            } else {
                over(s, load(dst_data, di))
            };
            store(dst_data, di, d);
        }
    }
}

/// 以仿射变换 `to_dst`（源像素坐标 → 目标像素坐标）把 `src` 双线性采样后贴到 `dst`。
/// `opacity` ∈ 0–1。
pub fn draw_affine(dst: &mut Pixmap, src: &Pixmap, to_dst: Transform, opacity: f32) {
    if opacity <= 0.0 {
        return;
    }
    let Some(inv) = to_dst.invert() else { return };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "不透明度 0–1"
    )]
    let op = (opacity.clamp(0.0, 1.0) * 256.0).round() as u32;
    let (sw, sh) = (to_i32(src.width()), to_i32(src.height()));
    let Some((x0, y0, x1, y1)) = dest_bounds(dst, to_dst, sw, sh) else {
        return;
    };

    const ONE: f32 = 65536.0;
    #[allow(clippy::cast_possible_truncation, reason = "定点数，坐标远小于 32768")]
    let (step_x, step_y) = ((inv.sx * ONE) as i32, (inv.ky * ONE) as i32);
    let src_data = src.data();
    let stride = to_usize(sw) * 4;
    let dw = to_usize(to_i32(dst.width()));
    let dst_data = dst.data_mut();
    // 四个采样点都在图内的范围：0 <= ix < sw-1，0 <= iy < sh-1（用无符号比较一次判断两端）。
    #[allow(clippy::cast_sign_loss, reason = "宽高至少为 1")]
    let (inner_w, inner_h) = ((sw - 1) as u32, (sh - 1) as u32);
    for row in y0..y1 {
        // 像素中心映射回源图，再减 0.5 得到双线性采样的左上角。
        #[allow(clippy::cast_precision_loss, reason = "坐标远小于 2^24")]
        let (cx, cy) = (x0 as f32 + 0.5, row as f32 + 0.5);
        let sx = inv.sx * cx + inv.kx * cy + inv.tx - 0.5;
        let sy = inv.ky * cx + inv.sy * cy + inv.ty - 0.5;
        #[allow(clippy::cast_possible_truncation, reason = "定点数，坐标远小于 32768")]
        let (mut fx, mut fy) = ((sx * ONE) as i32, (sy * ONE) as i32);
        let d_row = to_usize(row) * dw;
        for col in x0..x1 {
            let (ix, iy) = (fx >> 16, fy >> 16);
            fx = fx.wrapping_add(step_x);
            fy = fy.wrapping_add(step_y);
            #[allow(clippy::cast_sign_loss, reason = "负数转无符号后必然越界，正好被排除")]
            let quad = if (ix as u32) < inner_w && (iy as u32) < inner_h {
                let i = to_usize(iy) * stride + to_usize(ix) * 4;
                let top = <[u8; 8]>::try_from(&src_data[i..i + 8]).unwrap_or([0; 8]);
                let bottom =
                    <[u8; 8]>::try_from(&src_data[i + stride..i + stride + 8]).unwrap_or([0; 8]);
                [pair(top).0, pair(top).1, pair(bottom).0, pair(bottom).1]
            } else if ix >= -1 && iy >= -1 && ix < sw && iy < sh {
                let texel = |x: i32, y: i32| {
                    if x < 0 || y < 0 || x >= sw || y >= sh {
                        0
                    } else {
                        load(src_data, to_usize(y) * to_usize(sw) + to_usize(x))
                    }
                };
                [
                    texel(ix, iy),
                    texel(ix + 1, iy),
                    texel(ix, iy + 1),
                    texel(ix + 1, iy + 1),
                ]
            } else {
                continue;
            };
            let [a, b, c, d] = quad;
            if (a | b | c | d) >> 24 == 0 {
                continue;
            }
            #[allow(clippy::cast_sign_loss, reason = "取低 8 位作为权重")]
            let (wx, wy) = (
                (((fx - step_x) >> 8) & 0xFF) as u32,
                (((fy - step_y) >> 8) & 0xFF) as u32,
            );
            let mut s = lerp(lerp(a, b, wx), lerp(c, d, wx), wy);
            if op < 256 {
                s = scale(s, op);
            }
            let di = d_row + to_usize(col);
            store(dst_data, di, over(s, load(dst_data, di)));
        }
    }
}

#[inline]
fn pair(bytes: [u8; 8]) -> (u32, u32) {
    (
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    )
}

/// 抗锯齿圆环：按像素到圆心的距离直接算覆盖率，只遍历环带附近的像素。
/// `color` 为满覆盖时的预乘 BGRA。
pub fn stroke_ring(dst: &mut Pixmap, center: (f32, f32), radius: f32, width: f32, color: u32) {
    let (cx, cy) = center;
    let half = width / 2.0;
    let (outer, inner) = (radius + half + 1.0, (radius - half - 1.0).max(0.0));
    let (dw, dh) = (to_i32(dst.width()), to_i32(dst.height()));
    #[allow(clippy::cast_possible_truncation, reason = "坐标在输出范围附近")]
    let (y0, y1) = (
        ((cy - outer).floor() as i32).max(0),
        ((cy + outer).ceil() as i32).min(dh),
    );
    let data = dst.data_mut();
    for y in y0..y1 {
        #[allow(clippy::cast_precision_loss, reason = "坐标远小于 2^24")]
        let dy = y as f32 + 0.5 - cy;
        let outer_x = (outer * outer - dy * dy).max(0.0).sqrt();
        let inner_x = if dy.abs() < inner {
            (inner * inner - dy * dy).sqrt()
        } else {
            0.0
        };
        for (from, to) in [(cx - outer_x, cx - inner_x), (cx + inner_x, cx + outer_x)] {
            #[allow(clippy::cast_possible_truncation, reason = "坐标在输出范围附近")]
            let (x0, x1) = ((from.floor() as i32).max(0), (to.ceil() as i32).min(dw));
            for x in x0..x1 {
                #[allow(clippy::cast_precision_loss, reason = "坐标远小于 2^24")]
                let dx = x as f32 + 0.5 - cx;
                let coverage =
                    (half + 0.5 - ((dx * dx + dy * dy).sqrt() - radius).abs()).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    continue;
                }
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "覆盖率 0–1"
                )]
                let w = (coverage * 256.0) as u32;
                let i = to_usize(y) * to_usize(dw) + to_usize(x);
                store(data, i, over(scale(color, w), load(data, i)));
            }
        }
    }
}

/// 变换后源图在目标中的像素范围（已裁剪到目标内）。
fn dest_bounds(dst: &Pixmap, t: Transform, sw: i32, sh: i32) -> Option<(i32, i32, i32, i32)> {
    #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
    let corners = [
        (0.0, 0.0),
        (sw as f32, 0.0),
        (0.0, sh as f32),
        (sw as f32, sh as f32),
    ];
    let mapped = corners.map(|(x, y)| (t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty));
    let min_x = mapped.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let max_x = mapped.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
    let min_y = mapped.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let max_y = mapped.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
    if !(min_x.is_finite() && max_x.is_finite() && min_y.is_finite() && max_y.is_finite()) {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "已检查有限，坐标在输出范围附近"
    )]
    let (x0, y0) = (
        (min_x.floor() as i32 - 1).max(0),
        (min_y.floor() as i32 - 1).max(0),
    );
    #[allow(
        clippy::cast_possible_truncation,
        reason = "已检查有限，坐标在输出范围附近"
    )]
    let (x1, y1) = (
        (max_x.ceil() as i32 + 1).min(to_i32(dst.width())),
        (max_y.ceil() as i32 + 1).min(to_i32(dst.height())),
    );
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

fn to_usize(v: i32) -> usize {
    usize::try_from(v).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixmap(w: u32, h: u32, px: [u8; 4]) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        for c in p.data_mut().as_chunks_mut::<4>().0 {
            *c = px;
        }
        p
    }

    #[test]
    fn over_matches_premultiplied_formula() {
        let mut dst = pixmap(1, 1, [200, 100, 50, 255]);
        blit_over(&mut dst, &pixmap(1, 1, [64, 0, 0, 128]), 0, 0);
        let d = dst.data();
        assert!((i32::from(d[0]) - (64 + 200 * 127 / 255)).abs() <= 1);
        assert_eq!(d[3], 255);
    }

    #[test]
    fn identity_affine_equals_blit() {
        let mut src = Pixmap::new(4, 3).unwrap();
        for (i, c) in src.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let a = (i * 20) as u8;
            *c = [a / 2, a / 3, a, a];
        }
        let mut a = pixmap(8, 8, [10, 20, 30, 255]);
        let mut b = a.clone();
        blit_over(&mut a, &src, 2, 3);
        draw_affine(&mut b, &src, Transform::from_translate(2.0, 3.0), 1.0);
        let diff = a
            .data()
            .iter()
            .zip(b.data())
            .map(|(x, y)| x.abs_diff(*y))
            .max();
        assert!(diff <= Some(1), "差异 {diff:?}");
    }

    #[test]
    fn clips_outside_target() {
        let mut dst = pixmap(4, 4, [0, 0, 0, 0]);
        let src = pixmap(4, 4, [255, 255, 255, 255]);
        blit_over(&mut dst, &src, -2, -2);
        draw_affine(
            &mut dst,
            &src,
            Transform::from_translate(3.0, 3.0).pre_rotate(30.0),
            0.5,
        );
        assert_eq!(dst.data()[3], 255);
    }
}
