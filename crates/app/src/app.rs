//! 主线程的组装：持有 `AppState`，执行 `update` 与 `Effect`，
//! 再把派生值的变化应用到桌宠、节拍线程、托盘、控制台和存档（docs/架构.md 第 3 节）。
//!
//! 这里不做业务判断：规则都在 core，这里只负责"把结果送到该去的地方"。

use crate::beat::BeatControl;
use crate::characters;
use crate::clock;
use crate::custom_expr;
use crate::io_tasks;
use crate::pet::PetControl;
use crate::shell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;
use yume_audio::{AudioService, Source};
use yume_character::{CharacterRenderer, LayeredCharacter};
use yume_console::Console;
use yume_core::command::{CharacterCommand, SettingsCommand, TodoCommand};
use yume_core::derive::{self, PetScene, PetWindow};
use yume_core::model::audio::{AudioSource, TempoShift};
use yume_core::model::settings::opacity_to_u8;
use yume_core::model::shortcuts::Shortcuts;
use yume_core::model::todo_md::render_md;
use yume_core::persist::Persisted;
use yume_core::{AppState, Command, Ctx, Effect, SplitMix64, WallMs, update};
use yume_win::{Hotkeys, Tray};

const SAVE_DELAY: Duration = Duration::from_millis(500);
const AUTOSTART_NAME: &str = "韵梦桌伴";

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
    pub hotkeys: Option<Hotkeys>,
}

pub struct App {
    state: AppState,
    rng: SplitMix64,
    console: Console,
    links: Links,
    roots: Vec<PathBuf>,
    user_dir: Option<PathBuf>,
    state_file: Option<PathBuf>,
    current_canvas: (u32, u32),
    saved: Persisted,
    saved_md_seq: u64,
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
    console_opacity: Option<u8>,
    shortcuts: Option<Shortcuts>,
}

/// 后台加载好的角色。
struct LoadedCharacter {
    name: String,
    expressions: Vec<yume_core::model::expression::ExpressionMeta>,
    canvas: (u32, u32),
    custom_warning: Option<String>,
    renderer: Box<dyn CharacterRenderer>,
}

impl App {
    #[must_use]
    pub fn new(
        state: AppState,
        console: Console,
        links: Links,
        roots: Vec<PathBuf>,
        user_dir: Option<PathBuf>,
        state_file: Option<PathBuf>,
        first_run: bool,
    ) -> Self {
        let mut saved = state.persisted();
        if first_run {
            // 首次运行：让存档"过期"，下一次同步就会写出一份，下次启动不再当作首次运行。
            saved.version = 0;
        }
        let initial_shortcuts = state.settings().shortcuts.clone();
        Self {
            state,
            rng: SplitMix64::new(clock::seed()),
            console,
            links,
            roots,
            user_dir,
            state_file,
            current_canvas: (800, 800),
            saved,
            saved_md_seq: 0,
            save_timer: slint::Timer::default(),
            last: Applied {
                shortcuts: Some(initial_shortcuts),
                ..Applied::default()
            },
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
            Effect::PickTodoMdImport => {
                let owner = self.console.native_handle();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(cmd) = io_tasks::pick_todo_md_import(owner) {
                        dispatch(cmd);
                    }
                });
            }
            Effect::PickTodoMdExport => {
                let owner = self.console.native_handle();
                let list_name = self.state.todo().name().to_owned();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(cmd) = io_tasks::pick_todo_md_export(owner, &list_name) {
                        dispatch(cmd);
                    }
                });
            }
            Effect::LoadTodoMd { path } => {
                dispatch(io_tasks::load_todo_md(&path));
            }
            Effect::PickFontFile => {
                let owner = self.console.native_handle();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(cmd) = io_tasks::pick_font_file(owner) {
                        dispatch(cmd);
                    }
                });
            }
            Effect::LoadCustomFont { path } => {
                dispatch(io_tasks::load_custom_font(&path));
            }
            Effect::ListFonts => {
                let fonts = yume_console::fonts::list_system_fonts();
                dispatch(Command::Settings(SettingsCommand::FontsListed(fonts)));
            }
            Effect::PickBackgroundImage => {
                let owner = self.console.native_handle();
                let user_dir = self.user_dir.clone();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(cmd) = io_tasks::pick_background_image(owner, user_dir.as_deref()) {
                        dispatch(cmd);
                    }
                });
            }
            Effect::ApplyBackgroundImage { path } => {
                if let Err(msg) = self.console.set_background_image(path.as_deref()) {
                    dispatch(Command::Settings(SettingsCommand::BackgroundImageFailed(
                        msg,
                    )));
                }
            }
            Effect::PickExpressionAsset { slot } => {
                let owner = self.console.native_handle();
                let canvas = self.current_canvas;
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(cmd) = custom_expr::pick_asset(owner, slot, canvas) {
                        dispatch(cmd);
                    }
                });
            }
            Effect::SaveCustomExpression(spec) => {
                let custom = characters::custom_root(self.user_dir.as_deref());
                custom_expr::save_and_dispatch(custom.as_deref(), &spec, dispatch);
            }
            Effect::DeleteCustomExpression { character, id } => {
                let custom = characters::custom_root(self.user_dir.as_deref());
                custom_expr::delete_and_dispatch(custom.as_deref(), &character, &id, dispatch);
            }
            // 要等这次同步把窗口显示出来之后才能带到前面，见 `handle`。
            Effect::FocusConsole => self.focus_console = true,
            Effect::Quit => self.quitting = true,
        }
    }

    /// 在后台线程读图、建渲染器，完成后回到主线程交给桌宠。
    fn load_character(&mut self, id: String) {
        let roots = self.roots.clone();
        let custom_root = characters::custom_root(self.user_dir.as_deref());
        let seed = clock::seed();
        let spawned = std::thread::Builder::new()
            .name("load-character".into())
            .spawn(move || {
                let result = characters::load(&roots, custom_root.as_deref(), &id).map(|pack| {
                    let canvas = (pack.manifest.canvas.width, pack.manifest.canvas.height);
                    LoadedCharacter {
                        name: pack.manifest.name.clone(),
                        expressions: pack.expression_meta(),
                        canvas,
                        custom_warning: pack.custom_warning.clone(),
                        renderer: Box::new(LayeredCharacter::new(pack, seed)),
                    }
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
    fn accept_character(&mut self, id: String, result: Result<LoadedCharacter, String>) -> Command {
        match result {
            Ok(loaded) => {
                if self.state.settings().character == id {
                    self.current_canvas = loaded.canvas;
                    let _ = self.links.pet.send(PetControl::Character(loaded.renderer));
                }
                log::info!("角色已加载：{id}");
                Command::Character(CharacterCommand::Loaded {
                    id,
                    name: loaded.name,
                    expressions: loaded.expressions,
                    canvas: loaded.canvas,
                    custom_warning: loaded.custom_warning,
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
        let became_visible = visible && !last.console;
        if visible != last.console {
            if let Err(e) = self.console.set_visible(visible) {
                log::warn!("控制台窗口显示失败：{e}");
            }
            last.console = visible;
        }
        let console_alpha = opacity_to_u8(derive::console_opacity(&self.state));
        if visible
            && (became_visible || last.console_opacity != Some(console_alpha))
            && let Some(hwnd) = self.console.native_handle()
        {
            yume_win::set_window_opacity(hwnd, console_alpha);
            last.console_opacity = Some(console_alpha);
        }

        let current_shortcuts = self.state.settings().shortcuts.clone();
        if last.shortcuts.as_ref() != Some(&current_shortcuts) {
            last.shortcuts = Some(current_shortcuts.clone());
            if let Some(hotkeys) = self.links.hotkeys.as_mut() {
                let failed = shell::rebind_hotkeys(hotkeys, &current_shortcuts);
                dispatch(Command::Settings(SettingsCommand::GlobalHotkeysFailed(
                    failed,
                )));
            }
        }

        let needs_save = self.state.persisted() != self.saved
            || self.state.todo_md_dirty_seq() != self.saved_md_seq;
        if needs_save && !self.save_timer.running() {
            self.save_timer
                .start(slint::TimerMode::SingleShot, SAVE_DELAY, || {
                    with_app(App::save);
                });
        }
    }

    /// 写存档与外接 Markdown 清单。失败只记日志或回报错误，不重试（下次有变化时会再写）。
    pub fn save(&mut self) {
        let persisted = self.state.persisted();
        if persisted != self.saved {
            if let Some(path) = &self.state_file
                && let Err(e) = yume_win::storage::write_text_atomic(path, &persisted.to_json())
            {
                log::warn!("保存失败：{e}");
            }
            self.saved = persisted;
        }

        let md_seq = self.state.todo_md_dirty_seq();
        if md_seq != self.saved_md_seq {
            self.saved_md_seq = md_seq;
            if let Some(path) = self.state.settings().todo_md_path.clone() {
                let content = render_md(self.state.todo_md_doc(), self.state.todo());
                if let Err(err) = io_tasks::save_todo_md(&path, &content) {
                    dispatch(Command::Todo(TodoCommand::MdFailed(err)));
                }
            }
        }
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
