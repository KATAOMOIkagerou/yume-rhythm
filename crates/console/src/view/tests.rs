use super::*;
use yume_core::command::{
    AudioCommand, CharacterCommand, ExpressionCommand, StudyCommand, TodoCommand,
};
use yume_core::model::audio::{AudioDevice, Telemetry};
use yume_core::model::expression::{ExpressionMeta, NEUTRAL};
use yume_core::{Command, Ctx, SplitMix64, update};

struct World {
    state: AppState,
    now: WallMs,
    rng: SplitMix64,
}

impl World {
    fn new() -> Self {
        let mut w = Self {
            state: AppState::default(),
            now: WallMs(5_000_000),
            rng: SplitMix64::new(3),
        };
        let m = |id: &str, name: &str, kind, event| ExpressionMeta {
            id: id.into(),
            name: name.into(),
            kind,
            event,
            duration_ms: 3200,
            random: true,
        };
        w.send(Command::Character(CharacterCommand::Loaded {
            id: "sakura".into(),
            name: "樱音".into(),
            expressions: vec![
                m(NEUTRAL, "自然", ExpressionKind::Base, None),
                m("happy", "开心", ExpressionKind::Base, None),
                m(
                    "think",
                    "思考",
                    ExpressionKind::Accessory,
                    Some(RandomEvent::Think),
                ),
                m(
                    "doze",
                    "打瞌睡",
                    ExpressionKind::Accessory,
                    Some(RandomEvent::Doze),
                ),
            ],
        }));
        w
    }

    fn send(&mut self, cmd: Command) {
        let mut ctx = Ctx::new(self.now, &mut self.rng);
        update(&mut self.state, cmd, &mut ctx);
    }

    fn view(&self) -> View {
        project(&self.state, self.now)
    }

    fn start_music(&mut self) {
        self.send(Command::Audio(AudioCommand::StartSystem));
        let request = self.state.audio().request;
        self.send(Command::Audio(AudioCommand::Started { request }));
    }
}

fn active(chips: &[Chip]) -> Vec<&str> {
    chips
        .iter()
        .filter(|c| c.active)
        .map(|c| c.id.as_str())
        .collect()
}

#[test]
fn fresh_state() {
    let v = World::new().view();
    assert_eq!(v.header.character, "樱音");
    assert_eq!(v.header.audio_text, "未在听歌");
    assert_eq!(v.focus.time_text, "25:00");
    assert_eq!(v.focus.phase, TimerPhase::Idle);
    assert!(v.focus.countdown && v.focus.minutes_editable && !v.focus.show_progress);
    assert_eq!(v.todo_items.len(), 3);
    assert_eq!(v.dialog, 0);
    assert_eq!(v.devices, ["系统默认输出设备"]);
    assert_eq!(v.device_ids, [None]);
    assert_eq!(v.music.device_index, 0);
    assert_eq!(v.emotion.current_name, "自然");
}

#[test]
fn running_countdown_rounds_up_and_locks_mode() {
    let mut w = World::new();
    w.send(Command::Study(StudyCommand::Start));
    w.now = w.now.plus(1_500);
    let v = w.view();
    assert_eq!(v.focus.time_text, "24:59");
    assert_eq!(v.focus.phase, TimerPhase::Running);
    assert!(v.focus.study && v.focus.mode_locked && !v.focus.minutes_editable);
    assert!(v.focus.show_progress && v.focus.can_reset);
    assert!(v.focus.progress > 0.0 && v.focus.progress < 0.01);

    w.send(Command::Study(StudyCommand::RequestPause));
    assert_eq!(w.view().dialog, 1);
}

#[test]
fn clock_text_formats() {
    assert_eq!(clock_text(0, true), "00:00");
    assert_eq!(clock_text(59_001, true), "01:00");
    assert_eq!(clock_text(59_999, false), "00:59");
    assert_eq!(clock_text(3_600_000, false), "1:00:00");
    assert_eq!(clock_text(-5, true), "00:00");
}

#[test]
fn accessory_keeps_base_highlighted() {
    let mut w = World::new();
    w.send(Command::Expression(ExpressionCommand::Select(
        "happy".into(),
    )));
    w.send(Command::Expression(ExpressionCommand::Select(
        "think".into(),
    )));
    let v = w.view();
    assert_eq!(active(&v.base_expressions), ["happy"]);
    assert_eq!(active(&v.accessory_expressions), ["think"]);
    assert_eq!(v.emotion.current_name, "思考");
    assert!(!v.emotion.current_random);
}

#[test]
fn music_disables_doze_and_shows_bpm() {
    let mut w = World::new();
    w.start_music();
    w.send(Command::Audio(AudioCommand::Telemetry(Telemetry {
        bpm: 127.6,
        level: 0.3,
        bass: 0.5,
        beats: 12,
        beat_locked: true,
    })));
    let v = w.view();
    assert_eq!(v.header.audio_text, "128 BPM");
    assert!(v.header.audio_active);
    assert_eq!(v.music.bpm_text, "128");
    assert_eq!(v.music.beat_text, "已跟拍 12 次");
    assert!(v.music.can_shift && v.music.beat_locked);
    assert_eq!(v.music.button_text, "停止");
    assert!(v.emotion.doze_note);
    let doze = v.accessory_expressions.iter().find(|c| c.id == "doze");
    assert_eq!(doze.map(|c| c.enabled), Some(false));
    assert!(v.music.source_text.starts_with("电脑声音"));
}

#[test]
fn device_index_follows_setting() {
    let mut w = World::new();
    let dev = |id: &str| AudioDevice {
        id: id.into(),
        name: format!("扬声器 {id}"),
    };
    w.send(Command::Audio(AudioCommand::DevicesListed(vec![
        dev("a"),
        dev("b"),
    ])));
    w.send(Command::Audio(AudioCommand::SelectDevice(Some("b".into()))));
    let v = w.view();
    assert_eq!(v.devices.len(), 3);
    assert_eq!(v.music.device_index, 2);
    assert_eq!(v.device_ids[2].as_deref(), Some("b"));
}

#[test]
fn event_matrix_locks_music_doze() {
    let v = World::new().view();
    assert_eq!(v.event_columns, ["休闲", "自习", "听歌"]);
    let doze = &v.event_rows[0];
    assert_eq!(doze.name, "打瞌睡");
    assert!(doze.cells[0].on && doze.cells[0].editable);
    assert!(!doze.cells[2].on && !doze.cells[2].editable);
}

#[test]
fn todo_counts() {
    let mut w = World::new();
    let id = w.state.todo().items()[0].id;
    w.send(Command::Todo(TodoCommand::SetDone { id, done: true }));
    let v = w.view();
    assert_eq!((v.todo.done_count, v.todo.total), (1, 3));
    assert!(v.todo.can_remove);
}
