#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diagnostics;
mod metrics;
mod shell;

use diagnostics::{run_bench, run_self_test, BenchOptions};
use metrics::ProcessSample;
use pet_audio::AudioEngineHandle;
use pet_core::{
    is_supported_audio_file, load_character_assets, load_persisted_state, monotonic_ms,
    resolve_data_dir, save_persisted_state, wall_ms, AppAction, AppEffect, AppState, AssetStatus,
    CharacterAnimator, LoadedCharacterAssets,
};
use pet_render::{PetRenderInput, PetWindow, PetWindowEvent, WindowRect};
use pet_ui::{AnchorRect, ConsoleRuntimeMetrics, ConsoleWindow};
use shell::{
    attach_parent_console_for_cli, init_dpi_awareness, open_system_sound_settings,
    pump_win32_messages, show_fatal_error, wait_for_messages_or_timeout, ShellIntegration,
};
use std::path::{Path, PathBuf};

const TARGET_FPS: f64 = 24.0;
const FRAME_INTERVAL_MS: f64 = 1000.0 / TARGET_FPS;

enum CliMode {
    Gui,
    SelfTest { out_file: Option<PathBuf> },
    Bench(BenchOptions),
}

struct CliArgs {
    mode: CliMode,
    data_dir: Option<PathBuf>,
}

fn parse_cli_args() -> CliArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut data_dir = None;
    let mut out_file = None;
    let mut screenshot = None;
    let mut console_screenshot = None;
    let mut console_tab = None;
    let mut with_console = false;
    let mut is_self_test = false;
    let mut bench_secs: Option<f64> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--self-test" => is_self_test = true,
            "--bench" => {
                let next_num = args
                    .get(i + 1)
                    .filter(|s| !s.starts_with("--"))
                    .and_then(|s| s.parse::<f64>().ok());
                if let Some(secs) = next_num {
                    bench_secs = Some(secs);
                    i += 1;
                } else {
                    bench_secs = Some(5.0);
                }
            }
            "--with-console" => with_console = true,
            "--console-tab" => {
                if let Some(val) = args.get(i + 1) {
                    console_tab = match val.to_ascii_lowercase().as_str() {
                        "status" => Some(pet_core::MainTab::Status),
                        "emotion" => Some(pet_core::MainTab::Emotion),
                        "audio" | "audiosync" => Some(pet_core::MainTab::AudioSync),
                        "settings" => Some(pet_core::MainTab::Settings),
                        _ => None,
                    };
                    i += 1;
                }
            }
            "--data-dir" => {
                if let Some(val) = args.get(i + 1) {
                    data_dir = Some(PathBuf::from(val));
                    i += 1;
                }
            }
            "--screenshot" => {
                if let Some(val) = args.get(i + 1) {
                    screenshot = Some(PathBuf::from(val));
                    i += 1;
                }
            }
            "--console-screenshot" => {
                if let Some(val) = args.get(i + 1) {
                    console_screenshot = Some(PathBuf::from(val));
                    i += 1;
                }
            }
            "--out" => {
                if let Some(val) = args.get(i + 1) {
                    out_file = Some(PathBuf::from(val));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mode = if is_self_test {
        CliMode::SelfTest { out_file }
    } else if let Some(duration_secs) = bench_secs {
        CliMode::Bench(BenchOptions {
            duration_secs,
            with_console,
            console_tab,
            screenshot,
            console_screenshot,
            out_file,
        })
    } else {
        CliMode::Gui
    };

    CliArgs { mode, data_dir }
}

fn anchor_from_pet_rect(rect: WindowRect) -> AnchorRect {
    AnchorRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        scale_factor: rect.scale_factor,
    }
}

fn apply_asset_status(state: &mut AppState, assets: &LoadedCharacterAssets) {
    state.asset_status = AssetStatus {
        loaded: true,
        pack_name: assets.manifest.pack_name.clone(),
        source_hint: assets.source_hint.clone(),
        error: None,
    };
}

fn run_gui(data_dir: &Path) -> Result<(), String> {
    init_dpi_awareness();

    let mut assets = load_character_assets()?;
    let mut app_state = AppState::new(load_persisted_state(data_dir));
    apply_asset_status(&mut app_state, &assets);

    let audio = AudioEngineHandle::spawn()?;
    app_state.audio_devices = audio.devices();

    let mut pet = PetWindow::create()?;
    let mut console = ConsoleWindow::new();
    if app_state.console_visible {
        if let Err(err) = console.show_near(anchor_from_pet_rect(pet.window_rect())) {
            eprintln!("初始打开控制台失败：{err}");
            app_state.console_visible = false;
        }
    }

    let mut shell = ShellIntegration::init(
        console.is_visible(),
        app_state.pet_locked,
        app_state.music_running(),
    )?;

    let mut animator = CharacterAnimator::default();
    let app_start_ms = monotonic_ms();
    let mut next_frame_ms = app_start_ms;
    let mut next_metrics_ms = app_start_ms;
    let mut should_quit = false;

    while !should_quit {
        if pump_win32_messages() {
            break;
        }

        let now_ms = monotonic_ms();
        let wall_now_ms = wall_ms();

        // 1. Sync audio telemetry & enumerated output devices
        app_state.audio_telemetry = audio.latest_telemetry();
        let latest_devices = audio.devices();
        if !latest_devices.is_empty() || !app_state.audio_devices.is_empty() {
            app_state.audio_devices = latest_devices;
        }

        // 2. Collect incoming actions from tray/hotkeys, pet window, and floating console
        let mut pending_actions = shell.poll_actions();

        for event in pet.poll_events() {
            match event {
                PetWindowEvent::OpenConsoleClicked => pending_actions.push(AppAction::ShowConsole),
                PetWindowEvent::ToggleLockClicked => pending_actions.push(AppAction::ToggleLock),
                PetWindowEvent::ToggleAudioClicked => {
                    pending_actions.push(AppAction::TogglePlayLastAudio)
                }
                PetWindowEvent::RightClicked { .. } => {
                    pending_actions.push(AppAction::ToggleConsole)
                }
                PetWindowEvent::DroppedAudioFile(path) => {
                    if is_supported_audio_file(&path) {
                        pending_actions.push(AppAction::StartAudioFile(path));
                    }
                }
                PetWindowEvent::KeySpace => pending_actions.push(AppAction::TogglePlayLastAudio),
                PetWindowEvent::KeyQ => pending_actions.push(AppAction::StopAudio),
                PetWindowEvent::KeyH => pending_actions.push(AppAction::HideConsole),
                PetWindowEvent::CloseRequested => pending_actions.push(AppAction::Quit),
            }
        }

        if console.is_visible() {
            if now_ms >= next_metrics_ms {
                let sample = ProcessSample::capture(now_ms);
                console.set_runtime_metrics(ConsoleRuntimeMetrics {
                    sprite_count: assets.sprites.len(),
                    asset_decoded_bytes: assets.total_decoded_bytes,
                    asset_full_canvas_bytes: assets.full_canvas_equivalent_bytes,
                    pet_frame_ms: pet.last_render_ms() as f32,
                    ui_frame_ms: 0.0,
                    private_memory_mib: sample.private_mib() as f32,
                    loaded_font_name: None,
                });
                next_metrics_ms = now_ms + 1000.0;
            }
            let ui_actions = console.update(&mut app_state, &assets.expressions, now_ms);
            pending_actions.extend(ui_actions);
            if !console.is_visible() {
                app_state.console_visible = false;
            }
        }

        // 3. Dispatch actions + periodic state tick -> execute effects
        let mut effects = Vec::new();
        for action in pending_actions {
            effects.extend(app_state.dispatch(action, &assets.expressions, now_ms, wall_now_ms));
        }
        effects.extend(app_state.tick(&assets.expressions, now_ms, wall_now_ms));

        for effect in effects {
            match effect {
                AppEffect::Audio(cmd) => audio.send(cmd),
                AppEffect::PersistState => {
                    if let Err(err) = save_persisted_state(data_dir, &app_state.persisted) {
                        eprintln!("保存本地设置失败：{err}");
                    }
                }
                AppEffect::ReloadAssets => match load_character_assets() {
                    Ok(new_assets) => {
                        assets = new_assets;
                        pet.invalidate_cache();
                        apply_asset_status(&mut app_state, &assets);
                    }
                    Err(err) => {
                        app_state.asset_status.error = Some(err);
                    }
                },
                AppEffect::ApplyPetLock(locked) => {
                    if let Err(err) = pet.set_locked(locked) {
                        eprintln!("切换桌宠锁定状态失败：{err}");
                    }
                }
                AppEffect::ShowConsole => {
                    if let Err(err) = console.show_near(anchor_from_pet_rect(pet.window_rect())) {
                        eprintln!("显示控制台失败：{err}");
                        app_state.console_visible = false;
                    } else {
                        app_state.console_visible = true;
                    }
                }
                AppEffect::HideConsole => {
                    console.hide_and_release();
                    app_state.console_visible = false;
                }
                AppEffect::OpenSystemSoundSettings => open_system_sound_settings(),
                AppEffect::Quit => {
                    should_quit = true;
                }
            }
        }

        shell.sync_menu_state(
            console.is_visible(),
            app_state.pet_locked,
            app_state.music_running(),
        );

        if should_quit {
            break;
        }

        // 4. Pace pet rendering at 24fps
        if now_ms >= next_frame_ms {
            let mut layers = app_state.resolve_face_layers(
                &assets.manifest,
                &assets.expressions,
                false,
            );
            let pose = animator.step(
                now_ms,
                &app_state.audio_telemetry,
                app_state.music_running(),
                app_state.auto_follow,
                layers.is_dozing,
                layers.suppress_blink,
                assets.manifest.nod_amplitude_scale,
                assets.manifest.nod_tilt_scale,
            );
            if pose.is_blinking {
                layers = app_state.resolve_face_layers(
                    &assets.manifest,
                    &assets.expressions,
                    true,
                );
            }
            let input = PetRenderInput {
                assets: &assets,
                pose: &pose,
                layers: &layers,
                activity_mode: app_state.activity_mode,
                music_running: app_state.music_running(),
                timer_active: app_state.timer.state == pet_core::TimerState::Running,
                pet_locked: app_state.pet_locked,
                elapsed_ms: (now_ms - app_start_ms).max(0.0) as u64,
            };
            if let Err(err) = pet.present(&input) {
                eprintln!("渲染桌宠帧失败：{err}");
            }
            next_frame_ms = (next_frame_ms + FRAME_INTERVAL_MS).max(now_ms + 1.0);
        }

        // 5. Sleep cleanly via MsgWaitForMultipleObjects until next 24fps tick or OS message
        let wait_ms = (next_frame_ms - monotonic_ms()).clamp(1.0, 42.0) as u32;
        wait_for_messages_or_timeout(wait_ms);
    }

    console.hide_and_release();
    audio.send(pet_core::AudioCommand::Shutdown);
    Ok(())
}

fn main() {
    let cli = parse_cli_args();
    let data_dir = cli.data_dir.unwrap_or_else(resolve_data_dir);

    match cli.mode {
        CliMode::SelfTest { out_file } => {
            attach_parent_console_for_cli();
            if let Err(err) = run_self_test(&data_dir, out_file.as_deref()) {
                eprintln!("自检失败：{err}");
                std::process::exit(1);
            }
        }
        CliMode::Bench(opts) => {
            attach_parent_console_for_cli();
            if let Err(err) = run_bench(&data_dir, &opts) {
                eprintln!("性能基准失败：{err}");
                std::process::exit(1);
            }
        }
        CliMode::Gui => {
            if let Err(err) = run_gui(&data_dir) {
                eprintln!("启动失败：{err}");
                show_fatal_error(&err);
                std::process::exit(1);
            }
        }
    }
}
