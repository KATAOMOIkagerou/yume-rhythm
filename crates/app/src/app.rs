//! 主线程的组装：持有 `AppState`，执行 `update` 与 `Effect`，
//! 再把派生值的变化应用到桌宠、节拍线程、托盘、控制台和存档（docs/架构.md 第 3 节）。
//!
//! 这里不做业务判断：规则都在 core，这里只负责"把结果送到该去的地方"。

use crate::beat::BeatControl;
use crate::characters;
use crate::clock;
use crate::pet::PetControl;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;
use yume_audio::{AudioService, Source};
use yume_character::{CharacterRenderer, LayeredCharacter};
use yume_console::Console;
use yume_core::command::CharacterCommand;
use yume_core::derive::{self, PetScene, PetWindow};
use yume_core::model::audio::{AudioSource, TempoShift};
use yume_core::persist::Persisted;
use yume_core::{AppState, Command, Ctx, Effect, SplitMix64, WallMs, update};
use yume_win::Tray;

const SAVE_DELAY: Duration = Duration::from_millis(500);
const AUTOSTART_NAME: &str = "YumeRhythm";

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static QUEUE: RefCell<VecDeque<Command>> = const { RefCell::new(VecDeque::new()) };
}

/// 在主线程上处理命令。可重入：处理过程中产生的命令先排队，由最外层依次处理。
pub fn dispatch(cmd: Command) {
    QUEUE.with(|q| q.borrow_mut().push_back(cmd));
    with_app(|app| {
        while let Some(cmd) = QUEUE.with(|q| q.borrow_mut().pop_front()) {
            app.handle(cmd);
        }
    });
}

/// 借用 `App` 执行一段操作。正在处理命令（重入）时不执行。
pub fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut()
            && let Some(app) = guard.as_mut()
        {
            f(app);
        }
    });
}

pub fn install(app: App) {
    APP.with(|cell| *cell.borrow_mut() = Some(app));
}

pub fn uninstall() -> Option<App> {
    APP.with(|cell| cell.borrow_mut().take())
}

/// 其他线程与主线程之间的连接。
pub struct Links {
    pub pet: Sender<PetControl>,
    pub beat: Sender<BeatControl>,
    pub audio: AudioService,
    pub tray: Option<Tray>,
}

pub struct App {
    state: AppState,
    rng: SplitMix64,
    console: Console,
    links: Links,
    roots: Vec<PathBuf>,
    state_file: Option<PathBuf>,
    saved: Persisted,
    save_timer: slint::Timer,
    last: Applied,
    quitting: bool,
    focus_console: bool,
}

/// 上次应用出去的派生值，用来判断是否变化。
#[derive(Default)]
struct Applied {
    scene: Option<PetScene>,
    window: Option<PetWindow>,
    music: Option<bool>,
    locked: Option<bool>,
    autostart: Option<bool>,
    console: bool,
}

/// 后台加载好的角色。
struct LoadedCharacter {
    name: String,
    expressions: Vec<yume_core::model::expression::ExpressionMeta>,
    renderer: Box<dyn CharacterRenderer>,
}

impl App {
    #[must_use]
    pub fn new(
        state: AppState,
        console: Console,
        links: Links,
        roots: Vec<PathBuf>,
        state_file: Option<PathBuf>,
        first_run: bool,
    ) -> Self {
        let mut saved = state.persisted();
        if first_run {
            // 首次运行：让存档"过期"，下一次同步就会写出一份，下次启动不再当作首次运行。
            saved.version = 0;
        }
        Self {
            state,
            rng: SplitMix64::new(clock::seed()),
            console,
            links,
            roots,
            state_file,
            saved,
            save_timer: slint::Timer::default(),
            last: Applied::default(),
            quitting: false,
            focus_console: false,
        }
    }

    fn handle(&mut self, cmd: Command) {
        if self.quitting {
            return;
        }
        let now = clock::wall_now();
        let effects = {
            let mut ctx = Ctx::new(now, &mut self.rng);
            update(&mut self.state, cmd, &mut ctx)
        };
        for effect in effects {
            self.run(effect);
        }
        self.sync(now);
        if std::mem::take(&mut self.focus_console)
            && let Some(hwnd) = self.console.native_handle()
        {
            yume_win::bring_to_front(hwnd);
        }
        if self.quitting {
            self.shutdown();
        }
    }

    fn run(&mut self, effect: Effect) {
        let links = &self.links;
        match effect {
            Effect::PlayChime => links.audio.play_chime(),
            Effect::StartAudio {
                request,
                source,
                device,
            } => {
                let source = match source {
                    AudioSource::System => Source::Loopback { device },
                    AudioSource::File { path } => Source::File { path: path.into() },
                };
                links.audio.start(request, source);
            }
            Effect::StopAudio => links.audio.stop(),
            Effect::ShiftTempo(shift) => {
                let shift = match shift {
                    TempoShift::Double => yume_beat::TempoShift::Double,
                    TempoShift::Half => yume_beat::TempoShift::Half,
                };
                let _ = links.beat.send(BeatControl::Shift(shift));
            }
            Effect::ListAudioDevices => links.audio.list_devices(),
            Effect::ScanCharacters => {
                let list = characters::scan(&self.roots);
                dispatch(Command::Character(CharacterCommand::Scanned(list)));
            }
            Effect::LoadCharacter { id } => self.load_character(id),
            Effect::OpenSoundSettings => yume_win::open_sound_settings(),
            // 要等这次同步把窗口显示出来之后才能带到前面，见 `handle`。
            Effect::FocusConsole => self.focus_console = true,
            Effect::Quit => self.quitting = true,
        }
    }

    /// 在后台线程读图、建渲染器，完成后回到主线程交给桌宠。
    fn load_character(&mut self, id: String) {
        let roots = self.roots.clone();
        let seed = clock::seed();
        let spawned = std::thread::Builder::new()
            .name("load-character".into())
            .spawn(move || {
                let result = characters::load(&roots, &id).map(|pack| LoadedCharacter {
                    name: pack.manifest.name.clone(),
                    expressions: pack.expression_meta(),
                    renderer: Box::new(LayeredCharacter::new(pack, seed)),
                });
                let posted = slint::invoke_from_event_loop(move || {
                    let mut report = None;
                    with_app(|app| report = Some(app.accept_character(id, result)));
                    if let Some(cmd) = report {
                        dispatch(cmd);
                    }
                });
                if posted.is_err() {
                    log::debug!("事件循环已结束，丢弃角色加载结果");
                }
            });
        if let Err(e) = spawned {
            log::error!("无法启动角色加载线程：{e}");
        }
    }

    /// 只有仍是当前角色时才交给桌宠，避免快速切换时旧结果覆盖新角色。
    fn accept_character(&self, id: String, result: Result<LoadedCharacter, String>) -> Command {
        match result {
            Ok(loaded) => {
                if self.state.settings().character == id {
                    let _ = self.links.pet.send(PetControl::Character(loaded.renderer));
                }
                log::info!("角色已加载：{id}");
                Command::Character(CharacterCommand::Loaded {
                    id,
                    name: loaded.name,
                    expressions: loaded.expressions,
                })
            }
            Err(message) => {
                log::warn!("角色 {id} 加载失败：{message}");
                Command::Character(CharacterCommand::LoadFailed { id, message })
            }
        }
    }

    /// 把派生值的变化应用出去。
    fn sync(&mut self, now: WallMs) {
        let links = &self.links;
        let last = &mut self.last;

        let scene = derive::pet_scene(&self.state);
        if last.scene.as_ref() != Some(&scene) {
            let _ = links.pet.send(PetControl::Scene(scene.clone()));
            last.scene = Some(scene);
        }
        let window = derive::pet_window(&self.state);
        if last.window != Some(window) {
            let _ = links.pet.send(PetControl::Window(window));
            last.window = Some(window);
        }

        let music = self.state.audio().is_active();
        if last.music != Some(music) {
            let _ = links.beat.send(BeatControl::Active(music));
            let _ = links.pet.send(PetControl::MusicActive(music));
            if let Some(tray) = &links.tray {
                tray.set_label(
                    "audio",
                    if music {
                        "停止听歌"
                    } else {
                        "开始听歌"
                    },
                );
            }
            last.music = Some(music);
        }

        let locked = self.state.pet().locked;
        if last.locked != Some(locked) {
            if let Some(tray) = &links.tray {
                tray.set_checked("lock", locked);
            }
            last.locked = Some(locked);
        }

        let autostart = self.state.settings().autostart;
        if last.autostart != Some(autostart) {
            apply_autostart(autostart);
            if let Some(tray) = &links.tray {
                tray.set_checked("autostart", autostart);
            }
            last.autostart = Some(autostart);
        }

        let visible = self.state.pet().console_visible;
        if visible {
            self.console.render(&self.state, now);
        }
        if visible != last.console {
            if let Err(e) = self.console.set_visible(visible) {
                log::warn!("控制台窗口显示失败：{e}");
            }
            last.console = visible;
        }

        if self.state.persisted() != self.saved && !self.save_timer.running() {
            self.save_timer
                .start(slint::TimerMode::SingleShot, SAVE_DELAY, || {
                    with_app(App::save);
                });
        }
    }

    /// 写存档。失败只记日志，不重试（下次有变化时会再写）。
    pub fn save(&mut self) {
        let persisted = self.state.persisted();
        if persisted == self.saved {
            return;
        }
        if let Some(path) = &self.state_file
            && let Err(e) = yume_win::storage::write_text_atomic(path, &persisted.to_json())
        {
            log::warn!("保存失败：{e}");
        }
        self.saved = persisted;
    }

    /// 退出：先保存，再让各线程结束，最后退出事件循环。
    pub fn shutdown(&mut self) {
        self.quitting = true;
        self.save_timer.stop();
        self.save();
        self.links.audio.stop();
        let _ = self.links.pet.send(PetControl::Quit);
        let _ = self.links.beat.send(BeatControl::Quit);
        let _ = self.console.set_visible(false);
        let _ = slint::quit_event_loop();
    }
}

fn apply_autostart(on: bool) {
    let command = std::env::current_exe()
        .ok()
        .map(|exe| format!("\"{}\"", exe.display()));
    let result = if on {
        command
            .as_deref()
            .map_or(Ok(()), |c| yume_win::set_autostart(AUTOSTART_NAME, Some(c)))
    } else {
        yume_win::set_autostart(AUTOSTART_NAME, None)
    };
    if let Err(e) = result {
        log::warn!("设置开机自启失败：{e}");
    }
}
