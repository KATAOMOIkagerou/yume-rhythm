//! expressions.json 的数据结构（docs/角色包格式.md 第 3 节）。

use serde::{Deserialize, Serialize};
use yume_core::model::events::RandomEvent;
use yume_core::model::expr_rules::Conditions;
use yume_core::model::expression::{ExpressionAssets, ExpressionKind, ExpressionMeta};

pub const DURATION_MIN_MS: u32 = 100;
pub const DURATION_MAX_MS: u32 = 600_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExpressionsFile {
    #[serde(default)]
    pub version: u32,
    pub expressions: Vec<ExpressionDef>,
}

impl ExpressionsFile {
    /// 解析 `expressions.json` 字节流。
    ///
    /// # Errors
    /// JSON 语法或字段不匹配时返回可读错误。
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("读取表达式配置失败：{e}"))
    }

    /// 序列化为格式化 JSON 字符串，供保存自定义表情目录使用。
    ///
    /// # Errors
    /// 序列化失败时返回描述。
    pub fn to_pretty_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("序列化失败：{e}"))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KindDef {
    Base,
    Accessory,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eyes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouth: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
}

impl Resources {
    pub fn files(&self) -> impl Iterator<Item = &str> {
        [&self.eyes, &self.mouth, &self.overlay, &self.animation]
            .into_iter()
            .flatten()
            .map(String::as_str)
    }

    pub fn prefix_dir(&mut self, prefix: &str) {
        for slot in [
            &mut self.eyes,
            &mut self.mouth,
            &mut self.overlay,
            &mut self.animation,
        ]
        .into_iter()
        .flatten()
        {
            *slot = format!("{prefix}{slot}");
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<RandomEvent>,
    #[serde(default = "default_true")]
    pub blink: bool,
    #[serde(skip)]
    pub custom: bool,
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
            conditions: Conditions::ALL_ON,
            custom: self.custom,
            assets: ExpressionAssets {
                eyes: self.resources.eyes.clone(),
                mouth: self.resources.mouth.clone(),
                overlay: self.resources.overlay.clone(),
            },
        }
    }
}
