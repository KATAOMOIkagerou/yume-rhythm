//! 内存位图：32 位自上而下 DIB，供 `UpdateLayeredWindow` 读取。

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::core::Result;

pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u8,
    width: i32,
    height: i32,
}

impl Surface {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        let header = BITMAPINFOHEADER {
            biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(40),
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let info = BITMAPINFO {
            bmiHeader: header,
            ..Default::default()
        };
        // SAFETY: 标准 GDI 调用；屏幕 DC 用完立即释放；`bits` 由系统写入。
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(
                Some(dc),
                &raw const info,
                DIB_RGB_COLORS,
                &raw mut bits,
                None,
                0,
            ) {
                Ok(b) => b,
                Err(e) => {
                    let _ = DeleteDC(dc);
                    return Err(e);
                }
            };
            let previous = SelectObject(dc, bitmap.into());
            Ok(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    pub fn dc(&self) -> HDC {
        self.dc
    }

    /// 写入整帧。长度不符时只写入能容纳的部分。
    pub fn write(&mut self, bgra: &[u8]) {
        let capacity = usize::try_from(self.width * self.height * 4).unwrap_or(0);
        let len = bgra.len().min(capacity);
        // SAFETY: `bits` 指向 CreateDIBSection 分配的 width*height*4 字节，`len` 不超过它。
        unsafe {
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), self.bits, len);
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: 恢复原位图后释放本对象创建的 GDI 资源。
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
