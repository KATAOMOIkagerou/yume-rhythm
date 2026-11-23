//! 自定义表情的素材校验、目录复制与 `expressions.json` 读写（docs/角色包格式.md 第 6 节）。

use std::path::Path;
use yume_character::pack::EXPRESSIONS_FILE;
use yume_character::pack::expressions::{ExpressionDef, ExpressionsFile, KindDef, Resources};
use yume_character::sprite::decode_png;
use yume_core::Command;
use yume_core::command::{CharacterCommand, EditorCommand};
use yume_core::model::editor::{AssetChoice, AssetSlot, CustomExpressionSpec};
use yume_core::model::expression::ExpressionKind;

/// 弹出 PNG 选择框并立即校验是否能解码且与当前角色画布同尺寸。
#[must_use]
pub fn pick_asset(owner: Option<isize>, slot: AssetSlot, canvas: (u32, u32)) -> Option<Command> {
    let path = yume_win::pick_open_file(owner, "选择表情素材 PNG", "PNG 透明图 (*.png)", "*.png")?;
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            return Some(Command::Editor(EditorCommand::AssetRejected(format!(
                "无法读取文件：{e}"
            ))));
        }
    };
    let decoded = match decode_png(&bytes) {
        Ok(d) => d,
        Err(e) => {
            return Some(Command::Editor(EditorCommand::AssetRejected(format!(
                "不是有效的 PNG 图片：{e}"
            ))));
        }
    };
    if (decoded.width, decoded.height) != canvas {
        return Some(Command::Editor(EditorCommand::AssetRejected(format!(
            "图片尺寸 {}×{} 与角色画布 {}×{} 不一致",
            decoded.width, decoded.height, canvas.0, canvas.1
        ))));
    }
    Some(Command::Editor(EditorCommand::AssetPicked {
        slot,
        path: path.to_string_lossy().into_owned(),
    }))
}

/// 将新表情或编辑后的自定义表情写入 `%APPDATA%\韵梦桌伴\custom_expressions\<角色>\`。
pub fn save_and_dispatch(
    custom_root: Option<&Path>,
    spec: &CustomExpressionSpec,
    dispatch: impl Fn(Command),
) {
    let Some(root) = custom_root else {
        dispatch(Command::Editor(EditorCommand::SaveFailed(
            "找不到用户数据目录，无法保存自定义表情".into(),
        )));
        return;
    };
    let dir = root.join(&spec.character);
    match save_to_dir(&dir, spec) {
        Ok(()) => {
            dispatch(Command::Editor(EditorCommand::Saved));
            dispatch(Command::Character(CharacterCommand::Reload));
        }
        Err(msg) => dispatch(Command::Editor(EditorCommand::SaveFailed(msg))),
    }
}

/// 删除指定自定义表情记录及其 PNG 文件并重载角色。
pub fn delete_and_dispatch(
    custom_root: Option<&Path>,
    character: &str,
    id: &str,
    dispatch: impl Fn(Command),
) {
    let Some(root) = custom_root else {
        return;
    };
    if let Err(e) = delete_in_dir(&root.join(character), id) {
        log::warn!("删除自定义表情失败（{character}/{id}）：{e}");
    }
    dispatch(Command::Character(CharacterCommand::Reload));
}

fn save_to_dir(dir: &Path, spec: &CustomExpressionSpec) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败：{e}"))?;
    let eyes = materialize_slot(dir, &spec.id, "eyes", &spec.eyes)?;
    let mouth = materialize_slot(dir, &spec.id, "mouth", &spec.mouth)?;
    let overlay = materialize_slot(dir, &spec.id, "overlay", &spec.overlay)?;

    let json_path = dir.join(EXPRESSIONS_FILE);
    let mut file = read_or_empty(&json_path)?;
    let def = ExpressionDef {
        id: spec.id.clone(),
        name: spec.name.clone(),
        kind: match spec.kind {
            ExpressionKind::Base => KindDef::Base,
            ExpressionKind::Accessory => KindDef::Accessory,
        },
        resources: Resources {
            eyes,
            mouth,
            overlay,
            animation: None,
        },
        duration_ms: spec.duration_ms,
        random: spec.random,
        event: None,
        blink: true,
        custom: true,
    };
    if let Some(slot) = file.expressions.iter_mut().find(|e| e.id == spec.id) {
        *slot = def;
    } else {
        file.expressions.push(def);
    }
    let text = file.to_pretty_json()?;
    yume_win::storage::write_text_atomic(&json_path, &text)
        .map_err(|e| format!("写入 {EXPRESSIONS_FILE} 失败：{e}"))
}

fn delete_in_dir(dir: &Path, id: &str) -> Result<(), String> {
    let json_path = dir.join(EXPRESSIONS_FILE);
    let mut file = read_or_empty(&json_path)?;
    let before = file.expressions.len();
    file.expressions.retain(|e| e.id != id);
    if file.expressions.len() != before {
        let text = file.to_pretty_json()?;
        yume_win::storage::write_text_atomic(&json_path, &text)
            .map_err(|e| format!("更新 {EXPRESSIONS_FILE} 失败：{e}"))?;
    }
    for slot in ["eyes", "mouth", "overlay"] {
        let _ = std::fs::remove_file(dir.join(format!("{id}_{slot}.png")));
    }
    Ok(())
}

fn materialize_slot(
    dir: &Path,
    id: &str,
    slot_name: &str,
    choice: &Option<AssetChoice>,
) -> Result<Option<String>, String> {
    let target_name = format!("{id}_{slot_name}.png");
    let dest = dir.join(&target_name);
    match choice {
        Some(AssetChoice::Import(src_str)) => {
            let src = Path::new(src_str);
            if src != dest {
                std::fs::copy(src, &dest).map_err(|e| format!("复制图片失败：{e}"))?;
            }
            Ok(Some(target_name))
        }
        Some(AssetChoice::Keep(rel)) => {
            let cleaned = rel.strip_prefix("custom/").unwrap_or(rel);
            Ok(Some(cleaned.to_owned()))
        }
        None => {
            if dest.exists() {
                let _ = std::fs::remove_file(&dest);
            }
            Ok(None)
        }
    }
}

fn read_or_empty(path: &Path) -> Result<ExpressionsFile, String> {
    match std::fs::read(path) {
        Ok(bytes) => ExpressionsFile::from_json_bytes(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ExpressionsFile {
            version: 1,
            expressions: Vec::new(),
        }),
        Err(e) => Err(format!("读取现有自定义配置失败：{e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_delete_round_trip() {
        let dir =
            std::env::temp_dir().join(format!("yume_custom_expr_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src_dir = dir.join("src");
        let target_dir = dir.join("sakura");
        assert!(std::fs::create_dir_all(&src_dir).is_ok());
        let src_png = src_dir.join("star.png");
        assert!(std::fs::write(&src_png, b"pngbytes").is_ok());

        let spec = CustomExpressionSpec {
            character: "sakura".into(),
            id: "custom-1".into(),
            name: "小星星".into(),
            kind: ExpressionKind::Accessory,
            eyes: None,
            mouth: None,
            overlay: Some(AssetChoice::Import(src_png.to_string_lossy().into_owned())),
            random: true,
            duration_ms: 3200,
        };
        assert!(save_to_dir(&target_dir, &spec).is_ok());
        assert!(target_dir.join("custom-1_overlay.png").is_file());
        assert!(target_dir.join(EXPRESSIONS_FILE).is_file());

        assert!(delete_in_dir(&target_dir, "custom-1").is_ok());
        assert!(!target_dir.join("custom-1_overlay.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
