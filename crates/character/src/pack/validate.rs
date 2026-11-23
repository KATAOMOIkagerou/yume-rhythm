//! 角色包校验（docs/角色包格式.md 第 5 节）。错误信息是给用户看的中文。

use super::expressions::{DURATION_MAX_MS, DURATION_MIN_MS, ExpressionDef, KindDef};
use super::manifest::{FORMAT, Manifest};
use yume_core::model::expression::NEUTRAL;
use yume_core::persist::is_valid_character_id;

pub const MAX_IMAGES: usize = 200;

/// 包内相对路径：不能以 `/` 开头，不能含 `..`、`\`、`:`，只允许常见文件名字符。
///
/// # Errors
/// 返回中文描述。
pub fn check_path(path: &str) -> Result<(), String> {
    let ok_chars = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));
    let bad = path.is_empty()
        || path.starts_with('/')
        || !ok_chars
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        return Err(format!(
            "文件名「{path}」不合法：只能是包内的相对路径，且只含字母、数字、_-./"
        ));
    }
    Ok(())
}

/// 校验 character.json 中与图片无关的规则。
///
/// # Errors
/// 返回第一条不满足的规则。
pub fn check_manifest(m: &Manifest, expected_id: &str) -> Result<(), String> {
    if m.format != FORMAT {
        return Err(format!(
            "character.json 的 format 应为 {FORMAT}，实际为 {}",
            m.format
        ));
    }
    if !is_valid_character_id(&m.id) || m.id != expected_id {
        return Err(format!(
            "角色 id「{}」必须与目录名「{expected_id}」一致",
            m.id
        ));
    }
    let (cw, ch) = (m.canvas.width, m.canvas.height);
    if cw == 0 || ch == 0 || cw > crate::sprite::MAX_SIDE || ch > crate::sprite::MAX_SIDE {
        return Err(format!("画布尺寸 {cw}×{ch} 超出范围"));
    }
    let v = &m.viewport;
    #[allow(clippy::cast_precision_loss, reason = "画布不超过 4096")]
    let inside = v.x >= 0.0
        && v.y >= 0.0
        && v.width > 0.0
        && v.height > 0.0
        && v.x + v.width <= cw as f32
        && v.y + v.height <= ch as f32;
    if !inside {
        return Err("viewport 必须在画布范围内".into());
    }
    if !(m.display_scale > 0.0 && m.display_scale <= 4.0) {
        return Err("display_scale 必须在 0–4 之间".into());
    }
    if m.layers.is_empty() {
        return Err("layers 不能为空".into());
    }
    for (i, layer) in m.layers.iter().enumerate() {
        if layer.image.is_some() == layer.slot.is_some() {
            return Err(format!(
                "第 {} 个图层必须且只能写 image 或 slot 之一",
                i + 1
            ));
        }
    }
    if let Some(notes) = &m.effects.notes
        && (notes.images.is_empty() || notes.max == 0 || notes.duration_ms <= 0.0)
    {
        return Err("notes 需要至少一张图片，max 和 duration_ms 必须大于 0".into());
    }
    if let Some(ring) = &m.effects.beat_ring
        && (crate::sprite::parse_hex(&ring.color).is_none() || ring.duration_ms <= 0.0)
    {
        return Err("beat_ring 的 color 必须是 #rrggbb，duration_ms 必须大于 0".into());
    }
    Ok(())
}

/// 校验表情列表。
///
/// # Errors
/// 返回第一条不满足的规则。
pub fn check_expressions(list: &[ExpressionDef]) -> Result<(), String> {
    for (i, e) in list.iter().enumerate() {
        if e.id.is_empty() {
            return Err(format!("第 {} 个表情缺少 id", i + 1));
        }
        if list[..i].iter().any(|other| other.id == e.id) {
            return Err(format!("表情 id「{}」重复", e.id));
        }
        if !(DURATION_MIN_MS..=DURATION_MAX_MS).contains(&e.duration_ms) {
            return Err(format!(
                "表情「{}」的 durationMs 必须在 {DURATION_MIN_MS}–{DURATION_MAX_MS} 之间",
                e.id
            ));
        }
    }
    let neutral = list
        .iter()
        .find(|e| e.id == NEUTRAL)
        .ok_or("表情列表必须包含 id 为 neutral 的自然表情")?;
    let r = &neutral.resources;
    let complete = r.animation.is_some() || (r.eyes.is_some() && r.mouth.is_some());
    if neutral.kind != KindDef::Base || !complete {
        return Err("neutral 必须是基础表情，并且有 eyes 和 mouth（或 animation）".into());
    }
    Ok(())
}

/// 校验用户自定义表情列表（与主包表情不重复，且各自满足资源完整度要求）。
///
/// # Errors
/// 返回第一条不满足的规则。
pub fn check_custom_expressions(
    custom: &[ExpressionDef],
    builtin: &[ExpressionDef],
) -> Result<(), String> {
    for (i, e) in custom.iter().enumerate() {
        if e.id.is_empty() {
            return Err(format!("第 {} 个自定义表情缺少 id", i + 1));
        }
        if builtin.iter().any(|b| b.id == e.id) {
            return Err(format!("自定义表情 id「{}」与内置表情重复", e.id));
        }
        if custom[..i].iter().any(|other| other.id == e.id) {
            return Err(format!("自定义表情 id「{}」重复", e.id));
        }
        if !(DURATION_MIN_MS..=DURATION_MAX_MS).contains(&e.duration_ms) {
            return Err(format!(
                "自定义表情「{}」的 durationMs 必须在 {DURATION_MIN_MS}–{DURATION_MAX_MS} 之间",
                e.id
            ));
        }
        let r = &e.resources;
        match e.kind {
            KindDef::Base => {
                if r.eyes.is_none() || r.mouth.is_none() {
                    return Err(format!("自定义一级表情「{}」缺少眼睛或嘴巴素材", e.name));
                }
            }
            KindDef::Accessory => {
                if r.eyes.is_none() && r.mouth.is_none() && r.overlay.is_none() {
                    return Err(format!("自定义二级表情「{}」至少需要一张素材", e.name));
                }
            }
        }
        for file in r.files() {
            check_path(file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escaping_paths() {
        for bad in [
            "../x.png", "/x.png", "a\\b.png", "C:x.png", "a//b.png", "", "./a.png", "中.png",
        ] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
        assert!(check_path("fx/note_1.png").is_ok());
    }
}
