//! 持久化数据结构与宽松读取（docs/架构.md 第 8 节）。
//!
//! 读取永不失败：整体损坏时用默认值，单个字段非法时只回退那个字段。

use crate::model::events::{EventContext, EventSwitches, RandomEvent};
use crate::model::expr_rules::{ExpressionRule, ExpressionRules, clamp_duration_ms};
use crate::model::settings::{
    PetPosition, Settings, clamp_background_dim, clamp_console_opacity, clamp_pet_opacity,
    clamp_pet_scale, clamp_random_interval,
};
use crate::model::shortcuts::Shortcuts;
use crate::model::theme::{
    AccentPreset, BackgroundMode, migrate_legacy_theme, normalize_hex_color,
};
use crate::model::timer::{DEFAULT_MINUTES, TimerMode, clamp_minutes};
use crate::model::todo::{TodoItem, TodoList};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyPrefs {
    pub minutes: u32,
    pub mode: TimerMode,
}

impl Default for StudyPrefs {
    fn default() -> Self {
        Self {
            minutes: DEFAULT_MINUTES,
            mode: TimerMode::Countdown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Persisted {
    pub version: u32,
    pub settings: Settings,
    pub study: StudyPrefs,
    pub event_switches: EventSwitches,
    pub expression_rules: ExpressionRules,
    pub todo: TodoList,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            settings: Settings::default(),
            study: StudyPrefs::default(),
            event_switches: EventSwitches::default(),
            expression_rules: ExpressionRules::default(),
            todo: TodoList::default(),
        }
    }
}

impl Persisted {
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// 宽松读取。任何错误都回退到默认值，不会失败。
    #[must_use]
    pub fn from_json(text: &str) -> Self {
        // 用记事本另存过的文件可能带 UTF-8 BOM。
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let Ok(root) = serde_json::from_str::<Value>(text) else {
            log::warn!("配置文件无法解析，使用默认值");
            return Self::default();
        };
        Self {
            version: FORMAT_VERSION,
            settings: read_settings(&root["settings"]),
            study: read_study(&root["study"]),
            event_switches: read_switches(&root["event_switches"]),
            expression_rules: read_expression_rules(&root["expression_rules"]),
            todo: read_todo(&root["todo"]),
        }
    }
}

fn field<T: DeserializeOwned>(obj: &Value, key: &str) -> Option<T> {
    obj.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn read_shortcuts(v: Option<&Value>) -> Shortcuts {
    let d = Shortcuts::default();
    let Some(obj) = v else {
        return d;
    };
    let mut s = Shortcuts {
        global_toggle_console: field(obj, "global_toggle_console")
            .unwrap_or(d.global_toggle_console),
        global_toggle_locked: field(obj, "global_toggle_locked").unwrap_or(d.global_toggle_locked),
        global_toggle_audio: field(obj, "global_toggle_audio").unwrap_or(d.global_toggle_audio),
        local_audio_toggle: field(obj, "local_audio_toggle").unwrap_or(d.local_audio_toggle),
        local_audio_stop: field(obj, "local_audio_stop").unwrap_or(d.local_audio_stop),
        local_hide_console: field(obj, "local_hide_console").unwrap_or(d.local_hide_console),
    };
    s.sanitize();
    s
}

fn read_settings(v: &Value) -> Settings {
    let d = Settings::default();
    let shortcuts = read_shortcuts(v.get("shortcuts"));
    let legacy = if v.get("background").is_none() && v.get("accent").is_none() {
        field::<String>(v, "theme").map(|k| migrate_legacy_theme(&k))
    } else {
        None
    };
    let background = field::<BackgroundMode>(v, "background")
        .unwrap_or_else(|| legacy.map_or(d.background, |m| m.0));
    let custom_background = field::<Option<String>>(v, "custom_background")
        .flatten()
        .and_then(|raw| normalize_hex_color(&raw));
    let accent =
        field::<AccentPreset>(v, "accent").unwrap_or_else(|| legacy.map_or(d.accent, |m| m.1));
    let custom_accent = field::<Option<String>>(v, "custom_accent")
        .flatten()
        .and_then(|raw| normalize_hex_color(&raw));
    let background_image = trimmed_opt(v, "background_image");
    let font_family = trimmed_opt(v, "font_family");
    let custom_font_path = trimmed_opt(v, "custom_font_path");
    let todo_md_path = trimmed_opt(v, "todo_md_path");

    Settings {
        end_sound: field(v, "end_sound").unwrap_or(d.end_sound),
        random_enabled: field(v, "random_enabled").unwrap_or(d.random_enabled),
        random_interval_s: field::<i64>(v, "random_interval_s")
            .map_or(d.random_interval_s, clamp_random_interval),
        show_bpm: field(v, "show_bpm").unwrap_or(d.show_bpm),
        auto_follow: field(v, "auto_follow").unwrap_or(d.auto_follow),
        audio_device: field::<Option<String>>(v, "audio_device").unwrap_or(d.audio_device),
        pet_scale: field::<f32>(v, "pet_scale").map_or(d.pet_scale, clamp_pet_scale),
        pet_position: field::<Option<PetPosition>>(v, "pet_position").unwrap_or(d.pet_position),
        autostart: field(v, "autostart").unwrap_or(d.autostart),
        hide_on_fullscreen: field(v, "hide_on_fullscreen").unwrap_or(d.hide_on_fullscreen),
        character: field::<String>(v, "character")
            .filter(|id| is_valid_character_id(id))
            .unwrap_or(d.character),
        background,
        custom_background,
        accent,
        custom_accent,
        background_image,
        background_dim: field::<f32>(v, "background_dim")
            .map_or(d.background_dim, clamp_background_dim),
        console_opacity: field::<f32>(v, "console_opacity")
            .map_or(d.console_opacity, clamp_console_opacity),
        pet_opacity: field::<f32>(v, "pet_opacity").map_or(d.pet_opacity, clamp_pet_opacity),
        shortcuts,
        font_family,
        custom_font_path,
        todo_md_path,
    }
}

fn trimmed_opt(v: &Value, key: &str) -> Option<String> {
    field::<Option<String>>(v, key)
        .flatten()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// 角色 id 只允许小写字母、数字、`-`、`_`，防止被拼成任意路径。
#[must_use]
pub fn is_valid_character_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn read_study(v: &Value) -> StudyPrefs {
    let d = StudyPrefs::default();
    StudyPrefs {
        minutes: field::<i64>(v, "minutes").map_or(d.minutes, clamp_minutes),
        mode: field(v, "mode").unwrap_or(d.mode),
    }
}

fn read_switches(v: &Value) -> EventSwitches {
    let mut switches = EventSwitches::default();
    for ctx in EventContext::ALL {
        for event in RandomEvent::ALL {
            if let Some(on) = v[ctx.key()][event.key()].as_bool() {
                switches.set(ctx, event, on);
            }
        }
    }
    switches.normalize();
    switches
}

fn read_expression_rules(v: &Value) -> ExpressionRules {
    let mut out = ExpressionRules::default();
    let Some(chars) = v.as_object() else {
        return out;
    };
    for (char_id, exprs) in chars {
        if !is_valid_character_id(char_id) {
            continue;
        }
        let Some(expr_map) = exprs.as_object() else {
            continue;
        };
        for (expr_id, val) in expr_map {
            if expr_id.is_empty() || expr_id.len() > 64 {
                continue;
            }
            if let Ok(mut rule) = serde_json::from_value::<ExpressionRule>(val.clone()) {
                rule.duration_ms = clamp_duration_ms(i64::from(rule.duration_ms));
                out.set(char_id, expr_id, rule);
            }
        }
    }
    out
}

fn read_todo(v: &Value) -> TodoList {
    let name = field::<String>(v, "name").unwrap_or_default();
    let items = v["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| serde_json::from_value::<TodoItem>(i.clone()).ok())
        })
        .map(Iterator::collect)
        .unwrap_or_default();
    TodoList::from_parts(name, items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerates_utf8_bom() {
        let mut p = Persisted::default();
        p.settings.pet_scale = 1.5;
        let text = format!("\u{feff}{}", p.to_json());
        assert_eq!(Persisted::from_json(&text), p);
    }

    #[test]
    fn round_trips() {
        let mut p = Persisted::default();
        p.settings.random_interval_s = 30;
        p.settings.pet_position = Some(PetPosition { x: -100, y: 20 });
        p.settings.background = BackgroundMode::Custom;
        p.settings.custom_background = Some("#222530".into());
        p.settings.accent = AccentPreset::Sunset;
        p.settings.custom_accent = Some("#4a90e2".into());
        p.settings.background_image = Some("C:/img/bg.png".into());
        p.settings.background_dim = 0.75;
        p.settings.console_opacity = 0.85;
        p.settings.pet_opacity = 0.7;
        p.settings.shortcuts.global_toggle_audio = "Ctrl+Alt+M".into();
        p.settings.shortcuts.local_hide_console = "F9".into();
        p.settings.font_family = Some("LXGW WenKai".into());
        p.settings.custom_font_path = Some("C:/fonts/wenkai.ttf".into());
        p.settings.todo_md_path = Some("D:/notes/todo.md".into());
        p.expression_rules.set(
            "sakura",
            "think",
            ExpressionRule {
                conditions: crate::model::expr_rules::Conditions {
                    chill: true,
                    study: false,
                    music: true,
                },
                random: false,
                duration_ms: 4500,
            },
        );
        p.study.minutes = 45;
        p.todo.rename("今天");
        assert_eq!(Persisted::from_json(&p.to_json()), p);
    }

    #[test]
    fn garbage_falls_back_to_defaults() {
        assert_eq!(Persisted::from_json("not json"), Persisted::default());
        assert_eq!(Persisted::from_json("[]"), Persisted::default());
    }

    #[test]
    fn bad_fields_fall_back_individually() {
        let text = r#"{
            "settings": { "end_sound": "yes", "random_enabled": false, "random_interval_s": 1,
                          "pet_scale": 99, "pet_opacity": 0.05, "console_opacity": 5.0,
                          "background": "unknown", "custom_background": "bad",
                          "accent": "unknown", "custom_accent": "not-a-color",
                          "background_image": "   ", "background_dim": 9.0,
                          "shortcuts": { "global_toggle_console": "Space", "local_hide_console": "F2" },
                          "font_family": "   ", "custom_font_path": "",
                          "todo_md_path": "  ", "character": "../../evil" },
            "study": { "minutes": 0 },
            "event_switches": { "music": { "doze": true, "love": false } },
            "expression_rules": {
                "../evil": { "x": { "conditions": { "chill": true, "study": true, "music": true }, "random": true, "duration_ms": 1000 } },
                "sakura": {
                    "bad": 123,
                    "happy": { "conditions": { "chill": true, "study": false, "music": true }, "random": true, "duration_ms": 1 }
                }
            },
            "todo": { "name": 3, "items": [ { "id": 1, "text": "背单词", "done": true }, 42 ] }
        }"#;
        let p = Persisted::from_json(text);
        assert!(p.settings.end_sound);
        assert!(!p.settings.random_enabled);
        assert_eq!(p.settings.random_interval_s, 5);
        assert!((p.settings.pet_scale - 2.0).abs() < f32::EPSILON);
        assert!((p.settings.pet_opacity - 0.3).abs() < f32::EPSILON);
        assert!((p.settings.console_opacity - 1.0).abs() < f32::EPSILON);
        assert_eq!(p.settings.background, BackgroundMode::Light);
        assert_eq!(p.settings.custom_background, None);
        assert_eq!(p.settings.accent, AccentPreset::Sakura);
        assert_eq!(p.settings.custom_accent, None);
        assert_eq!(p.settings.background_image, None);
        assert!((p.settings.background_dim - 0.95).abs() < f32::EPSILON);
        assert_eq!(p.settings.shortcuts.global_toggle_console, "Ctrl+Alt+C");
        assert_eq!(p.settings.shortcuts.local_hide_console, "F2");
        assert_eq!(p.settings.font_family, None);
        assert_eq!(p.settings.custom_font_path, None);
        assert_eq!(p.settings.todo_md_path, None);
        assert_eq!(p.settings.character, "sakura");
        assert_eq!(p.study.minutes, 1);
        assert!(!p.event_switches.music.doze);
        assert!(!p.event_switches.music.love);
        assert!(p.expression_rules.get("../evil", "x").is_none());
        let happy_rule = p.expression_rules.get("sakura", "happy");
        assert_eq!(happy_rule.map(|r| r.duration_ms), Some(100));
        assert_eq!(p.todo.items().len(), 1);
        assert_eq!(p.todo.items()[0].text, "背单词");
    }

    #[test]
    fn legacy_theme_migrates_when_new_fields_absent() {
        let night =
            Persisted::from_json(r##"{"settings":{"theme":"night","custom_accent":"#123456"}}"##);
        assert_eq!(night.settings.background, BackgroundMode::Dark);
        assert_eq!(night.settings.accent, AccentPreset::Sakura);
        assert_eq!(night.settings.custom_accent.as_deref(), Some("#123456"));

        let peach = Persisted::from_json(r#"{"settings":{"theme":"peach"}}"#);
        assert_eq!(peach.settings.background, BackgroundMode::Light);
        assert_eq!(peach.settings.accent, AccentPreset::Sunset);
    }
}
