//! Slint 回调到 `yume_core::Command` 的翻译层。

use crate::ui::Store;
use crate::view::{AudioPhase, TimerPhase, View};
use slint::platform::Key;
use std::cell::RefCell;
use std::rc::Rc;
use yume_core::Command;
use yume_core::command::{
    AudioCommand, CharacterCommand, ExpressionCommand, PetCommand, SettingsCommand, StudyCommand,
    TodoCommand,
};
use yume_core::model::audio::TempoShift;
use yume_core::model::events::ActivityMode;
use yume_core::model::shortcuts::{ShortcutAction, format_recorded};
use yume_core::model::theme::{AccentPreset, BackgroundMode, CornerMode};
use yume_core::model::timer::TimerMode;

pub(super) fn wire(
    store: &Store<'_>,
    dispatch: &Rc<dyn Fn(Command)>,
    shown: &Rc<RefCell<Option<View>>>,
) {
    let with_view = make_view_dispatcher(dispatch, shown);
    wire_focus_and_todo(store, dispatch, &with_view);
    wire_emotion_and_audio(store, dispatch, &with_view);
    wire_settings(store, dispatch, &with_view, shown);
    super::editor::wire(store, dispatch);
}

type ViewDispatcher = Rc<dyn Fn(&dyn Fn(&View) -> Option<Command>)>;

fn make_view_dispatcher(
    dispatch: &Rc<dyn Fn(Command)>,
    shown: &Rc<RefCell<Option<View>>>,
) -> ViewDispatcher {
    let d = dispatch.clone();
    let lookup = shown.clone();
    Rc::new(move |pick: &dyn Fn(&View) -> Option<Command>| {
        let cmd = lookup.borrow().as_ref().and_then(pick);
        if let Some(cmd) = cmd {
            d(cmd);
        }
    })
}

fn send(dispatch: &Rc<dyn Fn(Command)>, f: fn() -> Command) -> impl Fn() + 'static {
    let d = dispatch.clone();
    move || d(f())
}

fn wire_focus_and_todo(
    store: &Store<'_>,
    dispatch: &Rc<dyn Fn(Command)>,
    with_view: &ViewDispatcher,
) {
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
    store.on_timer_reset(send(dispatch, || Command::Study(StudyCommand::Reset)));
    store.on_dialog_confirm(send(dispatch, || {
        Command::Study(StudyCommand::ConfirmDialog)
    }));
    store.on_dialog_cancel(send(dispatch, || {
        Command::Study(StudyCommand::CancelDialog)
    }));

    let d = dispatch.clone();
    store.on_todo_rename(move |name| d(Command::Todo(TodoCommand::Rename(name.into()))));
    store.on_todo_add(send(dispatch, || Command::Todo(TodoCommand::Add)));
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
    store.on_todo_pick_import_md(send(dispatch, || Command::Todo(TodoCommand::PickImportMd)));
    store.on_todo_pick_export_md(send(dispatch, || Command::Todo(TodoCommand::PickExportMd)));
    store.on_todo_reload_md(send(dispatch, || Command::Todo(TodoCommand::ReloadMd)));
    store.on_todo_unbind_md(send(dispatch, || Command::Todo(TodoCommand::UnbindMd)));
}

fn wire_emotion_and_audio(
    store: &Store<'_>,
    dispatch: &Rc<dyn Fn(Command)>,
    with_view: &ViewDispatcher,
) {
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

    let w = with_view.clone();
    store.on_audio_primary(move || {
        w(&|v| {
            Some(Command::Audio(match v.music.phase {
                AudioPhase::Idle => AudioCommand::StartSystem,
                AudioPhase::Starting | AudioPhase::Running => AudioCommand::Stop,
            }))
        });
    });
    store.on_audio_toggle(send(dispatch, || Command::Audio(AudioCommand::Toggle)));
    store.on_audio_stop(send(dispatch, || Command::Audio(AudioCommand::Stop)));
    let w = with_view.clone();
    store.on_select_device(move |index| {
        w(&|v| {
            let id = usize::try_from(index)
                .ok()
                .and_then(|i| v.device_ids.get(i))?;
            Some(Command::Audio(AudioCommand::SelectDevice(id.clone())))
        });
    });
    store.on_refresh_devices(send(dispatch, || {
        Command::Audio(AudioCommand::RefreshDevices)
    }));
    let d = dispatch.clone();
    store.on_shift_tempo(move |double| {
        let shift = if double {
            TempoShift::Double
        } else {
            TempoShift::Half
        };
        d(Command::Audio(AudioCommand::ShiftTempo(shift)));
    });
    store.on_open_sound_settings(send(dispatch, || {
        Command::Pet(PetCommand::OpenSoundSettings)
    }));
    let d = dispatch.clone();
    store.on_set_show_bpm(move |on| d(Command::Settings(SettingsCommand::SetShowBpm(on))));
    let d = dispatch.clone();
    store.on_set_auto_follow(move |on| d(Command::Settings(SettingsCommand::SetAutoFollow(on))));
}

fn wire_settings(
    store: &Store<'_>,
    dispatch: &Rc<dyn Fn(Command)>,
    with_view: &ViewDispatcher,
    shown: &Rc<RefCell<Option<View>>>,
) {
    let d = dispatch.clone();
    store.on_set_end_sound(move |on| d(Command::Settings(SettingsCommand::SetEndSound(on))));
    let d = dispatch.clone();
    store.on_set_pet_scale(move |s| d(Command::Settings(SettingsCommand::SetPetScale(s))));
    let d = dispatch.clone();
    store.on_set_pet_opacity(move |o| d(Command::Settings(SettingsCommand::SetPetOpacity(o))));
    let d = dispatch.clone();
    store.on_set_console_opacity(move |o| {
        d(Command::Settings(SettingsCommand::SetConsoleOpacity(o)));
    });
    let d = dispatch.clone();
    store.on_set_background_mode(move |key| {
        if let Some(mode) = BackgroundMode::from_key(&key) {
            d(Command::Settings(SettingsCommand::SetBackgroundMode(mode)));
        }
    });
    let d = dispatch.clone();
    store.on_set_custom_background(move |hex| {
        d(Command::Settings(SettingsCommand::SetCustomBackground(
            Some(hex.into()),
        )));
    });
    let d = dispatch.clone();
    store.on_set_bg_opacity(move |o| d(Command::Settings(SettingsCommand::SetBgOpacity(o))));
    let d = dispatch.clone();
    store.on_set_card_opacity(move |o| d(Command::Settings(SettingsCommand::SetCardOpacity(o))));
    let d = dispatch.clone();
    store.on_set_accent(move |key| {
        if let Some(preset) = AccentPreset::from_key(&key) {
            d(Command::Settings(SettingsCommand::SetAccent(preset)));
        }
    });
    let d = dispatch.clone();
    store.on_set_custom_accent(move |hex| {
        d(Command::Settings(SettingsCommand::SetCustomAccent(Some(
            hex.into(),
        ))));
    });
    store.on_clear_custom_accent(send(dispatch, || {
        Command::Settings(SettingsCommand::SetCustomAccent(None))
    }));
    let d = dispatch.clone();
    store.on_set_corner_mode(move |key| {
        if let Some(mode) = CornerMode::from_key(&key) {
            d(Command::Settings(SettingsCommand::SetCornerMode(mode)));
        }
    });
    let d = dispatch.clone();
    store.on_set_corner_radius(move |px| {
        d(Command::Settings(SettingsCommand::SetCustomCornerRadius(
            i64::from(px),
        )));
    });
    store.on_pick_background_image(send(dispatch, || {
        Command::Settings(SettingsCommand::PickBackgroundImage)
    }));
    store.on_clear_background_image(send(dispatch, || {
        Command::Settings(SettingsCommand::ClearBackgroundImage)
    }));
    let d = dispatch.clone();
    store.on_set_background_dim(move |dim| {
        d(Command::Settings(SettingsCommand::SetBackgroundDim(dim)));
    });
    let d = dispatch.clone();
    store.on_record_shortcut(move |action_id, ctrl, alt, shift, text| {
        let Some(action) = ShortcutAction::from_id(&action_id) else {
            return;
        };
        let key_token = slint_key_token(&text);
        let binding =
            format_recorded(ctrl, alt, shift, &key_token, action.is_global()).unwrap_or(key_token);
        d(Command::Settings(SettingsCommand::SetShortcut {
            action,
            binding,
        }));
    });
    let d = dispatch.clone();
    store.on_clear_shortcut(move |action_id| {
        if let Some(action) = ShortcutAction::from_id(&action_id) {
            d(Command::Settings(SettingsCommand::ClearShortcut(action)));
        }
    });
    store.on_reset_shortcuts(send(dispatch, || {
        Command::Settings(SettingsCommand::ResetShortcuts)
    }));
    let d = dispatch.clone();
    let lookup = shown.clone();
    store.on_console_key_pressed(move |ctrl, alt, shift, text| {
        let key = slint_key_token(&text);
        if key.is_empty() {
            return false;
        }
        let matched = lookup.borrow().as_ref().is_some_and(|v| {
            v.shortcuts_config
                .match_local(ctrl, alt, shift, &key)
                .is_some()
        });
        if matched {
            d(Command::Settings(SettingsCommand::ConsoleKeyPressed {
                ctrl,
                alt,
                shift,
                key,
            }));
        }
        matched
    });
    let w = with_view.clone();
    store.on_select_zh_font(move |index| {
        w(&|v| {
            let family = usize::try_from(index)
                .ok()
                .and_then(|i| v.zh_font_families.get(i))?;
            Some(Command::Settings(SettingsCommand::SetZhFontFamily(
                family.clone(),
            )))
        });
    });
    let w = with_view.clone();
    store.on_select_en_font(move |index| {
        w(&|v| {
            let family = usize::try_from(index)
                .ok()
                .and_then(|i| v.en_font_families.get(i))?;
            Some(Command::Settings(SettingsCommand::SetEnFontFamily(
                family.clone(),
            )))
        });
    });
    let d = dispatch.clone();
    store.on_set_font_size(move |px| {
        d(Command::Settings(SettingsCommand::SetFontSize(i64::from(
            px,
        ))))
    });
    store.on_pick_custom_font(send(dispatch, || {
        Command::Settings(SettingsCommand::PickCustomFont)
    }));
    store.on_reset_font(send(dispatch, || {
        Command::Settings(SettingsCommand::ResetFont)
    }));
    let d = dispatch.clone();
    store.on_set_locked(move |on| d(Command::Pet(PetCommand::SetLocked(on))));
    let d = dispatch.clone();
    store.on_set_hide_on_fullscreen(move |on| {
        d(Command::Settings(SettingsCommand::SetHideOnFullscreen(on)));
    });
    let d = dispatch.clone();
    store.on_set_autostart(move |on| d(Command::Settings(SettingsCommand::SetAutostart(on))));
    let w = with_view.clone();
    store.on_select_character(move |index| {
        w(&|v| {
            let id = usize::try_from(index)
                .ok()
                .and_then(|i| v.character_ids.get(i))?;
            Some(Command::Character(CharacterCommand::Select(id.clone())))
        });
    });
    store.on_reload_character(send(dispatch, || {
        Command::Character(CharacterCommand::Reload)
    }));
    store.on_hide_console(send(dispatch, || Command::Pet(PetCommand::HideConsole)));
    store.on_quit(send(dispatch, || Command::Pet(PetCommand::Quit)));
}

/// 将 Slint 的 `KeyEvent.text`（含 F1–F12 私有码位）翻译成标准按键名。
fn slint_key_token(text: &str) -> String {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return String::new();
    };
    if chars.next().is_some() {
        return text.to_owned();
    }
    let named = match ch {
        c if c == char::from(Key::Space) || c == ' ' => Some("Space"),
        c if c == char::from(Key::F1) => Some("F1"),
        c if c == char::from(Key::F2) => Some("F2"),
        c if c == char::from(Key::F3) => Some("F3"),
        c if c == char::from(Key::F4) => Some("F4"),
        c if c == char::from(Key::F5) => Some("F5"),
        c if c == char::from(Key::F6) => Some("F6"),
        c if c == char::from(Key::F7) => Some("F7"),
        c if c == char::from(Key::F8) => Some("F8"),
        c if c == char::from(Key::F9) => Some("F9"),
        c if c == char::from(Key::F10) => Some("F10"),
        c if c == char::from(Key::F11) => Some("F11"),
        c if c == char::from(Key::F12) => Some("F12"),
        _ => None,
    };
    named.map_or_else(|| ch.to_string(), str::to_owned)
}
