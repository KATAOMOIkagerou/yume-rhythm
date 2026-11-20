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
    /// 读取并校验角色包。`expected_id` 是目录名。
    ///
    /// # Errors
    /// 任何一条校验不通过都返回 `PackError`，信息可直接展示给用户。
    pub fn load(source: &dyn PackSource, expected_id: &str) -> Result<Self, PackError> {
        let manifest: Manifest = read_json(source, MANIFEST_FILE)?;
        validate::check_manifest(&manifest, expected_id).map_err(PackError::Invalid)?;
        let file: ExpressionsFile = read_json(source, EXPRESSIONS_FILE)?;
        validate::check_expressions(&file.expressions).map_err(PackError::Invalid)?;

        let canvas_files = canvas_files(&manifest, &file.expressions);
        let free_files = manifest.effects.notes.iter().flat_map(|n| n.images.iter());
        let free_files: Vec<&str> = free_files.map(String::as_str).collect();
        if canvas_files.len() + free_files.len() > validate::MAX_IMAGES {
            return Err(PackError::Invalid(format!(
                "图片数量超过 {}",
                validate::MAX_IMAGES
            )));
        }

        let mut images = HashMap::new();
        for name in canvas_files {
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
            images.insert(name.to_string(), decoded.into_sprite());
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
        Ok(Self {
            manifest,
            expressions: file.expressions,
            images,
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
