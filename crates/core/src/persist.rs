//! 持久化数据结构与宽松读取（docs/架构.md 第 8 节）。
//!
//! 读取永不失败：整体损坏时用默认值，单个字段非法时只回退那个字段。

use crate::model::events::{EventContext, EventSwitches, RandomEvent};
use crate::model::settings::{PetPosition, Settings, clamp_pet_scale, clamp_random_interval};
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
    pub todo: TodoList,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            settings: Settings::default(),
            study: StudyPrefs::default(),
            event_switches: EventSwitches::default(),
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
            todo: read_todo(&root["todo"]),
        }
    }
}

fn field<T: DeserializeOwned>(obj: &Value, key: &str) -> Option<T> {
    obj.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn read_settings(v: &Value) -> Settings {
    let d = Settings::default();
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
    }
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
                          "pet_scale": 99, "character": "../../evil" },
            "study": { "minutes": 0 },
            "event_switches": { "music": { "doze": true, "love": false } },
            "todo": { "name": 3, "items": [ { "id": 1, "text": "背单词", "done": true }, 42 ] }
        }"#;
        let p = Persisted::from_json(text);
        assert!(p.settings.end_sound);
        assert!(!p.settings.random_enabled);
        assert_eq!(p.settings.random_interval_s, 5);
        assert!((p.settings.pet_scale - 2.0).abs() < f32::EPSILON);
        assert_eq!(p.settings.character, "sakura");
        assert_eq!(p.study.minutes, 1);
        assert!(!p.event_switches.music.doze);
        assert!(!p.event_switches.music.love);
        assert_eq!(p.todo.items().len(), 1);
        assert_eq!(p.todo.items()[0].text, "背单词");
    }
}
