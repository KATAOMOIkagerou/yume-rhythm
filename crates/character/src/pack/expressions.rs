//! expressions.json 的数据结构（docs/角色包格式.md 第 3 节）。

use serde::Deserialize;
use yume_core::model::events::RandomEvent;
use yume_core::model::expression::{ExpressionKind, ExpressionMeta};

pub const DURATION_MIN_MS: u32 = 100;
pub const DURATION_MAX_MS: u32 = 600_000;

#[derive(Clone, Debug, Deserialize)]
pub struct ExpressionsFile {
    #[serde(default)]
    pub version: u32,
    pub expressions: Vec<ExpressionDef>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KindDef {
    Base,
    Accessory,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    #[serde(default)]
    pub eyes: Option<String>,
    #[serde(default)]
    pub mouth: Option<String>,
    #[serde(default)]
    pub overlay: Option<String>,
    #[serde(default)]
    pub animation: Option<String>,
}

impl Resources {
    pub fn files(&self) -> impl Iterator<Item = &str> {
        [&self.eyes, &self.mouth, &self.overlay, &self.animation]
            .into_iter()
            .flatten()
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: KindDef,
    #[serde(default)]
    pub resources: Resources,
    #[serde(rename = "durationMs")]
    pub duration_ms: u32,
    #[serde(default)]
    pub random: bool,
    #[serde(default)]
    pub event: Option<RandomEvent>,
    #[serde(default = "default_true")]
    pub blink: bool,
}

fn default_true() -> bool {
    true
}

impl ExpressionDef {
    /// 交给 core 的元数据（不含图片）。
    #[must_use]
    pub fn meta(&self) -> ExpressionMeta {
        ExpressionMeta {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: match self.kind {
                KindDef::Base => ExpressionKind::Base,
                KindDef::Accessory => ExpressionKind::Accessory,
            },
            event: self.event,
            duration_ms: self.duration_ms,
            random: self.random,
        }
    }
}
