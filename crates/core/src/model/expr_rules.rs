//! 表情出现条件规则（功能清单第 6、7 节）。
//!
//! 取代旧版全局「随机事件开关矩阵」，改为每个表情各自配置场景条件、随机开关与持续时长，
//! 同时保留从旧版 `event_switches` 派生默认值的兼容逻辑。

use crate::model::events::{ActivityMode, EventContext, EventSwitches, RandomEvent};
use crate::model::expression::ExpressionMeta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DURATION_MIN_MS: u32 = 100;
pub const DURATION_MAX_MS: u32 = 600_000;

/// 表情允许随机出现的三个外部场景。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    pub chill: bool,
    pub study: bool,
    pub music: bool,
}

impl Conditions {
    pub const ALL_ON: Self = Self {
        chill: true,
        study: true,
        music: true,
    };

    #[must_use]
    pub const fn for_context(self, ctx: EventContext) -> bool {
        match ctx {
            EventContext::Chill => self.chill,
            EventContext::Study => self.study,
            EventContext::Music => self.music,
        }
    }

    pub fn set(&mut self, ctx: EventContext, on: bool) {
        match ctx {
            EventContext::Chill => self.chill = on,
            EventContext::Study => self.study = on,
            EventContext::Music => self.music = on,
        }
    }

    #[must_use]
    pub const fn allows(self, activity: ActivityMode, music: bool) -> bool {
        let activity_ok = match activity {
            ActivityMode::Chill => self.chill,
            ActivityMode::Study => self.study,
        };
        activity_ok && (!music || self.music)
    }
}

impl Default for Conditions {
    fn default() -> Self {
        Self::ALL_ON
    }
}

/// 单个表情的用户覆盖条件。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionRule {
    pub conditions: Conditions,
    pub random: bool,
    pub duration_ms: u32,
}

/// 按角色 id → 表情 id 存储的条件覆盖表。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionRules(BTreeMap<String, BTreeMap<String, ExpressionRule>>);

impl ExpressionRules {
    #[must_use]
    pub fn get(&self, character: &str, id: &str) -> Option<&ExpressionRule> {
        self.0.get(character).and_then(|m| m.get(id))
    }

    pub fn set(&mut self, character: &str, id: &str, rule: ExpressionRule) {
        self.0
            .entry(character.to_owned())
            .or_default()
            .insert(id.to_owned(), rule);
    }

    pub fn remove(&mut self, character: &str, id: &str) -> bool {
        let Some(map) = self.0.get_mut(character) else {
            return false;
        };
        let removed = map.remove(id).is_some();
        if map.is_empty() {
            self.0.remove(character);
        }
        removed
    }

    #[must_use]
    pub fn map(&self) -> &BTreeMap<String, BTreeMap<String, ExpressionRule>> {
        &self.0
    }
}

/// 规整显示时长到 100–600000 ms。
#[must_use]
pub fn clamp_duration_ms(ms: i64) -> u32 {
    let clamped = ms.clamp(i64::from(DURATION_MIN_MS), i64::from(DURATION_MAX_MS));
    u32::try_from(clamped).unwrap_or(3200)
}

/// 从原始角色包元数据和兼容开关矩阵推导出默认规则。
#[must_use]
pub fn default_rule(meta: &ExpressionMeta, legacy: &EventSwitches) -> ExpressionRule {
    let mut conditions = match meta.event {
        Some(event) => Conditions {
            chill: legacy.get(EventContext::Chill, event),
            study: legacy.get(EventContext::Study, event),
            music: legacy.get(EventContext::Music, event),
        },
        None => Conditions::ALL_ON,
    };
    if meta.event == Some(RandomEvent::Doze) {
        conditions.music = false;
    }
    ExpressionRule {
        conditions,
        random: meta.random,
        duration_ms: clamp_duration_ms(i64::from(meta.duration_ms)),
    }
}

/// 将用户规则覆盖到原始表情目录，并强制满足不变量（听歌时不可打瞌睡）。
#[must_use]
pub fn resolve_catalog(
    raw: &[ExpressionMeta],
    character: &str,
    rules: &ExpressionRules,
    legacy: &EventSwitches,
) -> Vec<ExpressionMeta> {
    raw.iter()
        .map(|meta| {
            let mut rule = rules
                .get(character, &meta.id)
                .cloned()
                .unwrap_or_else(|| default_rule(meta, legacy));
            if meta.event == Some(RandomEvent::Doze) {
                rule.conditions.music = false;
            }
            let mut out = meta.clone();
            out.conditions = rule.conditions;
            out.random = rule.random;
            out.duration_ms = clamp_duration_ms(i64::from(rule.duration_ms));
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::expression::ExpressionKind;

    fn sample_raw() -> Vec<ExpressionMeta> {
        vec![
            ExpressionMeta {
                id: "doze".into(),
                name: "打瞌睡".into(),
                kind: ExpressionKind::Accessory,
                event: Some(RandomEvent::Doze),
                duration_ms: 3200,
                random: true,
                ..Default::default()
            },
            ExpressionMeta {
                id: "think".into(),
                name: "思考".into(),
                kind: ExpressionKind::Accessory,
                event: Some(RandomEvent::Think),
                duration_ms: 3200,
                random: true,
                ..Default::default()
            },
        ]
    }

    #[test]
    fn resolve_applies_overrides_and_locks_music_doze() {
        let legacy = EventSwitches::default();
        let mut rules = ExpressionRules::default();
        rules.set(
            "sakura",
            "doze",
            ExpressionRule {
                conditions: Conditions::ALL_ON,
                random: true,
                duration_ms: 5000,
            },
        );
        rules.set(
            "sakura",
            "think",
            ExpressionRule {
                conditions: Conditions {
                    chill: true,
                    study: false,
                    music: true,
                },
                random: true,
                duration_ms: 4000,
            },
        );
        let out = resolve_catalog(&sample_raw(), "sakura", &rules, &legacy);
        assert!(!out[0].conditions.music);
        assert!(out[0].conditions.study);
        assert!(!out[1].conditions.study);
        assert_eq!(out[1].duration_ms, 4000);
    }
}
