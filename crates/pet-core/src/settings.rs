use serde::{Deserialize, Serialize};

pub const MIN_RANDOM_INTERVAL_SECONDS: u32 = 5;
pub const MAX_RANDOM_INTERVAL_SECONDS: u32 = 86_400;
pub const DEFAULT_RANDOM_INTERVAL_SECONDS: u32 = 120;
pub const MIN_STUDY_MINUTES: u32 = 1;
pub const MAX_STUDY_MINUTES: u32 = 999;
pub const DEFAULT_STUDY_MINUTES: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityMode {
    Chill,
    Study,
}

impl Default for ActivityMode {
    fn default() -> Self {
        Self::Chill
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RandomEventType {
    Doze,
    Think,
    Shock,
    Love,
}

impl RandomEventType {
    pub const ALL: [RandomEventType; 4] = [
        RandomEventType::Doze,
        RandomEventType::Think,
        RandomEventType::Shock,
        RandomEventType::Love,
    ];

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Doze => "打瞌睡",
            Self::Think => "思考",
            Self::Shock => "震惊",
            Self::Love => "喜欢",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventContextKey {
    Chill,
    Study,
    Music,
}

impl EventContextKey {
    pub const ALL: [EventContextKey; 3] = [
        EventContextKey::Chill,
        EventContextKey::Study,
        EventContextKey::Music,
    ];

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Chill => "休闲",
            Self::Study => "专注",
            Self::Music => "音乐",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventSwitchRow {
    pub doze: bool,
    pub think: bool,
    pub shock: bool,
    pub love: bool,
}

impl Default for EventSwitchRow {
    fn default() -> Self {
        Self {
            doze: true,
            think: true,
            shock: true,
            love: true,
        }
    }
}

impl EventSwitchRow {
    pub fn get(&self, ev: RandomEventType) -> bool {
        match ev {
            RandomEventType::Doze => self.doze,
            RandomEventType::Think => self.think,
            RandomEventType::Shock => self.shock,
            RandomEventType::Love => self.love,
        }
    }

    pub fn set(&mut self, ev: RandomEventType, value: bool) {
        match ev {
            RandomEventType::Doze => self.doze = value,
            RandomEventType::Think => self.think = value,
            RandomEventType::Shock => self.shock = value,
            RandomEventType::Love => self.love = value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventSwitchMatrix {
    pub chill: EventSwitchRow,
    pub study: EventSwitchRow,
    pub music: EventSwitchRow,
}

impl Default for EventSwitchMatrix {
    fn default() -> Self {
        Self {
            chill: EventSwitchRow {
                doze: true,
                think: true,
                shock: true,
                love: true,
            },
            study: EventSwitchRow {
                doze: false,
                think: true,
                shock: true,
                love: true,
            },
            music: EventSwitchRow {
                doze: false,
                think: true,
                shock: true,
                love: true,
            },
        }
    }
}

impl EventSwitchMatrix {
    /// Enforce domain invariant: `music.doze` is strictly locked to `false`.
    pub fn sanitize(&mut self) {
        self.music.doze = false;
    }

    pub fn get(&self, ctx: EventContextKey, ev: RandomEventType) -> bool {
        if ctx == EventContextKey::Music && ev == RandomEventType::Doze {
            return false;
        }
        match ctx {
            EventContextKey::Chill => self.chill.get(ev),
            EventContextKey::Study => self.study.get(ev),
            EventContextKey::Music => self.music.get(ev),
        }
    }

    /// Toggles a switch and returns whether state actually changed.
    pub fn toggle(&mut self, ctx: EventContextKey, ev: RandomEventType) -> bool {
        if ctx == EventContextKey::Music && ev == RandomEventType::Doze {
            self.music.doze = false;
            return false;
        }
        let next = !self.get(ctx, ev);
        match ctx {
            EventContextKey::Chill => self.chill.set(ev, next),
            EventContextKey::Study => self.study.set(ev, next),
            EventContextKey::Music => self.music.set(ev, next),
        }
        self.sanitize();
        true
    }

    /// Matches legacy `isRandomEventAllowed(event, activityMode, musicRunning, eventSwitches)`.
    pub fn is_event_allowed(
        &self,
        event: RandomEventType,
        activity_mode: ActivityMode,
        music_running: bool,
    ) -> bool {
        if music_running && event == RandomEventType::Doze {
            return false;
        }
        let ctx = match activity_mode {
            ActivityMode::Chill => EventContextKey::Chill,
            ActivityMode::Study => EventContextKey::Study,
        };
        if !self.get(ctx, event) {
            return false;
        }
        if music_running {
            return self.get(EventContextKey::Music, event);
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GlobalSettings {
    pub end_sound_enabled: bool,
    pub random_enabled: bool,
    pub random_interval_seconds: u32,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            end_sound_enabled: true,
            random_enabled: true,
            random_interval_seconds: DEFAULT_RANDOM_INTERVAL_SECONDS,
        }
    }
}

impl GlobalSettings {
    pub fn sanitize(&mut self) {
        self.random_interval_seconds = self
            .random_interval_seconds
            .clamp(MIN_RANDOM_INTERVAL_SECONDS, MAX_RANDOM_INTERVAL_SECONDS);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TodoList {
    pub list_name: String,
    pub items: Vec<TodoItem>,
}

impl Default for TodoList {
    fn default() -> Self {
        Self {
            list_name: String::new(),
            items: default_todo_items(),
        }
    }
}

impl TodoList {
    pub fn sanitize(&mut self) {
        if self.list_name.chars().count() > 40 {
            self.list_name = self.list_name.chars().take(40).collect();
        }
        for item in &mut self.items {
            if item.text.chars().count() > 60 {
                item.text = item.text.chars().take(60).collect();
            }
        }
        if self.items.is_empty() {
            self.items = default_todo_items();
        }
    }
}

pub fn default_todo_items() -> Vec<TodoItem> {
    vec![
        TodoItem {
            id: "todo-1".into(),
            text: String::new(),
            done: false,
        },
        TodoItem {
            id: "todo-2".into(),
            text: String::new(),
            done: false,
        },
        TodoItem {
            id: "todo-3".into(),
            text: String::new(),
            done: false,
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PersistedState {
    pub settings: GlobalSettings,
    pub event_switches: EventSwitchMatrix,
    pub todo_list: TodoList,
    pub study_minutes: u32,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            settings: GlobalSettings::default(),
            event_switches: EventSwitchMatrix::default(),
            todo_list: TodoList::default(),
            study_minutes: DEFAULT_STUDY_MINUTES,
        }
    }
}

impl PersistedState {
    pub fn sanitize(&mut self) {
        self.settings.sanitize();
        self.event_switches.sanitize();
        self.todo_list.sanitize();
        self.study_minutes = self
            .study_minutes
            .clamp(MIN_STUDY_MINUTES, MAX_STUDY_MINUTES);
    }
}
