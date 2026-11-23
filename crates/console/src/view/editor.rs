//! 表情编辑器视图投影：列表项、场景摘要与新建/编辑草稿状态（不依赖 Slint）。

use super::file_name;
use yume_core::AppState;
use yume_core::model::editor::{AssetChoice, DraftTarget, EditorError};
use yume_core::model::expression::{ExpressionKind, ExpressionMeta};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorView {
    pub open: bool,
    pub warning: String,
    pub canvas_hint: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorRow {
    pub id: String,
    pub name: String,
    pub builtin: bool,
    pub primary: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorDraftView {
    pub active: bool,
    pub is_new: bool,
    pub builtin: bool,
    pub primary: bool,
    pub name: String,
    pub eyes_text: String,
    pub mouth_text: String,
    pub overlay_text: String,
    pub has_eyes: bool,
    pub has_mouth: bool,
    pub has_overlay: bool,
    pub chill: bool,
    pub study: bool,
    pub music: bool,
    pub music_locked: bool,
    pub random: bool,
    pub duration_s: i32,
    pub error: String,
    pub saving: bool,
    pub title: String,
}

pub(super) fn project_editor(state: &AppState) -> EditorView {
    let canvas_hint = match state.character().canvas {
        Some((w, h)) => format!("请使用 {w}×{h} 透明 PNG，与角色画布同尺寸"),
        None => "请使用与角色画布同尺寸的透明 PNG".to_owned(),
    };
    EditorView {
        open: state.editor().open,
        warning: state.character().custom_warning.clone().unwrap_or_default(),
        canvas_hint,
    }
}

pub(super) fn project_rows(state: &AppState) -> Vec<EditorRow> {
    state
        .expressions()
        .catalog()
        .iter()
        .map(|meta| EditorRow {
            id: meta.id.clone(),
            name: meta.name.clone(),
            builtin: !meta.custom,
            primary: meta.kind == ExpressionKind::Base,
            summary: summarize_meta(meta),
        })
        .collect()
}

pub(super) fn project_draft(state: &AppState) -> EditorDraftView {
    let editor = state.editor();
    let Some(draft) = editor.draft.as_ref() else {
        return EditorDraftView::default();
    };
    let is_new = matches!(draft.target, DraftTarget::New);
    let title = if is_new {
        "新建表情".to_owned()
    } else if draft.builtin {
        format!("编辑条件 · {}", draft.name)
    } else {
        format!("编辑表情 · {}", draft.name)
    };
    let duration_s =
        i32::try_from(((i64::from(draft.duration_ms) + 500) / 1000).clamp(1, 600)).unwrap_or(3);
    EditorDraftView {
        active: true,
        is_new,
        builtin: draft.builtin,
        primary: draft.kind == ExpressionKind::Base,
        name: draft.name.clone(),
        eyes_text: asset_label(draft.eyes.as_ref()),
        mouth_text: asset_label(draft.mouth.as_ref()),
        overlay_text: asset_label(draft.overlay.as_ref()),
        has_eyes: draft.eyes.is_some(),
        has_mouth: draft.mouth.is_some(),
        has_overlay: draft.overlay.is_some(),
        chill: draft.conditions.chill,
        study: draft.conditions.study,
        music: draft.conditions.music,
        music_locked: draft.doze_locked,
        random: draft.random,
        duration_s,
        error: draft
            .error
            .as_ref()
            .map(editor_error_text)
            .unwrap_or_default(),
        saving: editor.saving,
        title,
    }
}

fn summarize_meta(meta: &ExpressionMeta) -> String {
    if !meta.random {
        return "仅手动选择".to_owned();
    }
    let mut parts = Vec::with_capacity(3);
    if meta.conditions.chill {
        parts.push("休闲");
    }
    if meta.conditions.study {
        parts.push("自习");
    }
    if meta.conditions.music {
        parts.push("听歌");
    }
    let scenes = if parts.is_empty() {
        "不随机器触发".to_owned()
    } else {
        parts.join("·")
    };
    let dur = if meta.duration_ms.is_multiple_of(1000) {
        format!("{} 秒", meta.duration_ms / 1000)
    } else {
        format!("{:.1} 秒", f64::from(meta.duration_ms) / 1000.0)
    };
    format!("{scenes} · 随机 {dur}")
}

fn asset_label(choice: Option<&AssetChoice>) -> String {
    match choice {
        None => "未选择".to_owned(),
        Some(AssetChoice::Keep(rel)) => file_name(rel).to_owned(),
        Some(AssetChoice::Import(path)) => format!("已选择：{}", file_name(path)),
    }
}

fn editor_error_text(err: &EditorError) -> String {
    match err {
        EditorError::NameEmpty => "请给表情起个名字".to_owned(),
        EditorError::NameTooLong => "名字最多 12 个字".to_owned(),
        EditorError::NameDuplicate => "已经有同名表情了".to_owned(),
        EditorError::MissingFace => "一级表情需要眼睛和嘴巴两张图".to_owned(),
        EditorError::NoAsset => "至少导入一张素材".to_owned(),
        EditorError::AssetRejected(msg) | EditorError::SaveFailed(msg) => msg.clone(),
    }
}
