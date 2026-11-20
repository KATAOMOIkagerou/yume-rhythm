//! Opt-in desktop regression: compare actual compositor pixels against a known
//! underlay. A WebView screenshot alone cannot detect an opaque native backdrop.
use serde_json::{json, Value};
use tao::{
    event_loop::EventLoopWindowTarget,
    platform::windows::{WindowBuilderExtWindows, WindowExtWindows},
    window::{Window, WindowBuilder},
};
use windows_sys::Win32::{
    Graphics::Gdi::{GetDC, GetPixel, ReleaseDC},
    UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE},
};

#[derive(Default)]
pub struct CompositionProbe(Option<Window>);

impl CompositionProbe {
    pub fn command<T>(
        &mut self,
        pet: &Window,
        target: &EventLoopWindowTarget<T>,
        request: &Value,
    ) -> Result<Value, String> {
        match request["operation"].as_str().unwrap_or("") {
            "show" => {
                self.0.take();
                let color = request["color"].as_array().ok_or("缺少测试底色")?;
                if color.len() != 3 {
                    return Err("无效测试底色".into());
                }
                let color: Vec<u8> = color
                    .iter()
                    .map(|v| {
                        v.as_u64()
                            .filter(|v| *v <= 255)
                            .map(|v| v as u8)
                            .ok_or("无效测试底色")
                    })
                    .collect::<Result<_, _>>()?;
                let window = WindowBuilder::new()
                    .with_title("桌宠透明合成验证底板")
                    .with_decorations(false)
                    .with_undecorated_shadow(false)
                    .with_always_on_top(true)
                    .with_inner_size(pet.inner_size())
                    .with_position(pet.inner_position().map_err(|e| e.to_string())?)
                    .with_background_color((color[0], color[1], color[2], 255))
                    .with_focused(false)
                    .build(target)
                    .map_err(|e| e.to_string())?;
                unsafe {
                    SetWindowPos(
                        window.hwnd() as _,
                        pet.hwnd() as _,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                self.0 = Some(window);
                Ok(Value::Null)
            }
            "sample" => {
                let origin = pet.inner_position().map_err(|e| e.to_string())?;
                let scale = pet.scale_factor();
                let points = request["points"].as_array().ok_or("缺少测试采样点")?;
                unsafe {
                    let dc = GetDC(std::ptr::null_mut());
                    if dc.is_null() {
                        return Err("无法读取桌面合成结果".into());
                    }
                    let pixels: Vec<Value> = points
                        .iter()
                        .map(|p| {
                            let x =
                                origin.x + (p["x"].as_f64().unwrap_or(0.) * scale).round() as i32;
                            let y =
                                origin.y + (p["y"].as_f64().unwrap_or(0.) * scale).round() as i32;
                            let color = GetPixel(dc, x, y);
                            json!([color & 255, (color >> 8) & 255, (color >> 16) & 255])
                        })
                        .collect();
                    ReleaseDC(std::ptr::null_mut(), dc);
                    Ok(json!(pixels))
                }
            }
            "hide" => {
                self.0.take();
                Ok(Value::Null)
            }
            _ => Err("未知合成检查操作".into()),
        }
    }
}
