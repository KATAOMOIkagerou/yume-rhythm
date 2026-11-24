//! 桌宠线程：拥有桌宠窗口和角色渲染器。每帧读最新的场景与节拍帧，合成后提交给窗口。
//! 拖动、右键菜单、拖入文件在这里转成 `Command` 发给主线程。

use crate::ToMain;
use crate::beat::MotionSlot;
use crate::clock::{FramePacer, Mono};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use yume_character::{CharacterRenderer, Frame};
use yume_core::Command;
use yume_core::command::{AudioCommand, PetCommand};
use yume_core::derive::{PetScene, PetWindow as WindowProps};
use yume_core::model::events::ActivityMode;
use yume_core::model::expression::NEUTRAL;
use yume_core::model::settings::PetPosition;
use yume_win::{FrameTimer, MenuItem, PetEvent, PetWindow};

pub enum PetControl {
    Scene(PetScene),
    Window(WindowProps),
    Character(Box<dyn CharacterRenderer>),
    /// 右键菜单里「开始/停止听歌」的文案。
    MusicActive(bool),
    Quit,
}

/// 每轮最多处理的窗口消息数。拖动时鼠标消息不断，处理一批就回来看看该不该出帧。
const MESSAGE_BATCH: usize = 32;

const MENU_CONSOLE: u32 = 1;
const MENU_AUDIO: u32 = 2;
const MENU_LOCK: u32 = 3;
const MENU_QUIT: u32 = 4;

pub fn spawn(
    control: Receiver<PetControl>,
    motion: Arc<MotionSlot>,
    clock: Mono,
    to_main: ToMain,
    anchor: Option<PetPosition>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("pet".into())
        .spawn(move || {
            let mut pet = Pet {
                control,
                motion,
                clock,
                to_main,
                anchor: anchor.map(|p| (p.x, p.y)),
                renderer: None,
                scene: idle_scene(),
                props: WindowProps {
                    locked: false,
                    visible: true,
                    scale: 1.0,
                    opacity: 1.0,
                },
                music: false,
                placed: false,
                desktop_ticks: 0,
                was_desktop_fg: false,
                retop_countdown: 0,
            };
            if let Err(e) = pet.run() {
                log::error!("桌宠窗口出错：{e}");
                (pet.to_main)(Command::Pet(PetCommand::Quit));
            }
        })
}

fn idle_scene() -> PetScene {
    PetScene {
        activity: ActivityMode::Chill,
        music: false,
        follow_beat: false,
        timer_running: false,
        base_expression: NEUTRAL.into(),
        accessory_expression: None,
        event: None,
        dozing: false,
    }
}

struct Pet {
    control: Receiver<PetControl>,
    motion: Arc<MotionSlot>,
    clock: Mono,
    to_main: ToMain,
    /// 窗口底边中点（物理像素）。缩放变化时以它为锚点，也是保存的位置。
    anchor: Option<(i32, i32)>,
    renderer: Option<Box<dyn CharacterRenderer>>,
    scene: PetScene,
    props: WindowProps,
    music: bool,
    placed: bool,
    desktop_ticks: u8,
    was_desktop_fg: bool,
    retop_countdown: u8,
}

impl Pet {
    fn run(&mut self) -> anyhow::Result<()> {
        // 先放在屏幕外，第一帧画好、知道尺寸后再摆到位。
        let mut window = PetWindow::create("yume-rhythm", -32_000, -32_000)?;
        let timer = FrameTimer::new()?;
        let mut frame = Frame::default();
        let mut pacer = FramePacer::new(self.clock.now_ms());
        let mut stats = FrameStats::default();

        loop {
            if !yume_win::pump_messages(MESSAGE_BATCH) || !self.apply_controls(&mut window) {
                return Ok(());
            }
            for event in window.take_events() {
                self.on_event(&window, event, &frame);
            }

            // 跟拍时 60 fps；平时 30 fps 足够呼吸动画；隐藏时只处理消息。
            let interval = if !self.props.visible || self.renderer.is_none() {
                100.0
            } else if self.scene.follow_beat {
                1000.0 / 60.0
            } else {
                1000.0 / 30.0
            };
            let now = self.clock.now_ms();
            if !pacer.due(now) {
                // 被消息提前唤醒（例如拖动），还没到出帧时间。
                timer.wait(pacer.wait_ms(now, interval));
                continue;
            }
            pacer.advance(now, interval);
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.advance(now, &self.scene, &self.motion.get());
                if self.props.visible {
                    let started = Instant::now();
                    let changed =
                        renderer.render(&mut frame, self.props.scale * window.dpi_scale());
                    let rendered = Instant::now();
                    if !self.placed {
                        self.place(&window, &frame);
                    }
                    // 画面没变（静止或呼吸幅度不到 1/16 像素）就不提交。
                    if changed
                        && let Err(e) = window.present(frame.bgra(), frame.width(), frame.height())
                    {
                        log::warn!("提交画面失败：{e}");
                    }
                    self.keep_above_desktop(&window);
                    stats.record(started, rendered, changed, &frame);
                }
            }
            timer.wait(pacer.wait_ms(self.clock.now_ms(), interval));
        }
    }

    /// 桌面外壳成为前台（关闭所有窗口或按 `Win+D`）时重新置顶，避免被 `WorkerW` 遮挡。
    fn keep_above_desktop(&mut self, window: &PetWindow) {
        if !self.placed {
            return;
        }
        self.desktop_ticks = self.desktop_ticks.wrapping_add(1);
        if self.desktop_ticks < 15 {
            return;
        }
        self.desktop_ticks = 0;
        let desktop_fg = yume_win::desktop_shell_active();
        if desktop_fg && !self.was_desktop_fg {
            self.retop_countdown = 3;
        } else if !desktop_fg {
            self.retop_countdown = 0;
        }
        self.was_desktop_fg = desktop_fg;
        if self.retop_countdown > 0 {
            window.ensure_topmost();
            self.retop_countdown -= 1;
        }
    }

    /// 处理主线程发来的控制。返回 `false` 表示退出。
    fn apply_controls(&mut self, window: &mut PetWindow) -> bool {
        loop {
            match self.control.try_recv() {
                Ok(PetControl::Scene(scene)) => self.scene = scene,
                Ok(PetControl::Window(props)) => {
                    if props.locked != self.props.locked {
                        window.set_click_through(props.locked);
                    }
                    if props.visible != self.props.visible {
                        window.set_visible(props.visible);
                    }
                    if (props.opacity - self.props.opacity).abs() > f32::EPSILON {
                        let alpha = yume_core::model::settings::opacity_to_u8(props.opacity);
                        window.set_opacity(alpha);
                    }
                    self.props = props;
                }
                Ok(PetControl::Character(renderer)) => self.renderer = Some(renderer),
                Ok(PetControl::MusicActive(on)) => self.music = on,
                Ok(PetControl::Quit) | Err(TryRecvError::Disconnected) => return false,
                Err(TryRecvError::Empty) => return true,
            }
        }
    }

    /// 第一帧：按保存的底边中点摆放，并保证在某个显示器的工作区内。
    fn place(&mut self, window: &PetWindow, frame: &Frame) {
        let (w, h) = (to_i32(frame.width()), to_i32(frame.height()));
        let saved = self.anchor.map(|(x, y)| (x - w / 2, y - h));
        let (x, y) = yume_win::place_on_screen(saved, (w, h));
        window.set_position(x, y);
        self.anchor = Some((x + w / 2, y + h));
        self.placed = true;
    }

    fn on_event(&mut self, window: &PetWindow, event: PetEvent, frame: &Frame) {
        let send = |cmd| (self.to_main)(cmd);
        match event {
            PetEvent::Moved { x, y } => {
                let (ax, ay) = (x + to_i32(frame.width()) / 2, y + to_i32(frame.height()));
                self.anchor = Some((ax, ay));
                send(Command::Pet(PetCommand::Moved(PetPosition {
                    x: ax,
                    y: ay,
                })));
            }
            PetEvent::Clicked | PetEvent::ActivateRequested => {
                send(Command::Pet(PetCommand::ShowConsole));
            }
            PetEvent::FilesDropped(paths) => {
                send(Command::Audio(AudioCommand::FilesDropped(paths)));
            }
            PetEvent::ContextMenu { x, y } => {
                let item = |id, label: &str| MenuItem::Action {
                    id,
                    label: label.into(),
                    checked: false,
                };
                let items = [
                    item(MENU_CONSOLE, "打开控制台"),
                    item(
                        MENU_AUDIO,
                        if self.music {
                            "停止听歌"
                        } else {
                            "开始听歌"
                        },
                    ),
                    item(MENU_LOCK, "锁定桌宠"),
                    MenuItem::Separator,
                    item(MENU_QUIT, "退出"),
                ];
                let cmd = match window.show_menu(&items, x, y) {
                    Some(MENU_CONSOLE) => Some(Command::Pet(PetCommand::ShowConsole)),
                    Some(MENU_AUDIO) => Some(Command::Audio(AudioCommand::Toggle)),
                    Some(MENU_LOCK) => Some(Command::Pet(PetCommand::SetLocked(true))),
                    Some(MENU_QUIT) => Some(Command::Pet(PetCommand::Quit)),
                    _ => None,
                };
                if let Some(cmd) = cmd {
                    send(cmd);
                }
            }
        }
    }
}

/// 每 30 秒在日志里记一次帧率和耗时，方便在用户机器上排查性能。
struct FrameStats {
    since: Instant,
    frames: u32,
    presented: u32,
    render: Duration,
    present: Duration,
}

impl Default for FrameStats {
    fn default() -> Self {
        Self {
            since: Instant::now(),
            frames: 0,
            presented: 0,
            render: Duration::ZERO,
            present: Duration::ZERO,
        }
    }
}

impl FrameStats {
    fn record(&mut self, started: Instant, rendered: Instant, changed: bool, frame: &Frame) {
        self.frames += 1;
        self.render += rendered - started;
        if changed {
            self.presented += 1;
            self.present += rendered.elapsed();
        }
        let elapsed = self.since.elapsed();
        if elapsed >= Duration::from_secs(30) {
            let n = f64::from(self.frames.max(1));
            let shown = f64::from(self.presented.max(1));
            log::info!(
                "桌宠 {:.1} fps，其中重画 {:.0}%，合成 {:.2} ms/帧，提交 {:.2} ms/次（{}×{}）",
                n / elapsed.as_secs_f64(),
                f64::from(self.presented) * 100.0 / n,
                self.render.as_secs_f64() * 1000.0 / n,
                self.present.as_secs_f64() * 1000.0 / shown,
                frame.width(),
                frame.height()
            );
            *self = Self::default();
        }
    }
}

fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}
