//! 渲染输出：预乘 alpha 的 BGRA 像素，可直接交给 Windows 分层窗口。

use tiny_skia::Pixmap;

#[derive(Clone, Debug)]
pub struct Frame {
    pixmap: Pixmap,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            pixmap: crate::sprite::blank(),
        }
    }
}

impl Frame {
    #[must_use]
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// 每像素 4 字节，B、G、R、A，预乘，逐行从上到下，无行间填充。
    #[must_use]
    pub fn bgra(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// 某点的 alpha（窗口坐标）。越界返回 0。
    #[must_use]
    pub fn alpha_at(&self, x: i32, y: i32) -> u8 {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return 0;
        };
        if x >= self.width() || y >= self.height() {
            return 0;
        }
        self.pixmap.data()[((y * self.width() + x) * 4 + 3) as usize]
    }

    /// 调整尺寸并清空为全透明。
    pub(crate) fn reset(&mut self, width: u32, height: u32) -> &mut Pixmap {
        if self.pixmap.width() != width || self.pixmap.height() != height {
            if let Some(p) = Pixmap::new(width.max(1), height.max(1)) {
                self.pixmap = p;
            }
        } else {
            self.pixmap.fill(tiny_skia::Color::TRANSPARENT);
        }
        &mut self.pixmap
    }

    /// 导出为 PNG（测试与调试用）。像素会转换回 RGBA。
    ///
    /// # Errors
    /// 编码失败时返回描述。
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let unmul = |c: u8, a: u8| {
            if a == 0 {
                0
            } else {
                ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)) as u8
            }
        };
        let rgba: Vec<u8> = self
            .bgra()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[b, g, r, a]| [unmul(r, a), unmul(g, a), unmul(b, a), a])
            .collect();
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width(), self.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(out)
    }
}
