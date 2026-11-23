//! 界面截图：用 Slint 软件渲染器把各页面画成 PNG，写到 `target/ui-screens/`。
//!
//! 不比较像素（字体随系统变化），只保证每个页面都能实例化并渲染出内容，
//! 截图供人工检查。改界面后跑 `cargo test -p yume-console --test screens` 看效果。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "测试辅助函数，出错即测试失败"
)]

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, PhysicalSize};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use yume_console::Console;
use yume_core::command::{
    AudioCommand, CharacterCommand, EditorCommand, ExpressionCommand, SettingsCommand,
    StudyCommand, TodoCommand,
};
use yume_core::model::audio::{AudioDevice, AudioError, Telemetry};
use yume_core::model::events::RandomEvent;
use yume_core::model::expression::{ExpressionKind, ExpressionMeta};
use yume_core::model::pet::CharacterInfo;
use yume_core::model::theme::{AccentPreset, BackgroundMode};
use yume_core::{AppState, Command, Ctx, SplitMix64, WallMs, update};

const SCALE: f32 = 1.5;
const SIZE: (u32, u32) = (400, 680);

thread_local! {
    static LAST: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
}

struct Headless;

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        LAST.with(|l| *l.borrow_mut() = Some(window.clone()));
        Ok(window)
    }
}

struct World {
    state: AppState,
    now: WallMs,
    rng: SplitMix64,
}

impl World {
    fn new() -> Self {
        let mut w = Self {
            state: AppState::default(),
            now: WallMs(10_000_000),
            rng: SplitMix64::new(5),
        };
        let m = |id: &str, name: &str, kind, event| ExpressionMeta {
            id: id.into(),
            name: name.into(),
            kind,
            event,
            duration_ms: 3200,
            random: id != "neutral",
            ..Default::default()
        };
        use ExpressionKind::{Accessory, Base};
        w.send(Command::Character(CharacterCommand::Scanned(vec![
            CharacterInfo {
                id: "sakura".into(),
                name: "樱音".into(),
            },
        ])));
        w.send(Command::Character(CharacterCommand::Loaded {
            id: "sakura".into(),
            name: "樱音".into(),
            expressions: vec![
                m("neutral", "自然", Base, None),
                m("happy", "开心", Base, None),
                m("sad", "难过", Base, None),
                m("think", "思考", Accessory, Some(RandomEvent::Think)),
                m("doze", "打瞌睡", Accessory, Some(RandomEvent::Doze)),
                m("shock", "震惊", Accessory, Some(RandomEvent::Shock)),
                m("love", "喜欢", Accessory, Some(RandomEvent::Love)),
            ],
            canvas: (800, 800),
            custom_warning: None,
        }));
        w
    }

    fn send(&mut self, cmd: Command) {
        let mut ctx = Ctx::new(self.now, &mut self.rng);
        update(&mut self.state, cmd, &mut ctx);
    }

    fn todo_text(&mut self, index: usize, text: &str, done: bool) {
        let id = self.state.todo().items()[index].id;
        self.send(Command::Todo(TodoCommand::SetText {
            id,
            text: text.into(),
        }));
        self.send(Command::Todo(TodoCommand::SetDone { id, done }));
    }
}

fn shoot(name: &str, world: &World, tab: i32) {
    let console = Console::new(Rc::new(|_| {})).expect("创建控制台");
    console.set_font_family("Noto Sans CJK SC");
    console.set_tab(tab);
    console.render(&world.state, world.now);
    let window = LAST.with(|l| l.borrow().clone()).expect("窗口");
    window.dispatch_event(WindowEvent::ScaleFactorChanged {
        scale_factor: SCALE,
    });
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "截图尺寸"
    )]
    let (w, h) = (
        (SIZE.0 as f32 * SCALE) as u32,
        (SIZE.1 as f32 * SCALE) as u32,
    );
    window.set_size(PhysicalSize::new(w, h));
    console.window().show().expect("显示");
    slint::platform::update_timers_and_animations();

    let mut buffer = vec![PremultipliedRgbaColor::default(); (w * h) as usize];
    let drawn = window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, w as usize);
    });
    assert!(drawn, "{name} 没有绘制");
    let rgba: Vec<u8> = buffer
        .iter()
        .flat_map(|p| [p.red, p.green, p.blue, p.alpha])
        .collect();
    assert!(rgba.chunks(4).any(|p| p[0] < 128), "{name} 看起来是空白的");

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-screens");
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
    console.window().hide().unwrap();
}

#[test]
fn render_every_page() {
    slint::platform::set_platform(Box::new(Headless)).expect("设置平台");

    let mut w = World::new();
    shoot("focus_idle", &w, 0);

    w.todo_text(0, "背 30 个单词", true);
    w.todo_text(1, "数学练习册第 3 章", false);
    w.send(Command::Todo(TodoCommand::Rename("今天的计划".into())));
    w.send(Command::Study(StudyCommand::Start));
    w.now = w.now.plus(9 * 60_000 + 12_000);
    w.send(Command::Tick);
    shoot("focus_running", &w, 0);

    w.send(Command::Study(StudyCommand::RequestPause));
    shoot("dialog_pause", &w, 0);
    w.send(Command::Study(StudyCommand::ConfirmDialog));
    shoot("focus_paused", &w, 0);

    w.send(Command::Expression(ExpressionCommand::Select(
        "happy".into(),
    )));
    w.send(Command::Expression(ExpressionCommand::Select(
        "think".into(),
    )));
    shoot("emotion", &w, 1);

    shoot("music_idle", &w, 2);
    let dev = |id: &str, name: &str| AudioDevice {
        id: id.into(),
        name: name.into(),
    };
    w.send(Command::Audio(AudioCommand::DevicesListed(vec![
        dev("wasapi:1", "扬声器 (Realtek High Definition Audio)"),
        dev("wasapi:2", "耳机 (WH-1000XM4)"),
    ])));
    w.send(Command::Audio(AudioCommand::StartSystem));
    let request = w.state.audio().request;
    w.send(Command::Audio(AudioCommand::Started { request }));
    w.send(Command::Audio(AudioCommand::Telemetry(Telemetry {
        bpm: 128.2,
        level: 0.46,
        bass: 0.71,
        beats: 86,
        beat_locked: true,
    })));
    shoot("music_running", &w, 2);
    shoot("emotion_music", &w, 1);

    w.send(Command::Audio(AudioCommand::Stop));
    w.send(Command::Audio(AudioCommand::StartSystem));
    let request = w.state.audio().request;
    w.send(Command::Audio(AudioCommand::Failed {
        request,
        error: AudioError::DeviceUnavailable,
    }));
    shoot("music_error", &w, 2);

    shoot("settings", &w, 3);

    w.send(Command::Settings(SettingsCommand::SetBackgroundMode(
        BackgroundMode::Dark,
    )));
    w.send(Command::Settings(SettingsCommand::SetAccent(
        AccentPreset::Sky,
    )));
    shoot("settings_dark", &w, 3);

    w.send(Command::Settings(SettingsCommand::SetBackgroundMode(
        BackgroundMode::Light,
    )));
    w.send(Command::Editor(EditorCommand::Open));
    shoot("editor_list", &w, 1);

    w.send(Command::Editor(EditorCommand::NewDraft));
    w.send(Command::Editor(EditorCommand::SetName("小星星".into())));
    shoot("editor_new", &w, 1);
}
