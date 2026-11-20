#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pet_audio::AudioEngineHandle;
use pet_core::{
    load_character_assets, load_persisted_state, monotonic_ms, resolve_data_dir, wall_ms,
    AppState, CharacterAnimator,
};
use pet_render::{PetCompositor, PetRenderInput, PetWindow, PET_LOGICAL_HEIGHT, PET_LOGICAL_WIDTH};
use pet_ui::ConsoleWindow;

fn run_self_test() -> Result<(), String> {
    let assets = load_character_assets()?;
    let bpm = AudioEngineHandle::self_test_120bpm()?;
    println!(
        "{}",
        serde_json::json!({
            "ok": true,
            "packName": assets.manifest.pack_name,
            "expressions": assets.expressions.len(),
            "sprites": assets.sprites.len(),
            "decodedKiB": assets.total_decoded_bytes / 1024,
            "fullCanvasKiB": assets.full_canvas_equivalent_bytes / 1024,
            "selfTestBpm": bpm,
        })
    );
    Ok(())
}

fn run_bench() -> Result<(), String> {
    let assets = load_character_assets()?;
    let state = AppState::new(load_persisted_state(&resolve_data_dir()));
    let mut animator = CharacterAnimator::default();
    let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);
    let mut frame_times_ms = Vec::with_capacity(240);

    let start = monotonic_ms();
    for i in 0..240u64 {
        let now_ms = start + (i as f64) * (1000.0 / 24.0);
        let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        let pose = animator.step(
            now_ms,
            &state.audio_telemetry,
            false,
            true,
            layers.is_dozing,
            layers.suppress_blink,
            assets.manifest.nod_amplitude_scale,
            assets.manifest.nod_tilt_scale,
        );
        let t0 = std::time::Instant::now();
        let input = PetRenderInput {
            assets: &assets,
            pose: &pose,
            layers: &layers,
            activity_mode: state.activity_mode,
            music_running: false,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: i * 42,
        };
        let _ = compositor.render_frame(&input);
        frame_times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }

    frame_times_ms.sort_by(|a, b| a.total_cmp(b));
    let p50 = frame_times_ms[frame_times_ms.len() / 2];
    let p95 = frame_times_ms[(frame_times_ms.len() * 95) / 100];
    let max = *frame_times_ms.last().unwrap_or(&0.0);
    println!(
        "{}",
        serde_json::json!({
            "frames": frame_times_ms.len(),
            "p50Ms": p50,
            "p95Ms": p95,
            "maxMs": max,
            "decodedKiB": assets.total_decoded_bytes / 1024,
        })
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--self-test") {
        if let Err(err) = run_self_test() {
            eprintln!("自检失败：{err}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|a| a == "--bench") {
        if let Err(err) = run_bench() {
            eprintln!("性能基准失败：{err}");
            std::process::exit(1);
        }
        return;
    }

    let data_dir = resolve_data_dir();
    let mut app_state = AppState::new(load_persisted_state(&data_dir));
    let assets = match load_character_assets() {
        Ok(a) => a,
        Err(err) => {
            eprintln!("加载角色素材失败：{err}");
            std::process::exit(1);
        }
    };
    let _audio = AudioEngineHandle::spawn().expect("init audio engine");
    let _pet = PetWindow::create().expect("create pet window");
    let _console = ConsoleWindow::new();
    let _ = app_state.tick(&assets.expressions, monotonic_ms(), wall_ms());
}
