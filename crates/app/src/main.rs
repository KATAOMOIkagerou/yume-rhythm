//! L3 组装层入口：读存档、开线程、接线，然后运行 Slint 事件循环。
//! 线程与通信方式见 docs/架构.md 第 5 节。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(
    not(windows),
    allow(dead_code, reason = "非 Windows 平台只编译纯逻辑部分供测试")
)]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod beat;
mod characters;
mod clock;
#[cfg(windows)]
mod custom_expr;
#[cfg(windows)]
mod io_tasks;
mod logging;
#[cfg(windows)]
mod pet;
#[cfg(windows)]
mod shell;

use std::sync::Arc;
use yume_core::Command;

/// 从任意线程把命令投递到主线程。
pub type ToMain = Arc<dyn Fn(Command) + Send + Sync>;

/// 其他平台只编译纯逻辑部分（测试用），不提供可运行的程序。
#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            log::error!("启动失败：{e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn run() -> anyhow::Result<()> {
    use std::rc::Rc;
    use std::sync::mpsc;
    use yume_audio::{AudioService, SampleRing};
    use yume_console::Console;
    use yume_core::AppState;
    use yume_core::command::PetCommand;
    use yume_core::persist::Persisted;

    yume_win::enable_per_monitor_dpi();
    let roaming = yume_win::storage::roaming_dir();
    let local = yume_win::storage::local_dir();
    logging::init(local.map(|d| d.join("logs")).as_deref());
    log::info!("韵梦桌伴 v{} 启动", env!("CARGO_PKG_VERSION"));

    let Some(_instance) = yume_win::acquire_single_instance("YunMengZhuoBan.SingleInstance") else {
        log::info!("已有实例在运行，通知它显示控制台");
        yume_win::notify_existing_instance();
        return Ok(());
    };

    let state_file = roaming.as_ref().map(|d| d.join("state.json"));
    let saved = state_file
        .as_deref()
        .map(yume_win::storage::read_text)
        .transpose()
        .unwrap_or_else(|e| {
            log::warn!("读取存档失败，使用默认设置：{e}");
            None
        })
        .flatten();
    let first_run = saved.is_none();
    let persisted = saved.map_or_else(Persisted::default, |text| Persisted::from_json(&text));
    let anchor = persisted.settings.pet_position;
    let state = AppState::new(persisted);

    // Slint 后端在创建第一个窗口时初始化；之后其他线程才能往事件循环投递命令。
    let console = Console::new(Rc::new(app::dispatch))?;
    let to_main: ToMain = Arc::new(|cmd| {
        if slint::invoke_from_event_loop(move || app::dispatch(cmd)).is_err() {
            log::debug!("事件循环已结束，丢弃命令");
        }
    });

    let mono = clock::Mono::start();
    let ring = Arc::new(SampleRing::new(16_384));
    let motion = Arc::new(beat::MotionSlot::default());
    let (beat_tx, beat_rx) = mpsc::channel();
    let beat_thread = beat::spawn(ring.clone(), motion.clone(), beat_rx, mono, to_main.clone())?;
    let (pet_tx, pet_rx) = mpsc::channel();
    let pet_thread = pet::spawn(pet_rx, motion, mono, to_main.clone(), anchor)?;
    let audio = {
        let to_main = to_main.clone();
        AudioService::spawn(ring, move |event| to_main(shell::audio_command(event)))
    };

    let initial_shortcuts = state.settings().shortcuts.clone();
    let (hotkeys, failed_hotkeys) = shell::hotkeys(to_main.clone(), &initial_shortcuts);
    let links = app::Links {
        pet: pet_tx,
        beat: beat_tx,
        audio,
        tray: shell::tray(to_main),
        hotkeys,
    };
    let roots = characters::roots(roaming.as_deref());
    log::info!("角色目录：{roots:?}");
    app::install(app::App::new(
        state, console, links, roots, roaming, state_file, first_run,
    ));
    let _timers = shell::timers();

    if !failed_hotkeys.is_empty() {
        app::dispatch(Command::Settings(
            yume_core::command::SettingsCommand::GlobalHotkeysFailed(failed_hotkeys),
        ));
    }
    app::dispatch(Command::Startup);
    if first_run {
        app::dispatch(Command::Pet(PetCommand::ShowConsole));
    }
    slint::run_event_loop_until_quit()?;

    // 正常退出时 `shutdown` 已执行；这里兜底保存，并等待各线程结束。
    if let Some(mut app) = app::uninstall() {
        app.shutdown();
    }
    let _ = pet_thread.join();
    let _ = beat_thread.join();
    log::info!("已退出");
    Ok(())
}
