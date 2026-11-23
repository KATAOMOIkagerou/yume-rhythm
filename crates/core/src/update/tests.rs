//! 场景测试：经由 `update` 驱动，对照功能清单第 4–10 节。

use crate::clock::{Ctx, SplitMix64, WallMs};
use crate::command::{
    AudioCommand, CharacterCommand, Command, ExpressionCommand, PetCommand, SettingsCommand,
    StudyCommand, TodoCommand,
};
use crate::derive::{console_opacity, pet_scene, pet_window};
use crate::effect::Effect;
use crate::model::audio::{AudioError, AudioSource, AudioStatus};
use crate::model::events::{ActivityMode, RandomEvent};
use crate::model::expression::{ExpressionKind, ExpressionMeta, NEUTRAL};
use crate::model::pet::{CharacterStatus, Dialog};
use crate::model::settings::opacity_to_u8;
use crate::model::shortcuts::{ShortcutAction, ShortcutError};
use crate::model::theme::{AccentPreset, BackgroundMode};
use crate::model::timer::{TimerMode, TimerStatus};
use crate::model::todo_md::{TodoMdError, render_md};
use crate::persist::Persisted;
use crate::state::AppState;

struct World {
    state: AppState,
    now: WallMs,
    rng: SplitMix64,
}

impl World {
    fn new() -> Self {
        let mut w = Self {
            state: AppState::default(),
            now: WallMs(1_000_000),
            rng: SplitMix64::new(9),
        };
        w.send(Command::Startup);
        w.send(Command::Character(CharacterCommand::Loaded {
            id: "sakura".into(),
            name: "樱音".into(),
            expressions: catalog(),
            canvas: (800, 800),
            custom_warning: None,
        }));
        w
    }

    fn send(&mut self, cmd: Command) -> Vec<Effect> {
        let mut ctx = Ctx::new(self.now, &mut self.rng);
        super::update(&mut self.state, cmd, &mut ctx)
    }

    fn study(&mut self, cmd: StudyCommand) -> Vec<Effect> {
        self.send(Command::Study(cmd))
    }

    fn audio(&mut self, cmd: AudioCommand) -> Vec<Effect> {
        self.send(Command::Audio(cmd))
    }

    fn advance(&mut self, ms: i64) -> Vec<Effect> {
        self.now = self.now.plus(ms);
        self.send(Command::Tick)
    }

    fn start_music(&mut self) {
        self.audio(AudioCommand::StartSystem);
        let request = self.state.audio().request;
        self.audio(AudioCommand::Started { request });
    }
}

fn catalog() -> Vec<ExpressionMeta> {
    let m = |id: &str, kind, event| ExpressionMeta {
        id: id.into(),
        name: id.into(),
        kind,
        event,
        duration_ms: 3200,
        random: false,
        ..Default::default()
    };
    vec![
        m(NEUTRAL, ExpressionKind::Base, None),
        m("happy", ExpressionKind::Base, None),
        m("doze", ExpressionKind::Accessory, Some(RandomEvent::Doze)),
    ]
}

#[test]
fn starting_timer_switches_to_study() {
    let mut w = World::new();
    assert_eq!(w.state.activity(), ActivityMode::Chill);
    w.study(StudyCommand::Start);
    assert_eq!(w.state.activity(), ActivityMode::Study);
    assert!(w.state.timer().is_running());
}

#[test]
fn leaving_study_while_timing_needs_confirmation() {
    let mut w = World::new();
    w.study(StudyCommand::Start);
    w.study(StudyCommand::SelectActivity(ActivityMode::Chill));
    assert_eq!(w.state.dialog(), Dialog::ConfirmLeaveStudy);
    assert_eq!(w.state.activity(), ActivityMode::Study);

    w.study(StudyCommand::CancelDialog);
    assert_eq!(w.state.dialog(), Dialog::Encourage);
    assert!(w.state.timer().is_running());
    w.study(StudyCommand::CancelDialog);
    assert_eq!(w.state.dialog(), Dialog::None);

    w.study(StudyCommand::SelectActivity(ActivityMode::Chill));
    w.study(StudyCommand::ConfirmDialog);
    assert_eq!(w.state.activity(), ActivityMode::Chill);
    assert_eq!(w.state.timer().status(), TimerStatus::Paused);
}

#[test]
fn countdown_pause_asks_but_stopwatch_does_not() {
    let mut w = World::new();
    w.study(StudyCommand::Start);
    w.study(StudyCommand::RequestPause);
    assert_eq!(w.state.dialog(), Dialog::ConfirmPause);
    w.study(StudyCommand::ConfirmDialog);
    assert_eq!(w.state.timer().status(), TimerStatus::Paused);

    w.study(StudyCommand::Reset);
    w.study(StudyCommand::SetTimerMode(TimerMode::Stopwatch));
    w.study(StudyCommand::Start);
    w.study(StudyCommand::RequestPause);
    assert_eq!(w.state.dialog(), Dialog::None);
    assert_eq!(w.state.timer().status(), TimerStatus::Paused);
}

#[test]
fn completion_chimes_only_when_enabled_and_closes_dialog() {
    let mut w = World::new();
    w.study(StudyCommand::SetMinutes(1));
    w.study(StudyCommand::Start);
    w.study(StudyCommand::RequestPause);
    let fx = w.advance(60_000);
    assert_eq!(fx, vec![Effect::PlayChime]);
    assert_eq!(w.state.dialog(), Dialog::None);
    assert_eq!(w.state.timer().status(), TimerStatus::Completed);

    w.send(Command::Settings(SettingsCommand::SetEndSound(false)));
    w.study(StudyCommand::Reset);
    w.study(StudyCommand::Start);
    assert!(w.advance(60_000).is_empty());
}

#[test]
fn stale_audio_reports_are_ignored() {
    let mut w = World::new();
    let fx = w.audio(AudioCommand::StartSystem);
    let first = w.state.audio().request;
    assert!(matches!(
        fx.as_slice(),
        [Effect::StartAudio {
            source: AudioSource::System,
            ..
        }]
    ));
    w.audio(AudioCommand::Stop);
    w.audio(AudioCommand::Started { request: first });
    assert_eq!(w.state.audio().status, AudioStatus::Idle);
}

#[test]
fn space_replays_last_source() {
    let mut w = World::new();
    w.audio(AudioCommand::FilesDropped(vec![
        "x.txt".into(),
        "song.flac".into(),
    ]));
    let request = w.state.audio().request;
    w.audio(AudioCommand::Started { request });
    assert_eq!(w.audio(AudioCommand::Toggle), vec![Effect::StopAudio]);
    let fx = w.audio(AudioCommand::Toggle);
    let file = AudioSource::File {
        path: "song.flac".into(),
    };
    assert!(matches!(fx.as_slice(), [Effect::StartAudio { source, .. }] if *source == file));
}

#[test]
fn dropping_non_audio_reports_error() {
    let mut w = World::new();
    assert!(
        w.audio(AudioCommand::FilesDropped(vec!["a.png".into()]))
            .is_empty()
    );
    assert_eq!(w.state.audio().error, Some(AudioError::UnsupportedFile));
}

#[test]
fn music_hides_manual_doze_and_stops_droop() {
    let mut w = World::new();
    w.send(Command::Expression(ExpressionCommand::Select(
        "doze".into(),
    )));
    let scene = pet_scene(&w.state);
    assert!(scene.dozing);
    assert_eq!(scene.accessory_expression.as_deref(), Some("doze"));

    w.start_music();
    let scene = pet_scene(&w.state);
    assert!(scene.music && scene.follow_beat);
    assert!(!scene.dozing);
    assert_eq!(scene.accessory_expression, None);
}

#[test]
fn only_real_changes_alter_persisted_state() {
    let mut w = World::new();
    let before = w.state.persisted();
    w.advance(10_000);
    assert_eq!(w.state.persisted(), before);
    w.send(Command::Todo(TodoCommand::Rename("期末".into())));
    assert_ne!(w.state.persisted(), before);
}

#[test]
fn failed_character_load_is_reported_for_current_only() {
    let mut w = World::new();
    w.send(Command::Character(CharacterCommand::LoadFailed {
        id: "other".into(),
        message: "x".into(),
    }));
    assert_eq!(w.state.character().status, CharacterStatus::Ready);
    w.send(Command::Character(CharacterCommand::Select(
        "../evil".into(),
    )));
    assert_eq!(w.state.settings().character, "sakura");
}

#[test]
fn tray_toggles_autostart() {
    let mut w = World::new();
    assert!(!w.state.settings().autostart);
    w.send(Command::Settings(SettingsCommand::ToggleAutostart));
    assert!(w.state.settings().autostart);
    w.send(Command::Settings(SettingsCommand::ToggleAutostart));
    assert!(!w.state.settings().autostart);
}

#[test]
fn showing_console_always_brings_it_forward() {
    let mut w = World::new();
    let fx = w.send(Command::Pet(PetCommand::ShowConsole));
    assert!(w.state.pet().console_visible);
    assert_eq!(fx, [Effect::FocusConsole]);
    // 已经显示时再要求显示，仍要带到最前面。
    assert_eq!(
        w.send(Command::Pet(PetCommand::ShowConsole)),
        [Effect::FocusConsole]
    );
    assert!(w.send(Command::Pet(PetCommand::ToggleConsole)).is_empty());
    assert!(!w.state.pet().console_visible);
    assert_eq!(
        w.send(Command::Pet(PetCommand::ToggleConsole)),
        [Effect::FocusConsole]
    );
}

#[test]
fn audio_is_active_while_starting_and_running() {
    let mut w = World::new();
    assert!(!w.state.audio().is_active());
    w.audio(AudioCommand::StartSystem);
    assert!(w.state.audio().is_active() && w.state.audio().is_busy());
    let request = w.state.audio().request;
    w.audio(AudioCommand::Started { request });
    assert!(w.state.audio().is_active() && !w.state.audio().is_busy());
    w.audio(AudioCommand::Stop);
    assert!(!w.state.audio().is_active());
}

#[test]
fn theme_and_opacity_settings_validate_and_clamp() {
    let mut w = World::new();
    w.send(Command::Settings(SettingsCommand::SetBackgroundMode(
        BackgroundMode::Dark,
    )));
    assert_eq!(w.state.settings().background, BackgroundMode::Dark);

    w.send(Command::Settings(SettingsCommand::SetCustomBackground(
        Some("zzzzzz".into()),
    )));
    assert!(w.state.bg_error());
    assert_eq!(w.state.settings().background, BackgroundMode::Dark);

    w.send(Command::Settings(SettingsCommand::SetCustomBackground(
        Some("#123456".into()),
    )));
    assert!(!w.state.bg_error());
    assert_eq!(w.state.settings().background, BackgroundMode::Custom);
    assert_eq!(
        w.state.settings().custom_background.as_deref(),
        Some("#123456")
    );

    w.send(Command::Settings(SettingsCommand::SetCustomAccent(Some(
        "zzzzzz".into(),
    ))));
    assert!(w.state.accent_error());
    assert_eq!(w.state.settings().custom_accent, None);

    w.send(Command::Settings(SettingsCommand::SetCustomAccent(Some(
        "#00AAFF".into(),
    ))));
    assert!(!w.state.accent_error());
    assert_eq!(w.state.settings().custom_accent.as_deref(), Some("#00aaff"));

    w.send(Command::Settings(SettingsCommand::SetAccent(
        AccentPreset::Mint,
    )));
    assert_eq!(w.state.settings().accent, AccentPreset::Mint);
    assert_eq!(w.state.settings().custom_accent, None);

    let fx = w.send(Command::Settings(SettingsCommand::BackgroundImageLoaded {
        path: "C:/bg.png".into(),
    }));
    assert_eq!(
        fx,
        vec![Effect::ApplyBackgroundImage {
            path: Some("C:/bg.png".into()),
        }]
    );
    w.send(Command::Settings(SettingsCommand::SetBackgroundDim(0.05)));
    assert!((w.state.settings().background_dim - 0.2).abs() < f32::EPSILON);

    w.send(Command::Settings(SettingsCommand::SetPetOpacity(0.1)));
    w.send(Command::Settings(SettingsCommand::SetConsoleOpacity(0.2)));
    assert!((w.state.settings().pet_opacity - 0.3).abs() < f32::EPSILON);
    assert!((w.state.settings().console_opacity - 0.4).abs() < f32::EPSILON);
    assert_eq!(opacity_to_u8(pet_window(&w.state).opacity), 77);
    assert_eq!(opacity_to_u8(console_opacity(&w.state)), 102);
}

#[test]
fn shortcuts_validate_duplicates_and_dispatch_local_keys() {
    let mut w = World::new();
    w.send(Command::Settings(SettingsCommand::SetShortcut {
        action: ShortcutAction::GlobalToggleAudio,
        binding: "Ctrl+Alt+C".into(),
    }));
    assert_eq!(
        w.state.shortcut_error(),
        Some(&ShortcutError::Duplicate(
            ShortcutAction::GlobalToggleConsole
        ))
    );

    w.send(Command::Settings(SettingsCommand::SetShortcut {
        action: ShortcutAction::LocalHideConsole,
        binding: "F8".into(),
    }));
    assert_eq!(w.state.shortcut_error(), None);
    assert_eq!(w.state.settings().shortcuts.local_hide_console, "F8");

    w.send(Command::Pet(PetCommand::ShowConsole));
    assert!(w.state.pet().console_visible);
    w.send(Command::Settings(SettingsCommand::ConsoleKeyPressed {
        ctrl: false,
        alt: false,
        shift: false,
        key: "F8".into(),
    }));
    assert!(!w.state.pet().console_visible);

    w.send(Command::Settings(SettingsCommand::ResetShortcuts));
    assert_eq!(w.state.settings().shortcuts.local_hide_console, "H");
}

#[test]
fn external_md_todo_loads_tracks_dirty_and_unbinds() {
    let mut w = World::new();
    let fx = w.send(Command::Todo(TodoCommand::MdLoaded {
        path: "D:/study.md".into(),
        content: "# 数学复习\n保留说明\n- [x] 微积分\n- [ ] 线代\n".into(),
    }));
    assert!(fx.is_empty());
    assert_eq!(
        w.state.settings().todo_md_path.as_deref(),
        Some("D:/study.md")
    );
    assert_eq!(w.state.todo().name(), "数学复习");
    assert_eq!(w.state.todo().items().len(), 2);
    let seq_before = w.state.todo_md_dirty_seq();

    let id = w.state.todo().items()[1].id;
    w.send(Command::Todo(TodoCommand::SetDone { id, done: true }));
    assert!(w.state.todo_md_dirty_seq() > seq_before);
    let rendered = render_md(w.state.todo_md_doc(), w.state.todo());
    assert!(rendered.contains("保留说明"));
    assert!(rendered.contains("- [x] 线代"));

    w.send(Command::Todo(TodoCommand::MdFailed(
        TodoMdError::WriteFailed,
    )));
    assert_eq!(w.state.todo_md_error(), Some(TodoMdError::WriteFailed));

    w.send(Command::Todo(TodoCommand::UnbindMd));
    assert_eq!(w.state.settings().todo_md_path, None);
    assert_eq!(w.state.todo_md_error(), None);
}

#[test]
fn startup_requests_fonts_custom_font_and_bound_md() {
    let mut persisted = Persisted::default();
    persisted.settings.custom_font_path = Some("C:/fonts/custom.ttf".into());
    persisted.settings.todo_md_path = Some("D:/tasks.md".into());
    persisted.settings.background_image = Some("C:/bg.jpg".into());
    let mut state = AppState::new(persisted);
    let mut rng = SplitMix64::new(1);
    let mut ctx = Ctx::new(WallMs(1000), &mut rng);
    let fx = super::update(&mut state, Command::Startup, &mut ctx);
    assert!(fx.contains(&Effect::ListFonts));
    assert!(fx.contains(&Effect::LoadCustomFont {
        path: "C:/fonts/custom.ttf".into(),
    }));
    assert!(fx.contains(&Effect::LoadTodoMd {
        path: "D:/tasks.md".into(),
    }));
    assert!(fx.contains(&Effect::ApplyBackgroundImage {
        path: Some("C:/bg.jpg".into()),
    }));
}
