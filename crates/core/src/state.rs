//! `AppState`：全部业务状态。字段对外只读，修改只能经过 `update`（docs/架构.md 第 3 节）。

use crate::model::audio::AudioState;
use crate::model::editor::EditorState;
use crate::model::events::{ActivityMode, EventSwitches};
use crate::model::expr_rules::ExpressionRules;
use crate::model::expression::{ExpressionMeta, Expressions, Situation};
use crate::model::pet::{CharacterState, Dialog, PetState};
use crate::model::settings::Settings;
use crate::model::shortcuts::{ShortcutAction, ShortcutError};
use crate::model::timer::StudyTimer;
use crate::model::todo::TodoList;
use crate::model::todo_md::{TodoMdDoc, TodoMdError};
use crate::persist::{Persisted, StudyPrefs};

#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    pub(crate) settings: Settings,
    pub(crate) activity: ActivityMode,
    pub(crate) timer: StudyTimer,
    pub(crate) dialog: Dialog,
    pub(crate) switches: EventSwitches,
    pub(crate) expression_rules: ExpressionRules,
    pub(crate) raw_catalog: Vec<ExpressionMeta>,
    pub(crate) expressions: Expressions,
    pub(crate) editor: EditorState,
    pub(crate) todo: TodoList,
    pub(crate) todo_md_doc: Option<TodoMdDoc>,
    pub(crate) todo_md_error: Option<TodoMdError>,
    pub(crate) todo_md_dirty_seq: u64,
    pub(crate) bg_error: bool,
    pub(crate) bg_image_error: Option<String>,
    pub(crate) accent_error: bool,
    pub(crate) shortcut_error: Option<ShortcutError>,
    pub(crate) global_hotkeys_failed: Vec<ShortcutAction>,
    pub(crate) available_fonts: Vec<String>,
    pub(crate) font_error: Option<String>,
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
            expression_rules,
            todo,
            ..
        } = persisted;
        Self {
            settings,
            activity: ActivityMode::default(),
            timer: StudyTimer::new(study.minutes, study.mode),
            dialog: Dialog::None,
            switches: event_switches,
            expression_rules,
            raw_catalog: Vec::new(),
            expressions: Expressions::default(),
            editor: EditorState::default(),
            todo,
            todo_md_doc: None,
            todo_md_error: None,
            todo_md_dirty_seq: 0,
            bg_error: false,
            bg_image_error: None,
            accent_error: false,
            shortcut_error: None,
            global_hotkeys_failed: Vec::new(),
            available_fonts: Vec::new(),
            font_error: None,
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
            expression_rules: self.expression_rules.clone(),
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
    pub fn expression_rules(&self) -> &ExpressionRules {
        &self.expression_rules
    }

    #[must_use]
    pub fn raw_catalog(&self) -> &[ExpressionMeta] {
        &self.raw_catalog
    }

    #[must_use]
    pub fn expressions(&self) -> &Expressions {
        &self.expressions
    }

    #[must_use]
    pub fn editor(&self) -> &EditorState {
        &self.editor
    }

    #[must_use]
    pub fn todo(&self) -> &TodoList {
        &self.todo
    }

    #[must_use]
    pub fn todo_md_doc(&self) -> Option<&TodoMdDoc> {
        self.todo_md_doc.as_ref()
    }

    #[must_use]
    pub fn todo_md_error(&self) -> Option<TodoMdError> {
        self.todo_md_error
    }

    #[must_use]
    pub fn todo_md_dirty_seq(&self) -> u64 {
        self.todo_md_dirty_seq
    }

    #[must_use]
    pub fn bg_error(&self) -> bool {
        self.bg_error
    }

    #[must_use]
    pub fn bg_image_error(&self) -> Option<&str> {
        self.bg_image_error.as_deref()
    }

    #[must_use]
    pub fn accent_error(&self) -> bool {
        self.accent_error
    }

    #[must_use]
    pub fn shortcut_error(&self) -> Option<&ShortcutError> {
        self.shortcut_error.as_ref()
    }

    #[must_use]
    pub fn global_hotkeys_failed(&self) -> &[ShortcutAction] {
        &self.global_hotkeys_failed
    }

    #[must_use]
    pub fn available_fonts(&self) -> &[String] {
        &self.available_fonts
    }

    #[must_use]
    pub fn font_error(&self) -> Option<&str> {
        self.font_error.as_deref()
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
    pub fn situation(&self) -> Situation {
        Situation {
            activity: self.activity,
            music: self.audio.is_running(),
        }
    }
}
