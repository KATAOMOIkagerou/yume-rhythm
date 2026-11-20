//! 图片：PNG 解码、裁掉透明边、转为预乘 alpha 的 BGRA。
//!
//! 所有像素数据都按 **BGRA** 排列，直接就是 Windows `UpdateLayeredWindow` 需要的格式。
//! tiny-skia 把它当 RGBA 处理也没问题：预乘混合对每个通道一视同仁。
//! 唯一要注意的是纯色绘制（光环、音符着色），颜色要用 `bgra_color` 交换 R、B。

use std::io::Cursor;
use tiny_skia::{Color, IntSize, Pixmap};

pub const MAX_SIDE: u32 = 4096;

/// 一张已裁剪的图片：`pixmap` 放在画布的 `(x, y)` 处。
#[derive(Clone, Debug)]
pub struct Sprite {
    pub pixmap: Pixmap,
    pub x: i32,
    pub y: i32,
}

/// 解码后的完整图片（未裁剪）。
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// 直通（非预乘）RGBA8。
    pub rgba: Vec<u8>,
}

/// 解码 PNG 为 RGBA8。
///
/// # Errors
/// 不是合法 PNG、尺寸超限时返回中文描述。
pub fn decode_png(bytes: &[u8]) -> Result<Decoded, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("不是有效的 PNG：{e}"))?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!(
            "图片尺寸 {width}×{height} 超出范围（最大 {MAX_SIDE}）"
        ));
    }
    let size = reader.output_buffer_size().ok_or("图片过大")?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG 解码失败：{e}"))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("调色板图片展开失败".into()),
    };
    Ok(Decoded {
        width,
        height,
        rgba,
    })
}

impl Decoded {
    /// 不透明像素的包围盒 `(x0, y0, x1, y1)`，全透明时为 `None`。
    #[must_use]
    pub fn content_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let w = self.width as usize;
        let mut bounds: Option<(u32, u32, u32, u32)> = None;
        for (i, px) in self.rgba.as_chunks::<4>().0.iter().enumerate() {
            if px[3] == 0 {
                continue;
            }
            let (x, y) = (to_u32(i % w), to_u32(i / w));
            bounds = Some(match bounds {
                None => (x, y, x + 1, y + 1),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)),
            });
        }
        bounds
    }

    /// 裁掉透明边，转为预乘 BGRA。全透明图片返回 1×1 透明图。
    #[must_use]
    pub fn into_sprite(self) -> Sprite {
        let Some((x0, y0, x1, y1)) = self.content_bounds() else {
            return Sprite {
                pixmap: blank(),
                x: 0,
                y: 0,
            };
        };
        let (cw, ch) = (x1 - x0, y1 - y0);
        let mut data = Vec::with_capacity((cw * ch * 4) as usize);
        for y in y0..y1 {
            let start = ((y * self.width + x0) * 4) as usize;
            let row = &self.rgba[start..start + (cw * 4) as usize];
            data.extend(
                row.as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|&[r, g, b, a]| premultiplied_bgra(r, g, b, a)),
            );
        }
        let pixmap = IntSize::from_wh(cw, ch).and_then(|size| Pixmap::from_vec(data, size));
        Sprite {
            pixmap: pixmap.unwrap_or_else(blank),
            x: to_i32(x0),
            y: to_i32(y0),
        }
    }
}

fn premultiplied_bgra(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    let mul = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
    [mul(b), mul(g), mul(r), a]
}

/// 1×1 透明图。
#[must_use]
pub fn blank() -> Pixmap {
    #[allow(clippy::expect_used, reason = "1×1 是合法尺寸，创建不会失败")]
    Pixmap::new(1, 1).expect("1x1 pixmap")
}

/// 纯色绘制用的颜色：交换 R、B，配合 BGRA 像素。
#[must_use]
pub fn bgra_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(b, g, r, a)
}

/// 解析 `#rrggbb`。
#[must_use]
pub fn parse_hex(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

/// 用 alpha 作为遮罩，生成一张纯色图（预乘 BGRA）。音符着色、光晕用。
#[must_use]
pub fn tinted(mask: &Pixmap, rgb: (u8, u8, u8)) -> Pixmap {
    let mut out = mask.clone();
    let (r, g, b) = rgb;
    for px in out.data_mut().as_chunks_mut::<4>().0 {
        *px = premultiplied_bgra(r, g, b, px[3]);
    }
    out
}

/// 把颜色通道限制在 alpha 以内。tiny-skia 的双三次采样偶尔会比 alpha 大 1，
/// 不合法的预乘像素会让后续混合溢出。
pub fn clamp_premultiplied(pixmap: &mut Pixmap) {
    for px in pixmap.data_mut().as_chunks_mut::<4>().0 {
        let a = px[3];
        for c in &mut px[..3] {
            *c = (*c).min(a);
        }
    }
}

/// 对 alpha 做三次盒式模糊（近似高斯），四周留出 `radius * 3` 的边。返回新图与边宽。
#[must_use]
pub fn blurred_alpha(src: &Pixmap, radius: u32) -> (Pixmap, u32) {
    let pad = radius * 3;
    let (w, h) = (src.width() + pad * 2, src.height() + pad * 2);
    let mut alpha = vec![0f32; (w * h) as usize];
    for y in 0..src.height() {
        for x in 0..src.width() {
            let a = src.data()[((y * src.width() + x) * 4 + 3) as usize];
            alpha[((y + pad) * w + x + pad) as usize] = f32::from(a);
        }
    }
    for _ in 0..3 {
        box_pass(&mut alpha, w, h, radius, true);
        box_pass(&mut alpha, w, h, radius, false);
    }
    let data = alpha
        .iter()
        .flat_map(|&a| [0, 0, 0, a.round().clamp(0.0, 255.0) as u8])
        .collect();
    let pixmap = IntSize::from_wh(w, h).and_then(|size| Pixmap::from_vec(data, size));
    (pixmap.unwrap_or_else(blank), pad)
}

fn box_pass(buf: &mut [f32], w: u32, h: u32, r: u32, horizontal: bool) {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let idx = |line: u32, i: u32| if horizontal { line * w + i } else { i * w + line } as usize;
    let mut tmp = vec![0f32; len as usize];
    #[allow(clippy::cast_precision_loss, reason = "半径很小")]
    let norm = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let mut sum = 0.0;
        for i in 0..=r.min(len - 1) {
            sum += buf[idx(line, i)];
        }
        for i in 0..len {
            tmp[i as usize] = sum * norm;
            if i + r + 1 < len {
                sum += buf[idx(line, i + r + 1)];
            }
            if i >= r {
                sum -= buf[idx(line, i - r)];
            }
        }
        for i in 0..len {
            buf[idx(line, i)] = tmp[i as usize];
        }
    }
}

fn to_u32(v: usize) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn crops_and_premultiplies_to_bgra() {
        let mut rgba = vec![0u8; 4 * 4 * 4];
        let i = (2 * 4 + 1) * 4;
        rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 128]);
        let sprite = decode_png(&encode(4, 4, &rgba)).unwrap().into_sprite();
        assert_eq!(
            (
                sprite.x,
                sprite.y,
                sprite.pixmap.width(),
                sprite.pixmap.height()
            ),
            (1, 2, 1, 1)
        );
        assert_eq!(sprite.pixmap.data(), &[0, 0, 128, 128]);
    }

    #[test]
    fn clamps_color_to_alpha() {
        let mut p = Pixmap::new(1, 1).unwrap();
        p.data_mut().copy_from_slice(&[16, 3, 20, 15]);
        clamp_premultiplied(&mut p);
        assert_eq!(p.data(), &[15, 3, 15, 15]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_png(b"nope").is_err());
    }

    #[test]
    fn parses_hex_colors() {
        assert_eq!(parse_hex("#c084fc"), Some((0xc0, 0x84, 0xfc)));
        assert_eq!(parse_hex("c084fc"), None);
    }
}
