//! 表情编辑器模型：草稿、槽位、校验规则与 id 生成（功能清单第 6、7 节）。

use crate::model::events::RandomEvent;
use crate::model::expr_rules::Conditions;
use crate::model::expression::{ExpressionKind, ExpressionMeta};
use std::collections::HashSet;

pub const CUSTOM_EXPRESSION_NAME_MAX_CHARS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetSlot {
    Eyes,
    Mouth,
    Overlay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetChoice {
    Keep(String),
    Import(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftTarget {
    New,
    Existing(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorError {
    NameEmpty,
    NameTooLong,
    NameDuplicate,
    MissingFace,
    NoAsset,
    AssetRejected(String),
    SaveFailed(String),
}

/// 自定义表情保存请求，由组装层复制 PNG 并写入 `expressions.json`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomExpressionSpec {
    pub character: String,
    pub id: String,
    pub name: String,
    pub kind: ExpressionKind,
    pub eyes: Option<AssetChoice>,
    pub mouth: Option<AssetChoice>,
    pub overlay: Option<AssetChoice>,
    pub random: bool,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub target: DraftTarget,
    pub builtin: bool,
    pub kind: ExpressionKind,
    pub name: String,
    pub eyes: Option<AssetChoice>,
    pub mouth: Option<AssetChoice>,
    pub overlay: Option<AssetChoice>,
    pub conditions: Conditions,
    pub random: bool,
    pub duration_ms: u32,
    pub doze_locked: bool,
    pub error: Option<EditorError>,
}

impl Draft {
    #[must_use]
    pub fn new_blank() -> Self {
        Self {
            target: DraftTarget::New,
            builtin: false,
            kind: ExpressionKind::Base,
            name: String::new(),
            eyes: None,
            mouth: None,
            overlay: None,
            conditions: Conditions::ALL_ON,
            random: true,
            duration_ms: 3200,
            doze_locked: false,
            error: None,
        }
    }

    #[must_use]
    pub fn from_existing(meta: &ExpressionMeta) -> Self {
        Self {
            target: DraftTarget::Existing(meta.id.clone()),
            builtin: !meta.custom,
            kind: meta.kind,
            name: meta.name.clone(),
            eyes: meta.assets.eyes.clone().map(AssetChoice::Keep),
            mouth: meta.assets.mouth.clone().map(AssetChoice::Keep),
            overlay: meta.assets.overlay.clone().map(AssetChoice::Keep),
            conditions: meta.conditions,
            random: meta.random,
            duration_ms: meta.duration_ms,
            doze_locked: meta.event == Some(RandomEvent::Doze),
            error: None,
        }
    }

    pub fn slot_mut(&mut self, slot: AssetSlot) -> &mut Option<AssetChoice> {
        match slot {
            AssetSlot::Eyes => &mut self.eyes,
            AssetSlot::Mouth => &mut self.mouth,
            AssetSlot::Overlay => &mut self.overlay,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorState {
    pub open: bool,
    pub draft: Option<Draft>,
    pub saving: bool,
}

/// 校验草稿的名字和素材是否合法；内置表情跳过这两项校验。
///
/// # Errors
/// 违反任一约束时返回 `EditorError`。
pub fn validate_draft(draft: &Draft, existing: &[ExpressionMeta]) -> Result<(), EditorError> {
    if draft.builtin {
        return Ok(());
    }
    let trimmed = draft.name.trim();
    if trimmed.is_empty() {
        return Err(EditorError::NameEmpty);
    }
    if trimmed.chars().count() > CUSTOM_EXPRESSION_NAME_MAX_CHARS {
        return Err(EditorError::NameTooLong);
    }
    let self_id = match &draft.target {
        DraftTarget::New => None,
        DraftTarget::Existing(id) => Some(id.as_str()),
    };
    let duplicate = existing
        .iter()
        .any(|m| Some(m.id.as_str()) != self_id && m.name.trim() == trimmed);
    if duplicate {
        return Err(EditorError::NameDuplicate);
    }
    match draft.kind {
        ExpressionKind::Base => {
            if draft.eyes.is_none() || draft.mouth.is_none() {
                return Err(EditorError::MissingFace);
            }
        }
        ExpressionKind::Accessory => {
            if draft.eyes.is_none() && draft.mouth.is_none() && draft.overlay.is_none() {
                return Err(EditorError::NoAsset);
            }
        }
    }
    Ok(())
}

/// 取当前目录中最小空闲的 `custom-N` 正整数 id。
#[must_use]
pub fn next_custom_id(raw_catalog: &[ExpressionMeta]) -> String {
    let used: HashSet<&str> = raw_catalog.iter().map(|m| m.id.as_str()).collect();
    let mut n = 1usize;
    loop {
        let candidate = format!("custom-{n}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}
