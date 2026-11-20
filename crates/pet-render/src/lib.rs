//! Pure-Rust Win32 `WS_EX_LAYERED` + `UpdateLayeredWindow` pet window & `tiny-skia` tight-bbox compositor.
//! Owned by `@pet-renderer`.

mod compositor;
mod toolbar;
mod window;

pub use compositor::PetCompositor;
pub use toolbar::{toolbar_hit_test, toolbar_layout, ToolbarButton, ToolbarButtonRect, ToolbarState};
pub use window::{clamp_rect_to_bounds, PetWindow};

use pet_core::{ActivityMode, AnimationPose, LoadedCharacterAssets, ResolvedFaceLayers};
use std::path::PathBuf;

pub const PET_LOGICAL_WIDTH: u32 = 540;
pub const PET_LOGICAL_HEIGHT: u32 = 590;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f32,
}

#[derive(Debug, Clone)]
pub struct PetRenderInput<'a> {
    pub assets: &'a LoadedCharacterAssets,
    pub pose: &'a AnimationPose,
    pub layers: &'a ResolvedFaceLayers,
    pub activity_mode: ActivityMode,
    pub music_running: bool,
    pub timer_active: bool,
    pub pet_locked: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetWindowEvent {
    OpenConsoleClicked,
    ToggleLockClicked,
    ToggleAudioClicked,
    RightClicked { screen_x: i32, screen_y: i32 },
    DroppedAudioFile(PathBuf),
    KeySpace,
    KeyQ,
    KeyH,
    CloseRequested,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pet_core::{
        load_embedded_character_assets, AppAction, AppState, AudioTelemetry, CharacterAnimator,
        FastRng, NoteParticleRender, PersistedState,
    };

    fn assert_premultiplied_bgra_invariant(bgra: &[u8]) {
        assert_eq!(bgra.len() % 4, 0);
        for (idx, px) in bgra.chunks_exact(4).enumerate() {
            let [b, g, r, a] = [px[0], px[1], px[2], px[3]];
            assert!(
                b <= a && g <= a && r <= a,
                "premultiplied BGRA invariant violated at pixel {idx}: BGRA=({b}, {g}, {r}, {a})"
            );
        }
    }

    fn pixel_at(bgra: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let off = ((y as usize) * (width as usize) + (x as usize)) * 4;
        [bgra[off], bgra[off + 1], bgra[off + 2], bgra[off + 3]]
    }

    #[test]
    fn compositor_premultiplied_invariant_corners_and_character_center() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let state = AppState::new(PersistedState::default());
        let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        let pose = AnimationPose::default();

        let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);
        let input = PetRenderInput {
            assets: &assets,
            pose: &pose,
            layers: &layers,
            activity_mode: ActivityMode::Chill,
            music_running: false,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: 0,
        };
        let frame = compositor.render_frame(&input);
        assert_eq!(
            frame.len(),
            (PET_LOGICAL_WIDTH as usize) * (PET_LOGICAL_HEIGHT as usize) * 4
        );
        assert_premultiplied_bgra_invariant(frame);

        // Top corners must be 100% transparent (alpha == 0)
        assert_eq!(pixel_at(frame, PET_LOGICAL_WIDTH, 0, 0)[3], 0);
        assert_eq!(
            pixel_at(frame, PET_LOGICAL_WIDTH, PET_LOGICAL_WIDTH - 1, 0)[3],
            0
        );
        assert_eq!(pixel_at(frame, PET_LOGICAL_WIDTH, 12, 12)[3], 0);
        assert_eq!(
            pixel_at(frame, PET_LOGICAL_WIDTH, PET_LOGICAL_WIDTH - 13, 12)[3],
            0
        );

        // Center character area must have opaque pixels (alpha > 0)
        let center_px = pixel_at(
            frame,
            PET_LOGICAL_WIDTH,
            PET_LOGICAL_WIDTH / 2,
            PET_LOGICAL_HEIGHT / 2 + 20,
        );
        assert!(
            center_px[3] > 0,
            "expected opaque character pixel near center, got {center_px:?}"
        );
    }

    #[test]
    fn compositor_static_cache_hits_and_invalidates_on_mode_or_reload() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let state = AppState::new(PersistedState::default());
        let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        let pose = AnimationPose::default();

        let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);

        let chill_input = PetRenderInput {
            assets: &assets,
            pose: &pose,
            layers: &layers,
            activity_mode: ActivityMode::Chill,
            music_running: false,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: 0,
        };

        let frame1 = compositor.render_frame(&chill_input).to_vec();
        assert_eq!(compositor.static_cache_misses(), 1);
        assert_eq!(compositor.static_cache_hits(), 0);

        let frame2 = compositor.render_frame(&chill_input).to_vec();
        assert_eq!(compositor.static_cache_misses(), 1);
        assert_eq!(compositor.static_cache_hits(), 1);
        assert_eq!(frame1, frame2);

        // Switching ActivityMode::Chill -> Study must rebuild static cache & change output
        let study_input = PetRenderInput {
            activity_mode: ActivityMode::Study,
            ..chill_input.clone()
        };
        let frame_study = compositor.render_frame(&study_input).to_vec();
        assert_eq!(compositor.static_cache_misses(), 2);
        assert_ne!(frame1, frame_study, "Study mode should update desk/computer/arms pixels");

        // Subsequent Study frame hits cache
        let _ = compositor.render_frame(&study_input);
        assert_eq!(compositor.static_cache_hits(), 2);

        // Explicit invalidation triggers rebuild on next frame
        compositor.invalidate_static_cache();
        let _ = compositor.render_frame(&study_input);
        assert_eq!(compositor.static_cache_misses(), 3);
    }

    #[test]
    fn expressions_animations_effects_toolbar_and_hidpi_render_valid_premul() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let mut state = AppState::new(PersistedState::default());

        let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);
        let neutral_layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        let base_pose = AnimationPose::default();
        let base_frame = compositor
            .render_frame(&PetRenderInput {
                assets: &assets,
                pose: &base_pose,
                layers: &neutral_layers,
                activity_mode: ActivityMode::Chill,
                music_running: false,
                timer_active: false,
                pet_locked: false,
                elapsed_ms: 0,
            })
            .to_vec();

        // Change expression to Happy (base) + Think (accessory overlay)
        state.dispatch(
            AppAction::SelectExpression("happy".into()),
            &assets.expressions,
            0.0,
            0,
        );
        state.dispatch(
            AppAction::SelectExpression("think".into()),
            &assets.expressions,
            10.0,
            10,
        );
        let expr_layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        assert_eq!(expr_layers.overlay.as_deref(), Some("fx_think_bubble.png"));

        let mut active_pose = AnimationPose::default();
        active_pose.head_y = 6.5;
        active_pose.head_rot_deg = -4.2;
        active_pose.ring_radius = 180.0;
        active_pose.ring_opacity = 0.35;
        active_pose.ring_stroke_width = 8.0;
        active_pose.timer_glow_strength = 0.85;
        active_pose.notes = vec![
            NoteParticleRender {
                glyph: '♪',
                x_pct: 42.0,
                y_pct: 20.0,
                offset_x: 4.0,
                offset_y: -12.0,
                opacity: 0.9,
                scale: 1.0,
                rot_deg: 6.0,
                rgb: [240, 120, 210],
            },
            NoteParticleRender {
                glyph: '♫',
                x_pct: 58.0,
                y_pct: 18.0,
                offset_x: -6.0,
                offset_y: -18.0,
                opacity: 0.85,
                scale: 1.05,
                rot_deg: -5.0,
                rgb: [120, 195, 250],
            },
            NoteParticleRender {
                glyph: '✦',
                x_pct: 50.0,
                y_pct: 14.0,
                offset_x: 0.0,
                offset_y: -8.0,
                opacity: 0.95,
                scale: 0.95,
                rot_deg: 12.0,
                rgb: [255, 190, 120],
            },
        ];

        // Show hover toolbar
        compositor.set_toolbar_visible(true, Some(ToolbarButton::AudioSync), None);
        let rich_frame = compositor
            .render_frame(&PetRenderInput {
                assets: &assets,
                pose: &active_pose,
                layers: &expr_layers,
                activity_mode: ActivityMode::Study,
                music_running: true,
                timer_active: true,
                pet_locked: false,
                elapsed_ms: 420,
            })
            .to_vec();

        assert_premultiplied_bgra_invariant(&rich_frame);
        assert_ne!(
            base_frame, rich_frame,
            "expression/pose/ring/notes/toolbar must change rendered pixels"
        );
        // Even with hover toolbar shown, top-left and top-right corners remain alpha == 0
        assert_eq!(pixel_at(&rich_frame, PET_LOGICAL_WIDTH, 0, 0)[3], 0);
        assert_eq!(
            pixel_at(&rich_frame, PET_LOGICAL_WIDTH, PET_LOGICAL_WIDTH - 1, 0)[3],
            0
        );

        // Verify a toolbar button center actually has rendered non-transparent pixels
        let console_btn = toolbar_layout()[0];
        let btn_cx = (console_btn.x + console_btn.width * 0.5).round() as u32;
        let btn_cy = (console_btn.y + console_btn.height * 0.5).round() as u32;
        assert!(
            pixel_at(&rich_frame, PET_LOGICAL_WIDTH, btn_cx, btn_cy)[3] > 200,
            "toolbar button center should be opaque when hovered"
        );

        // High-DPI (1.5x) render must also preserve premultiplied invariant cleanly
        compositor.set_scale_factor(1.5);
        assert_eq!(compositor.dimensions(), (810, 885));
        let hidpi_frame = compositor.render_frame(&PetRenderInput {
            assets: &assets,
            pose: &active_pose,
            layers: &expr_layers,
            activity_mode: ActivityMode::Study,
            music_running: true,
            timer_active: true,
            pet_locked: false,
            elapsed_ms: 840,
        });
        assert_premultiplied_bgra_invariant(hidpi_frame);
    }

    #[test]
    fn base_animation_expression_renders_cleanly() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let state = AppState::new(PersistedState::default());
        let mut layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        // Simulate a base animation sprite replacement using an existing sprite asset
        layers.base_animation = Some(assets.manifest.layers.head_base.clone());
        let pose = AnimationPose::default();

        let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);
        let frame = compositor.render_frame(&PetRenderInput {
            assets: &assets,
            pose: &pose,
            layers: &layers,
            activity_mode: ActivityMode::Chill,
            music_running: false,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: 120,
        });
        assert_premultiplied_bgra_invariant(frame);
        let non_zero_pixels = frame.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(non_zero_pixels > 1000, "base animation should draw non-empty pixels");
    }

    #[test]
    fn toolbar_hit_test_and_work_area_clamping() {
        let rects = toolbar_layout();
        for rect in rects {
            let cx = rect.x + rect.width * 0.5;
            let cy = rect.y + rect.height * 0.5;
            assert_eq!(toolbar_hit_test(cx, cy), Some(rect.button));
        }
        assert_eq!(toolbar_hit_test(0.0, 0.0), None);
        assert_eq!(toolbar_hit_test(539.0, 0.0), None);
        assert_eq!(toolbar_hit_test(270.0, 300.0), None);

        assert_eq!(
            clamp_rect_to_bounds(-200, -100, 540, 590, 0, 0, 1920, 1040),
            (0, 0)
        );
        assert_eq!(
            clamp_rect_to_bounds(1800, 900, 540, 590, 0, 0, 1920, 1040),
            (1380, 450)
        );
    }

    #[test]
    fn benchmark_100_frames_compositor_timing() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let state = AppState::new(PersistedState::default());
        let mut animator = CharacterAnimator::new(FastRng::from_seed(42));
        let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);
        let mut telemetry = AudioTelemetry {
            running: true,
            locked: true,
            bpm: 120,
            period_ms: Some(500.0),
            level: 0.55,
            strength: 1.4,
            ..AudioTelemetry::default()
        };

        // Warm up caches on frame 0
        let init_layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        let init_pose = AnimationPose::default();
        let _ = compositor.render_frame(&PetRenderInput {
            assets: &assets,
            pose: &init_pose,
            layers: &init_layers,
            activity_mode: ActivityMode::Chill,
            music_running: true,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: 0,
        });

        let mut times_ms = Vec::with_capacity(100);
        for i in 1..=100u64 {
            let now_ms = (i as f64) * (1000.0 / 24.0);
            if i % 12 == 0 {
                telemetry.beats += 1;
                telemetry.next_beat_at_ms = Some(now_ms + 500.0);
            }
            let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
            let pose = animator.step(
                now_ms,
                &telemetry,
                true,
                true,
                layers.is_dozing,
                layers.suppress_blink,
                assets.manifest.nod_amplitude_scale,
                assets.manifest.nod_tilt_scale,
            );
            let input = PetRenderInput {
                assets: &assets,
                pose: &pose,
                layers: &layers,
                activity_mode: ActivityMode::Chill,
                music_running: true,
                timer_active: false,
                pet_locked: false,
                elapsed_ms: i * 42,
            };
            let _ = compositor.render_frame(&input);
            times_ms.push(compositor.last_render_ms());
        }

        let avg = times_ms.iter().sum::<f64>() / (times_ms.len() as f64);
        times_ms.sort_by(|a, b| a.total_cmp(b));
        let p50 = times_ms[times_ms.len() / 2];
        let p95 = times_ms[(times_ms.len() * 95) / 100];
        let max = *times_ms.last().unwrap_or(&0.0);
        println!(
            "[pet-render bench 100 frames @ 540x590] avg={avg:.3}ms p50={p50:.3}ms p95={p95:.3}ms max={max:.3}ms cache_hits={}",
            compositor.static_cache_hits()
        );
        assert!(compositor.static_cache_hits() >= 100);
    }

    #[cfg(windows)]
    #[test]
    fn pet_window_layered_lifecycle_and_lock_toggle() {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_EX_TRANSPARENT,
        };

        let assets = load_embedded_character_assets().expect("embedded assets load");
        let state = AppState::new(PersistedState::default());
        let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
        let pose = AnimationPose::default();

        let mut win = PetWindow::create().expect("PetWindow::create");
        let hwnd = win.hwnd() as HWND;
        assert!(!hwnd.is_null());

        let ex_unlocked = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_ne!(ex_unlocked & WS_EX_LAYERED, 0);
        assert_ne!(ex_unlocked & WS_EX_TOPMOST, 0);
        assert_ne!(ex_unlocked & WS_EX_TOOLWINDOW, 0);
        assert_eq!(
            ex_unlocked & WS_EX_TRANSPARENT,
            0,
            "unlocked PetWindow must NOT set WS_EX_TRANSPARENT (UpdateLayeredWindow handles alpha==0 natively)"
        );

        let input = PetRenderInput {
            assets: &assets,
            pose: &pose,
            layers: &layers,
            activity_mode: ActivityMode::Chill,
            music_running: false,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: 0,
        };
        win.present(&input).expect("PetWindow::present");
        let (bgra, w, h) = win.last_frame_bgra();
        assert_eq!(bgra.len(), (w as usize) * (h as usize) * 4);
        assert_premultiplied_bgra_invariant(bgra);
        assert_eq!(win.window_rect(), win.anchor_rect());

        win.set_locked(true).expect("lock pet window");
        assert!(win.is_locked());
        let ex_locked = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_ne!(
            ex_locked & WS_EX_TRANSPARENT,
            0,
            "locked PetWindow must enable WS_EX_TRANSPARENT"
        );

        win.set_locked(false).expect("unlock pet window");
        assert!(!win.is_locked());
        let ex_relocked = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_eq!(ex_relocked & WS_EX_TRANSPARENT, 0);

        let _ = win.poll_events();
        win.invalidate_cache();
        drop(win);
    }
}
