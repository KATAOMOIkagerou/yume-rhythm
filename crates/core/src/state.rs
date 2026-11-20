//! `AppState`：全部业务状态。字段对外只读，修改只能经过 `update`（docs/架构.md 第 3 节）。

use crate::model::audio::AudioState;
use crate::model::events::{ActivityMode, EventSwitches};
use crate::model::expression::{Expressions, Situation};
use crate::model::pet::{CharacterState, Dialog, PetState};
use crate::model::settings::Settings;
use crate::model::timer::StudyTimer;
use crate::model::todo::TodoList;
use crate::persist::{Persisted, StudyPrefs};

#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    pub(crate) settings: Settings,
    pub(crate) activity: ActivityMode,
    pub(crate) timer: StudyTimer,
    pub(crate) dialog: Dialog,
    pub(crate) switches: EventSwitches,
    pub(crate) expressions: Expressions,
    pub(crate) todo: TodoList,
    pub(crate) audio: AudioState,
    pub(crate) pet: PetState,
    pub(crate) character: CharacterState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Persisted::default())
    }
}

impl AppState {
    /// 由持久化数据构建初始状态。之后要发送一次 `Command::Startup`。
    #[must_use]
    pub fn new(persisted: Persisted) -> Self {
        let Persisted {
            settings,
            study,
            event_switches,
            todo,
            ..
        } = persisted;
        Self {
            settings,
            activity: ActivityMode::default(),
            timer: StudyTimer::new(study.minutes, study.mode),
            dialog: Dialog::None,
            switches: event_switches,
            expressions: Expressions::default(),
            todo,
            audio: AudioState::default(),
            pet: PetState::default(),
            character: CharacterState::default(),
        }
    }

    /// 需要持久化的部分。组装层在每次 `update` 后比较它，有变化就保存。
    #[must_use]
    pub fn persisted(&self) -> Persisted {
        Persisted {
            settings: self.settings.clone(),
            study: StudyPrefs {
                minutes: self.timer.minutes(),
                mode: self.timer.mode(),
            },
            event_switches: self.switches,
            todo: self.todo.clone(),
            ..Persisted::default()
        }
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    #[must_use]
    pub fn activity(&self) -> ActivityMode {
        self.activity
    }

    #[must_use]
    pub fn timer(&self) -> &StudyTimer {
        &self.timer
    }

    #[must_use]
    pub fn dialog(&self) -> Dialog {
        self.dialog
    }

    #[must_use]
    pub fn switches(&self) -> &EventSwitches {
        &self.switches
    }

    #[must_use]
    pub fn expressions(&self) -> &Expressions {
        &self.expressions
    }

    #[must_use]
    pub fn todo(&self) -> &TodoList {
        &self.todo
    }

    #[must_use]
    pub fn audio(&self) -> &AudioState {
        &self.audio
    }

    #[must_use]
    pub fn pet(&self) -> &PetState {
        &self.pet
    }

    #[must_use]
    pub fn character(&self) -> &CharacterState {
        &self.character
    }

    /// 判断表情是否可用的当前条件。
    #[must_use]
    pub fn situation(&self) -> Situation<'_> {
        Situation {
            activity: self.activity,
            music: self.audio.is_running(),
            switches: &self.switches,
        }
    }
}
