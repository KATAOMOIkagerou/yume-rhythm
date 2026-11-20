use crate::manifest::CharacterManifest;
use crate::settings::RandomEventType;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpressionType {
    Base,
    Accessory,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionResources {
    #[serde(default)]
    pub animation: Option<String>,
    #[serde(default)]
    pub eyes: Option<String>,
    #[serde(default)]
    pub mouth: Option<String>,
    #[serde(default)]
    pub overlay: Option<String>,
}

impl ExpressionResources {
    pub fn file_paths(&self) -> Vec<&str> {
        let mut out = Vec::new();
        if let Some(v) = &self.animation {
            out.push(v.as_str());
        }
        if let Some(v) = &self.eyes {
            out.push(v.as_str());
        }
        if let Some(v) = &self.mouth {
            out.push(v.as_str());
        }
        if let Some(v) = &self.overlay {
            out.push(v.as_str());
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.animation.is_none()
            && self.eyes.is_none()
            && self.mouth.is_none()
            && self.overlay.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressionDefinition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ExpressionType,
    pub resources: ExpressionResources,
    pub duration_ms: u32,
    pub random: bool,
    #[serde(default)]
    pub event: Option<RandomEventType>,
}

#[derive(Debug, Deserialize)]
struct ExpressionsFileRaw {
    #[allow(dead_code)]
    #[serde(default)]
    version: Option<u32>,
    expressions: Vec<ExpressionDefinition>,
}

/// Validates a relative resource path inside the character asset directory,
/// rejecting drive letters, leading slashes, query/fragment chars, and `..` traversal.
pub fn is_safe_relative_asset_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return false;
    }
    if trimmed.contains([':', '?', '#', '\0']) {
        return false;
    }
    for part in trimmed.split(['/', '\\']) {
        if part == ".." || part.is_empty() {
            return false;
        }
    }
    true
}

fn is_valid_expression_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Parses and validates `expressions.json` with identical rules and Chinese errors as legacy `expressions.ts`.
pub fn parse_expressions(raw: &str) -> Result<Vec<ExpressionDefinition>, String> {
    let parsed: ExpressionsFileRaw =
        serde_json::from_str(raw).map_err(|_| "表情配置必须包含有效的表情列表".to_string())?;
    let mut ids = HashSet::new();
    let mut list = Vec::with_capacity(parsed.expressions.len());

    for mut entry in parsed.expressions {
        if !is_valid_expression_id(&entry.id)
            || ids.contains(&entry.id)
            || entry.name.trim().is_empty()
            || entry.duration_ms < 100
            || entry.duration_ms > 600_000
        {
            return Err("表情配置的标识、名称、类型、时长或随机开关无效".into());
        }

        let validate_opt = |opt: &mut Option<String>, name: &str| -> Result<(), String> {
            if let Some(val) = opt {
                if !is_safe_relative_asset_path(val) {
                    return Err(format!("表情「{name}」的资源必须使用素材目录内的相对路径"));
                }
                *val = val.trim().replace('\\', "/");
            }
            Ok(())
        };

        validate_opt(&mut entry.resources.animation, &entry.name)?;
        validate_opt(&mut entry.resources.eyes, &entry.name)?;
        validate_opt(&mut entry.resources.mouth, &entry.name)?;
        validate_opt(&mut entry.resources.overlay, &entry.name)?;

        let has_playable = match entry.kind {
            ExpressionType::Base => {
                entry.resources.animation.is_some()
                    || (entry.resources.eyes.is_some() && entry.resources.mouth.is_some())
            }
            ExpressionType::Accessory => !entry.resources.is_empty(),
        };
        if !has_playable {
            return Err(format!("表情「{}」缺少可播放资源", entry.name));
        }

        ids.insert(entry.id.clone());
        list.push(entry);
    }

    if !list
        .iter()
        .any(|item| item.id == "neutral" && item.kind == ExpressionType::Base)
    {
        return Err("表情列表必须包含自然表情".into());
    }

    Ok(list)
}

/// Generates compatible expression entries from a legacy `manifest.json` lacking `expressions.json`.
pub fn legacy_expressions(manifest: &CharacterManifest) -> Vec<ExpressionDefinition> {
    vec![
        ExpressionDefinition {
            id: "neutral".into(),
            name: "自然".into(),
            kind: ExpressionType::Base,
            resources: ExpressionResources {
                eyes: Some(manifest.expressions.neutral.eyes.clone()),
                mouth: Some(manifest.expressions.neutral.mouth.clone()),
                ..Default::default()
            },
            duration_ms: 3200,
            random: false,
            event: None,
        },
        ExpressionDefinition {
            id: "happy".into(),
            name: "开心".into(),
            kind: ExpressionType::Base,
            resources: ExpressionResources {
                eyes: Some(manifest.expressions.happy.eyes.clone()),
                mouth: Some(manifest.expressions.happy.mouth.clone()),
                ..Default::default()
            },
            duration_ms: 3200,
            random: true,
            event: None,
        },
        ExpressionDefinition {
            id: "sad".into(),
            name: "难过".into(),
            kind: ExpressionType::Base,
            resources: ExpressionResources {
                eyes: Some(manifest.expressions.sad.eyes.clone()),
                mouth: Some(manifest.expressions.sad.mouth.clone()),
                ..Default::default()
            },
            duration_ms: 3200,
            random: true,
            event: None,
        },
        ExpressionDefinition {
            id: "doze".into(),
            name: "打瞌睡".into(),
            kind: ExpressionType::Accessory,
            resources: ExpressionResources {
                eyes: Some(manifest.random_events.doze.eyes.clone()),
                mouth: Some(manifest.random_events.doze.mouth.clone()),
                overlay: manifest.random_events.doze.fx.clone(),
                animation: None,
            },
            duration_ms: 3200,
            random: true,
            event: Some(RandomEventType::Doze),
        },
        ExpressionDefinition {
            id: "think".into(),
            name: "思考".into(),
            kind: ExpressionType::Accessory,
            resources: ExpressionResources {
                eyes: Some(manifest.random_events.think.eyes.clone()),
                mouth: Some(manifest.random_events.think.mouth.clone()),
                overlay: manifest.random_events.think.fx.clone(),
                animation: None,
            },
            duration_ms: 3200,
            random: true,
            event: Some(RandomEventType::Think),
        },
        ExpressionDefinition {
            id: "shock".into(),
            name: "震惊".into(),
            kind: ExpressionType::Accessory,
            resources: ExpressionResources {
                eyes: Some(manifest.random_events.shock.eyes.clone()),
                mouth: Some(manifest.random_events.shock.mouth.clone()),
                overlay: manifest.random_events.shock.fx.clone(),
                animation: None,
            },
            duration_ms: 3200,
            random: true,
            event: Some(RandomEventType::Shock),
        },
        ExpressionDefinition {
            id: "love".into(),
            name: "喜欢".into(),
            kind: ExpressionType::Accessory,
            resources: ExpressionResources {
                eyes: Some(manifest.random_events.love.eyes.clone()),
                mouth: Some(manifest.random_events.love.mouth.clone()),
                overlay: manifest.random_events.love.fx.clone(),
                animation: None,
            },
            duration_ms: 3200,
            random: true,
            event: Some(RandomEventType::Love),
        },
    ]
}
