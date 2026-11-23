//! 角色包：读取、校验、解码图片（docs/角色包格式.md）。

pub mod expressions;
pub mod manifest;
mod source;
pub mod validate;

pub use source::{MemorySource, PackSource};

use crate::sprite::{Sprite, decode_png};
use expressions::{ExpressionDef, ExpressionsFile};
use manifest::Manifest;
use serde::Deserialize;
use std::collections::HashMap;
use yume_core::model::expression::ExpressionMeta;

pub const MANIFEST_FILE: &str = "character.json";
pub const EXPRESSIONS_FILE: &str = "expressions.json";

/// 加载失败的原因，`Display` 是给用户看的中文。
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("缺少文件 {0}")]
    Missing(String),
    #[error("读取 {file} 失败：{detail}")]
    Io { file: String, detail: String },
    #[error("{file} 格式错误：{detail}")]
    Syntax { file: String, detail: String },
    #[error("{0}")]
    Invalid(String),
    #[error("图片 {file}：{detail}")]
    Image { file: String, detail: String },
}

/// 已加载并校验通过的角色包。
#[derive(Clone, Debug)]
pub struct Pack {
    pub manifest: Manifest,
    pub expressions: Vec<ExpressionDef>,
    /// 按文件名索引的图片。画布图层已裁边；音符图片保持原尺寸（坐标为 0）。
    pub images: HashMap<String, Sprite>,
    /// 自定义表情目录读取或校验失败时的中文提示（主包仍正常加载）。
    pub custom_warning: Option<String>,
}

const CUSTOM_PREFIX: &str = "custom/";

struct LayeredSource<'a> {
    main: &'a dyn PackSource,
    custom: &'a dyn PackSource,
}

impl PackSource for LayeredSource<'_> {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        if let Some(rel) = path.strip_prefix(CUSTOM_PREFIX) {
            self.custom.read(rel)
        } else {
            self.main.read(path)
        }
    }
}

/// 只读 id 和名字，用于列出可选角色。
#[derive(Clone, Debug, Deserialize)]
pub struct PackInfo {
    pub id: String,
    pub name: String,
}

impl PackInfo {
    /// # Errors
    /// character.json 不是合法 JSON 或缺少 id、name。
    pub fn parse(bytes: &[u8]) -> Result<Self, PackError> {
        serde_json::from_slice(bytes).map_err(|e| syntax(MANIFEST_FILE, &e))
    }
}

impl Pack {
    /// 读取并校验角色包，同时尝试合并用户自定义表情。`expected_id` 是目录名。
    ///
    /// 自定义表情加载失败只会记录 `custom_warning` 并舍弃自定义项，不影响主角色包返回 `Ok`。
    ///
    /// # Errors
    /// 主包任何一条校验不通过都返回 `PackError`，信息可直接展示给用户。
    pub fn load(
        source: &dyn PackSource,
        expected_id: &str,
        custom: Option<&dyn PackSource>,
    ) -> Result<Self, PackError> {
        let manifest: Manifest = read_json(source, MANIFEST_FILE)?;
        validate::check_manifest(&manifest, expected_id).map_err(PackError::Invalid)?;
        let mut file: ExpressionsFile = read_json(source, EXPRESSIONS_FILE)?;
        validate::check_expressions(&file.expressions).map_err(PackError::Invalid)?;

        let canvas_files = canvas_files(&manifest, &file.expressions);
        let free_files = manifest.effects.notes.iter().flat_map(|n| n.images.iter());
        let free_files: Vec<&str> = free_files.map(String::as_str).collect();
        let base_image_count = canvas_files.len() + free_files.len();
        if base_image_count > validate::MAX_IMAGES {
            return Err(PackError::Invalid(format!(
                "图片数量超过 {}",
                validate::MAX_IMAGES
            )));
        }

        let mut images = HashMap::new();
        for name in canvas_files {
            images.insert(
                name.to_string(),
                load_canvas_sprite(source, name, &manifest)?,
            );
        }
        for name in free_files {
            let decoded = read_image(source, name)?;
            let size = tiny_skia::IntSize::from_wh(decoded.width, decoded.height);
            let premul = decoded.into_sprite();
            // 音符图片要保持原尺寸居中，裁边后再贴回原尺寸画布。
            let mut full = size.and_then(|s| tiny_skia::Pixmap::new(s.width(), s.height()));
            if let Some(full) = full.as_mut() {
                let paint = tiny_skia::PixmapPaint::default();
                let t = tiny_skia::Transform::identity();
                full.draw_pixmap(premul.x, premul.y, premul.pixmap.as_ref(), &paint, t, None);
            }
            let pixmap = full.unwrap_or_else(crate::sprite::blank);
            images.insert(name.to_string(), Sprite { pixmap, x: 0, y: 0 });
        }

        let mut custom_warning = None;
        if let Some(custom_source) = custom {
            let layered = LayeredSource {
                main: source,
                custom: custom_source,
            };
            match load_custom(
                &layered,
                custom_source,
                &manifest,
                &file.expressions,
                base_image_count,
            ) {
                Ok((extra_exprs, extra_images)) => {
                    file.expressions.extend(extra_exprs);
                    images.extend(extra_images);
                }
                Err(msg) => custom_warning = Some(msg),
            }
        }

        Ok(Self {
            manifest,
            expressions: file.expressions,
            images,
            custom_warning,
        })
    }

    #[must_use]
    pub fn expression_meta(&self) -> Vec<ExpressionMeta> {
        self.expressions.iter().map(ExpressionDef::meta).collect()
    }

    #[must_use]
    pub fn expression(&self, id: &str) -> Option<&ExpressionDef> {
        self.expressions.iter().find(|e| e.id == id)
    }
}

/// 需要与画布同尺寸的图片：图层、眨眼、表情资源。去重并保持顺序。
fn canvas_files<'a>(m: &'a Manifest, list: &'a [ExpressionDef]) -> Vec<&'a str> {
    let layers = m.layers.iter().filter_map(|l| l.image.as_deref());
    let blink = m.blink.iter().map(|b| b.image.as_str());
    let faces = list.iter().flat_map(|e| e.resources.files());
    let mut out: Vec<&str> = Vec::new();
    for name in layers.chain(blink).chain(faces) {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn read_bytes(source: &dyn PackSource, name: &str) -> Result<Vec<u8>, PackError> {
    validate::check_path(name).map_err(PackError::Invalid)?;
    match source.read(name) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => Err(PackError::Missing(name.into())),
        Err(detail) => Err(PackError::Io {
            file: name.into(),
            detail,
        }),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(
    source: &dyn PackSource,
    name: &str,
) -> Result<T, PackError> {
    let bytes = read_bytes(source, name)?;
    serde_json::from_slice(&bytes).map_err(|e| syntax(name, &e))
}

fn read_image(source: &dyn PackSource, name: &str) -> Result<crate::sprite::Decoded, PackError> {
    let bytes = read_bytes(source, name)?;
    decode_png(&bytes).map_err(|detail| PackError::Image {
        file: name.into(),
        detail,
    })
}

fn syntax(file: &str, e: &serde_json::Error) -> PackError {
    PackError::Syntax {
        file: file.into(),
        detail: e.to_string(),
    }
}

fn load_canvas_sprite(
    source: &dyn PackSource,
    name: &str,
    manifest: &Manifest,
) -> Result<Sprite, PackError> {
    let decoded = read_image(source, name)?;
    let (cw, ch) = (manifest.canvas.width, manifest.canvas.height);
    if (decoded.width, decoded.height) != (cw, ch) {
        let detail = format!(
            "尺寸 {}×{} 与画布 {cw}×{ch} 不一致",
            decoded.width, decoded.height
        );
        return Err(PackError::Image {
            file: name.into(),
            detail,
        });
    }
    Ok(decoded.into_sprite())
}

fn load_custom(
    layered: &LayeredSource<'_>,
    custom_source: &dyn PackSource,
    manifest: &Manifest,
    builtin: &[ExpressionDef],
    base_image_count: usize,
) -> Result<(Vec<ExpressionDef>, HashMap<String, Sprite>), String> {
    let bytes = match custom_source.read(EXPRESSIONS_FILE) {
        Ok(Some(b)) => b,
        Ok(None) => return Ok((Vec::new(), HashMap::new())),
        Err(detail) => return Err(format!("读取自定义表情配置失败：{detail}")),
    };
    let mut file: ExpressionsFile =
        serde_json::from_slice(&bytes).map_err(|e| format!("自定义表情配置格式错误：{e}"))?;
    validate::check_custom_expressions(&file.expressions, builtin)?;

    let mut unique_files: Vec<String> = Vec::new();
    for def in &mut file.expressions {
        def.custom = true;
        def.resources.prefix_dir(CUSTOM_PREFIX);
        for file_name in def.resources.files() {
            if !unique_files.iter().any(|u| u == file_name) {
                unique_files.push(file_name.to_owned());
            }
        }
    }
    if base_image_count + unique_files.len() > validate::MAX_IMAGES {
        return Err(format!(
            "自定义表情导致图片总数超过 {}",
            validate::MAX_IMAGES
        ));
    }

    let mut extra_images = HashMap::new();
    for prefixed in unique_files {
        let sprite = load_canvas_sprite(layered, &prefixed, manifest).map_err(|e| e.to_string())?;
        extra_images.insert(prefixed, sprite);
    }
    Ok((file.expressions, extra_images))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_png(width: u32, height: u32) -> Vec<u8> {
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        rgba[3] = 255;
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Ok(mut writer) = encoder.write_header() {
            let _ = writer.write_image_data(&rgba);
        }
        out
    }

    fn base_source() -> MemorySource {
        let mut src = MemorySource::default();
        src.insert(
            MANIFEST_FILE,
            r#"{
                "format": 2,
                "id": "mini",
                "name": "测试角色",
                "canvas": { "width": 16, "height": 16 },
                "viewport": { "x": 0, "y": 0, "width": 16, "height": 16 },
                "display_scale": 1.0,
                "rigs": {
                    "body": { "pivot": [8, 12] },
                    "head": { "pivot": [8, 8] }
                },
                "layers": [
                    { "slot": "eyes" },
                    { "slot": "mouth" },
                    { "slot": "overlay" }
                ]
            }"#
            .as_bytes()
            .to_vec(),
        );
        src.insert(
            EXPRESSIONS_FILE,
            r#"{
                "version": 1,
                "expressions": [
                    {
                        "id": "neutral",
                        "name": "自然",
                        "type": "base",
                        "resources": { "eyes": "e.png", "mouth": "m.png" },
                        "durationMs": 3200
                    }
                ]
            }"#
            .as_bytes()
            .to_vec(),
        );
        let png16 = test_png(16, 16);
        src.insert("e.png", png16.clone());
        src.insert("m.png", png16);
        src
    }

    #[test]
    fn loads_and_renders_custom_accessory_with_overlay_only() {
        let main = base_source();
        let mut custom = MemorySource::default();
        custom.insert(
            EXPRESSIONS_FILE,
            r#"{
                "version": 1,
                "expressions": [
                    {
                        "id": "custom-1",
                        "name": "星星",
                        "type": "accessory",
                        "resources": { "overlay": "stars.png" },
                        "durationMs": 3000,
                        "random": true
                    }
                ]
            }"#
            .as_bytes()
            .to_vec(),
        );
        custom.insert("stars.png", test_png(16, 16));

        let Ok(pack) = Pack::load(&main, "mini", Some(&custom)) else {
            assert!(Pack::load(&main, "mini", Some(&custom)).is_ok());
            return;
        };
        assert_eq!(pack.custom_warning, None);
        let meta = pack.expression_meta();
        let custom_meta = meta.iter().find(|m| m.id == "custom-1");
        assert!(custom_meta.is_some_and(|m| m.custom));
        assert!(pack.images.contains_key("custom/stars.png"));

        let mut renderer = crate::LayeredCharacter::new(pack, 1);
        let scene = yume_core::derive::PetScene {
            activity: yume_core::model::events::ActivityMode::Chill,
            music: false,
            follow_beat: false,
            timer_running: false,
            base_expression: "neutral".into(),
            accessory_expression: Some("custom-1".into()),
            event: None,
            dozing: false,
        };
        let mut frame = crate::Frame::default();
        use crate::CharacterRenderer;
        renderer.advance(0.0, &scene, &yume_beat::MotionFrame::default());
        assert!(renderer.render(&mut frame, 1.0));
    }

    #[test]
    fn bad_custom_size_or_duplicate_id_warns_without_failing_main_pack() {
        let main = base_source();
        let mut wrong_size = MemorySource::default();
        wrong_size.insert(
            EXPRESSIONS_FILE,
            r#"{
                "version": 1,
                "expressions": [
                    {
                        "id": "custom-1",
                        "name": "错尺寸",
                        "type": "accessory",
                        "resources": { "overlay": "bad.png" },
                        "durationMs": 3000
                    }
                ]
            }"#
            .as_bytes()
            .to_vec(),
        );
        wrong_size.insert("bad.png", test_png(8, 8));
        let Ok(pack) = Pack::load(&main, "mini", Some(&wrong_size)) else {
            assert!(Pack::load(&main, "mini", Some(&wrong_size)).is_ok());
            return;
        };
        assert!(pack.custom_warning.is_some());
        assert_eq!(pack.expressions.len(), 1);

        let mut dup = MemorySource::default();
        dup.insert(
            EXPRESSIONS_FILE,
            r#"{
                "version": 1,
                "expressions": [
                    {
                        "id": "neutral",
                        "name": "冲突",
                        "type": "accessory",
                        "resources": { "overlay": "ok.png" },
                        "durationMs": 3000
                    }
                ]
            }"#
            .as_bytes()
            .to_vec(),
        );
        dup.insert("ok.png", test_png(16, 16));
        let Ok(pack_dup) = Pack::load(&main, "mini", Some(&dup)) else {
            assert!(Pack::load(&main, "mini", Some(&dup)).is_ok());
            return;
        };
        assert!(pack_dup.custom_warning.is_some());
        assert_eq!(pack_dup.expressions.len(), 1);
    }
}
