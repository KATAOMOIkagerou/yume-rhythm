//! 把 `View` 写进 Slint，把界面回调翻译成 `Command`。
//!
//! 只推送有变化的部分；列表原地更新（不整体替换），避免正在输入的待办失去焦点。

use crate::ui::{self, ConsoleWindow, Store, Theme};
use crate::view::{self, AudioPhase, TimerPhase, View};
use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use yume_core::command::{
    AudioCommand, CharacterCommand, ExpressionCommand, PetCommand, SettingsCommand, StudyCommand,
    TodoCommand,
};
use yume_core::model::audio::TempoShift;
use yume_core::model::events::{ActivityMode, EventContext, RandomEvent};
use yume_core::model::timer::TimerMode;
use yume_core::{AppState, Command, WallMs};

/// 控制台窗口。只能在 Slint 事件循环所在的线程上使用。
pub struct Console {
    window: ConsoleWindow,
    /// 最近一次推送的内容。回调用它把下标换成 id。
    shown: Rc<RefCell<Option<View>>>,
    models: Models,
}

struct Models {
    todo_items: Rc<VecModel<ui::TodoRow>>,
    base: Rc<VecModel<ui::ExpressionChip>>,
    accessory: Rc<VecModel<ui::ExpressionChip>>,
    event_columns: Rc<VecModel<SharedString>>,
    event_rows: Rc<VecModel<ui::EventRow>>,
    devices: Rc<VecModel<SharedString>>,
    characters: Rc<VecModel<SharedString>>,
}

impl Console {
    /// 创建窗口（不显示）。`dispatch` 收到用户操作产生的命令。
    ///
    /// # Errors
    /// 无法创建窗口（例如没有可用的图形后端）时返回错误。
    pub fn new(dispatch: Rc<dyn Fn(Command)>) -> Result<Self, slint::PlatformError> {
        let window = ConsoleWindow::new()?;
        let models = Models {
            todo_items: Rc::new(VecModel::default()),
            base: Rc::new(VecModel::default()),
            accessory: Rc::new(VecModel::default()),
            event_columns: Rc::new(VecModel::default()),
            event_rows: Rc::new(VecModel::default()),
            devices: Rc::new(VecModel::default()),
            characters: Rc::new(VecModel::default()),
        };
        let store = window.global::<Store>();
        store.set_todo_items(ModelRc::from(models.todo_items.clone()));
        store.set_base_expressions(ModelRc::from(models.base.clone()));
        store.set_accessory_expressions(ModelRc::from(models.accessory.clone()));
        store.set_event_columns(ModelRc::from(models.event_columns.clone()));
        store.set_event_rows(ModelRc::from(models.event_rows.clone()));
        store.set_devices(ModelRc::from(models.devices.clone()));
        store.set_characters(ModelRc::from(models.characters.clone()));

        let shown = Rc::new(RefCell::new(None));
        wire(&store, &dispatch, &shown);
        let close = dispatch.clone();
        window.window().on_close_requested(move || {
            close(Command::Pet(PetCommand::HideConsole));
            CloseRequestResponse::HideWindow
        });
        Ok(Self {
            window,
            shown,
            models,
        })
    }

    #[must_use]
    pub fn window(&self) -> &ConsoleWindow {
        &self.window
    }

    /// 显示或隐藏窗口。
    ///
    /// # Errors
    /// 窗口系统拒绝时返回错误。
    pub fn set_visible(&self, visible: bool) -> Result<(), slint::PlatformError> {
        if visible {
            self.window.show()
        } else {
            self.window.hide()
        }
    }

    /// 控制台的原生窗口句柄（Windows 上是 HWND）。窗口显示过之后才有。
    #[must_use]
    pub fn native_handle(&self) -> Option<isize> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let handle = self.window.window().window_handle();
        match handle.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        }
    }

    /// 界面字体。默认是 Windows 的微软雅黑。
    pub fn set_font_family(&self, family: &str) {
        self.window.global::<Theme>().set_font_family(family.into());
    }

    /// 切换到第几个标签页（0 专注，1 表情，2 音乐，3 设置）。
    pub fn set_tab(&self, tab: i32) {
        self.window.set_tab(tab);
    }

    /// 按当前状态刷新界面。
    pub fn render(&self, state: &AppState, now: WallMs) {
        let next = view::project(state, now);
        // Slint 属性的初始值与 `View::default()` 一致，所以首次渲染也可以直接比较。
        let prev = self.shown.borrow_mut().take().unwrap_or_default();
        let store = self.window.global::<Store>();
        let m = &self.models;

        if prev.header != next.header {
            store.set_header(header(&next));
        }
        if prev.focus != next.focus {
            store.set_focus(focus(&next));
        }
        if prev.todo != next.todo {
            store.set_todo(todo(&next));
        }
        if prev.todo_items != next.todo_items {
            sync(&m.todo_items, next.todo_items.iter().map(todo_row));
        }
        if prev.emotion != next.emotion {
            store.set_emotion(emotion(&next));
        }
        if prev.base_expressions != next.base_expressions {
            sync(&m.base, next.base_expressions.iter().map(chip));
        }
        if prev.accessory_expressions != next.accessory_expressions {
            sync(&m.accessory, next.accessory_expressions.iter().map(chip));
        }
        if prev.event_columns != next.event_columns {
            sync(
                &m.event_columns,
                next.event_columns.iter().map(SharedString::from),
            );
        }
        if prev.event_rows != next.event_rows {
            sync(&m.event_rows, next.event_rows.iter().map(event_row));
        }
        if prev.devices != next.devices {
            sync(&m.devices, next.devices.iter().map(SharedString::from));
        }
        if prev.music != next.music {
            store.set_music(music(&next));
        }
        if prev.characters != next.characters {
            sync(
                &m.characters,
                next.characters.iter().map(SharedString::from),
            );
        }
        if prev.settings != next.settings {
            store.set_settings(settings(&next));
        }
        if prev.dialog != next.dialog {
            store.set_dialog(next.dialog);
        }
        *self.shown.borrow_mut() = Some(next);
    }
}

/// 原地更新列表：只改变化的行，多删少补。
fn sync<T: Clone + PartialEq + 'static>(model: &VecModel<T>, rows: impl Iterator<Item = T>) {
    let mut len = 0;
    for (i, row) in rows.enumerate() {
        len = i + 1;
        if i < model.row_count() {
            if model.row_data(i).as_ref() != Some(&row) {
                model.set_row_data(i, row);
            }
        } else {
            model.push(row);
        }
    }
    while model.row_count() > len {
        model.remove(model.row_count() - 1);
    }
}

fn header(v: &View) -> ui::HeaderView {
    ui::HeaderView {
        character: v.header.character.as_str().into(),
        audio_text: v.header.audio_text.as_str().into(),
        audio_active: v.header.audio_active,
        locked: v.header.locked,
    }
}

fn focus(v: &View) -> ui::FocusView {
    let f = &v.focus;
    ui::FocusView {
        study: f.study,
        countdown: f.countdown,
        mode_locked: f.mode_locked,
        minutes: f.minutes,
        minutes_editable: f.minutes_editable,
        time_text: f.time_text.as_str().into(),
        status_text: f.status_text.as_str().into(),
        phase: f.phase as i32,
        progress: f.progress,
        show_progress: f.show_progress,
        can_reset: f.can_reset,
    }
}

fn todo(v: &View) -> ui::TodoView {
    ui::TodoView {
        name: v.todo.name.as_str().into(),
        can_remove: v.todo.can_remove,
        done_count: v.todo.done_count,
        total: v.todo.total,
    }
}

fn todo_row(r: &view::TodoRow) -> ui::TodoRow {
    ui::TodoRow {
        id: i32::try_from(r.id).unwrap_or(i32::MAX),
        text: r.text.as_str().into(),
        done: r.done,
    }
}

fn emotion(v: &View) -> ui::EmotionView {
    let e = &v.emotion;
    ui::EmotionView {
        current_name: e.current_name.as_str().into(),
        current_random: e.current_random,
        random_enabled: e.random_enabled,
        random_interval: e.random_interval,
        doze_note: e.doze_note,
    }
}

fn chip(c: &view::Chip) -> ui::ExpressionChip {
    ui::ExpressionChip {
        id: c.id.as_str().into(),
        name: c.name.as_str().into(),
        active: c.active,
        enabled: c.enabled,
    }
}

fn event_row(r: &view::EventRow) -> ui::EventRow {
    let cells: Vec<_> = r
        .cells
        .iter()
        .map(|c| ui::EventCell {
            on: c.on,
            editable: c.editable,
        })
        .collect();
    ui::EventRow {
        name: r.name.as_str().into(),
        cells: ModelRc::new(VecModel::from(cells)),
    }
}

fn music(v: &View) -> ui::MusicView {
    let m = &v.music;
    ui::MusicView {
        phase: m.phase as i32,
        status_text: m.status_text.as_str().into(),
        source_text: m.source_text.as_str().into(),
        button_text: m.button_text.as_str().into(),
        show_bpm: m.show_bpm,
        bpm_text: m.bpm_text.as_str().into(),
        beat_locked: m.beat_locked,
        beat_text: m.beat_text.as_str().into(),
        level: m.level,
        bass: m.bass,
        can_shift: m.can_shift,
        silence_hint: m.silence_hint,
        error_text: m.error_text.as_str().into(),
        device_index: m.device_index,
        auto_follow: m.auto_follow,
    }
}

fn settings(v: &View) -> ui::SettingsView {
    let s = &v.settings;
    ui::SettingsView {
        end_sound: s.end_sound,
        pet_scale: s.pet_scale,
        pet_scale_text: s.pet_scale_text.as_str().into(),
        locked: s.locked,
        hide_on_fullscreen: s.hide_on_fullscreen,
        autostart: s.autostart,
        character_index: s.character_index,
        character_status: s.character_status.as_str().into(),
        character_failed: s.character_failed,
        version: s.version.as_str().into(),
    }
}

/// 界面回调 → 命令。
fn wire(store: &Store<'_>, dispatch: &Rc<dyn Fn(Command)>, shown: &Rc<RefCell<Option<View>>>) {
    let send = |f: fn() -> Command| {
        let d = dispatch.clone();
        move || d(f())
    };
    let d = dispatch.clone();
    let lookup = shown.clone();
    // 根据当前显示的内容决定命令（主按钮的含义、下标对应的 id）。
    let with_view = move |pick: &dyn Fn(&View) -> Option<Command>| {
        let cmd = lookup.borrow().as_ref().and_then(pick);
        if let Some(cmd) = cmd {
            d(cmd);
        }
    };
    let with_view = Rc::new(with_view);

    let d = dispatch.clone();
    store.on_select_activity(move |study| {
        let mode = if study {
            ActivityMode::Study
        } else {
            ActivityMode::Chill
        };
        d(Command::Study(StudyCommand::SelectActivity(mode)));
    });
    let d = dispatch.clone();
    store.on_set_countdown(move |countdown| {
        let mode = if countdown {
            TimerMode::Countdown
        } else {
            TimerMode::Stopwatch
        };
        d(Command::Study(StudyCommand::SetTimerMode(mode)));
    });
    let d = dispatch.clone();
    store.on_set_minutes(move |m| d(Command::Study(StudyCommand::SetMinutes(i64::from(m)))));
    let w = with_view.clone();
    store.on_timer_primary(move || {
        w(&|v| {
            Some(Command::Study(match v.focus.phase {
                TimerPhase::Idle | TimerPhase::Paused => StudyCommand::Start,
                TimerPhase::Running => StudyCommand::RequestPause,
                TimerPhase::Completed => StudyCommand::Reset,
            }))
        });
    });
    store.on_timer_reset(send(|| Command::Study(StudyCommand::Reset)));
    store.on_dialog_confirm(send(|| Command::Study(StudyCommand::ConfirmDialog)));
    store.on_dialog_cancel(send(|| Command::Study(StudyCommand::CancelDialog)));

    let d = dispatch.clone();
    store.on_todo_rename(move |name| d(Command::Todo(TodoCommand::Rename(name.into()))));
    store.on_todo_add(send(|| Command::Todo(TodoCommand::Add)));
    let d = dispatch.clone();
    store.on_todo_remove(move |id| {
        if let Ok(id) = u32::try_from(id) {
            d(Command::Todo(TodoCommand::Remove(id)));
        }
    });
    let d = dispatch.clone();
    store.on_todo_set_text(move |id, text| {
        if let Ok(id) = u32::try_from(id) {
            d(Command::Todo(TodoCommand::SetText {
                id,
                text: text.into(),
            }));
        }
    });
    let d = dispatch.clone();
    store.on_todo_set_done(move |id, done| {
        if let Ok(id) = u32::try_from(id) {
            d(Command::Todo(TodoCommand::SetDone { id, done }));
        }
    });

    let d = dispatch.clone();
    store.on_select_expression(move |id| {
        d(Command::Expression(ExpressionCommand::Select(id.into())));
    });
    let d = dispatch.clone();
    store.on_set_random_enabled(move |on| {
        d(Command::Settings(SettingsCommand::SetRandomEnabled(on)));
    });
    let d = dispatch.clone();
    store.on_set_random_interval(move |s| {
        d(Command::Settings(SettingsCommand::SetRandomInterval(
            i64::from(s),
        )));
    });
    let d = dispatch.clone();
    store.on_set_event(move |row, column, on| {
        let event = usize::try_from(row)
            .ok()
            .and_then(|r| RandomEvent::ALL.get(r));
        let context = usize::try_from(column)
            .ok()
            .and_then(|c| EventContext::ALL.get(c));
        if let (Some(&event), Some(&context)) = (event, context) {
            d(Command::Expression(ExpressionCommand::SetEventSwitch {
                context,
                event,
                on,
            }));
        }
    });

    let w = with_view.clone();
    store.on_audio_primary(move || {
        w(&|v| {
            Some(Command::Audio(match v.music.phase {
                AudioPhase::Idle => AudioCommand::StartSystem,
                AudioPhase::Starting | AudioPhase::Running => AudioCommand::Stop,
            }))
        });
    });
    store.on_audio_toggle(send(|| Command::Audio(AudioCommand::Toggle)));
    store.on_audio_stop(send(|| Command::Audio(AudioCommand::Stop)));
    let w = with_view.clone();
    store.on_select_device(move |index| {
        w(&|v| {
            let id = usize::try_from(index)
                .ok()
                .and_then(|i| v.device_ids.get(i))?;
            Some(Command::Audio(AudioCommand::SelectDevice(id.clone())))
        });
    });
    store.on_refresh_devices(send(|| Command::Audio(AudioCommand::RefreshDevices)));
    let d = dispatch.clone();
    store.on_shift_tempo(move |double| {
        let shift = if double {
            TempoShift::Double
        } else {
            TempoShift::Half
        };
        d(Command::Audio(AudioCommand::ShiftTempo(shift)));
    });
    store.on_open_sound_settings(send(|| Command::Pet(PetCommand::OpenSoundSettings)));
    let d = dispatch.clone();
    store.on_set_show_bpm(move |on| d(Command::Settings(SettingsCommand::SetShowBpm(on))));
    let d = dispatch.clone();
    store.on_set_auto_follow(move |on| d(Command::Settings(SettingsCommand::SetAutoFollow(on))));

    let d = dispatch.clone();
    store.on_set_end_sound(move |on| d(Command::Settings(SettingsCommand::SetEndSound(on))));
    let d = dispatch.clone();
    store.on_set_pet_scale(move |s| d(Command::Settings(SettingsCommand::SetPetScale(s))));
    let d = dispatch.clone();
    store.on_set_locked(move |on| d(Command::Pet(PetCommand::SetLocked(on))));
    let d = dispatch.clone();
    store.on_set_hide_on_fullscreen(move |on| {
        d(Command::Settings(SettingsCommand::SetHideOnFullscreen(on)));
    });
    let d = dispatch.clone();
    store.on_set_autostart(move |on| d(Command::Settings(SettingsCommand::SetAutostart(on))));
    let w = with_view;
    store.on_select_character(move |index| {
        w(&|v| {
            let id = usize::try_from(index)
                .ok()
                .and_then(|i| v.character_ids.get(i))?;
            Some(Command::Character(CharacterCommand::Select(id.clone())))
        });
    });
    store.on_reload_character(send(|| Command::Character(CharacterCommand::Reload)));
    store.on_hide_console(send(|| Command::Pet(PetCommand::HideConsole)));
    store.on_quit(send(|| Command::Pet(PetCommand::Quit)));
}
