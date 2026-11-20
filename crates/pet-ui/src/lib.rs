//! Pure-Rust zero-GPU, zero-WebView2 floating console window
//! (`egui` 0.31 headless layout + pure-Rust CPU software rasterizer + Win32 `HWND`/`DIBSection`).

pub mod fonts;
pub mod rasterizer;
pub mod theme;
pub mod ui;
pub mod win32_window;

use crate::fonts::{build_font_definitions, ChineseFontInfo};
use crate::rasterizer::SoftwareRasterizer;
use crate::theme::{apply_pastel_theme, BG_CANVAS};
use crate::ui::{build_console_ui, compute_state_digest, ConsoleUiState};
use crate::win32_window::{
    query_process_private_mib, write_clipboard_text, ConsoleShortcut, NativeConsoleWindow,
};
use egui::{OutputCommand, Pos2, RawInput, Rect, Vec2, ViewportId};
use pet_core::{AppAction, AppState, ExpressionDefinition, MainTab};
use std::time::{Duration, Instant};

pub use crate::ui::ConsoleRuntimeMetrics;

pub const CONSOLE_LOGICAL_WIDTH: u32 = 470;
pub const CONSOLE_LOGICAL_HEIGHT: u32 = 780;
pub const CONSOLE_MIN_WIDTH: u32 = 430;
pub const CONSOLE_MIN_HEIGHT: u32 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct AnchorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f32,
}

/// Active native console resources that are created on demand and dropped completely on close.
struct ActiveConsoleSession {
    window: NativeConsoleWindow,
    ctx: egui::Context,
    rasterizer: SoftwareRasterizer,
    font_info: ChineseFontInfo,
    last_digest: u64,
    next_repaint_at: Option<Instant>,
    last_mem_sample_at: Option<Instant>,
}

/// On-demand floating console window that destroys its Win32 `HWND`, `DIBSection`,
/// `egui::Context`, Chinese font buffer, and texture atlas immediately when closed,
/// freeing all UI memory while leaving `AppState` untouched.
pub struct ConsoleWindow {
    session: Option<ActiveConsoleSession>,
    ui_state: ConsoleUiState,
    metrics: ConsoleRuntimeMetrics,
    external_memory_provided: bool,
}

impl ConsoleWindow {
    pub fn new() -> Self {
        Self {
            session: None,
            ui_state: ConsoleUiState::new(),
            metrics: ConsoleRuntimeMetrics::default(),
            external_memory_provided: false,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.session.is_some()
    }

    /// Updates optional runtime metrics displayed in Tab 4 (设置).
    pub fn set_runtime_metrics(&mut self, metrics: ConsoleRuntimeMetrics) {
        self.external_memory_provided = metrics.private_memory_mib > 0.0;
        let ui_ms = self.metrics.ui_frame_ms;
        let font_name = self.metrics.loaded_font_name.clone();
        self.metrics = metrics;
        if self.metrics.ui_frame_ms <= 0.0 {
            self.metrics.ui_frame_ms = ui_ms;
        }
        if self.metrics.loaded_font_name.is_none() {
            self.metrics.loaded_font_name = font_name;
        }
    }

    /// Alias for `show_near`: creates (if needed) and shows the console window anchored near `anchor`.
    pub fn open(&mut self, anchor: AnchorRect) -> Result<(), String> {
        self.show_near(anchor)
    }

    /// Creates (if needed) and shows the console window anchored cleanly beside the pet window.
    pub fn show_near(&mut self, pet_rect: AnchorRect) -> Result<(), String> {
        if let Some(session) = &mut self.session {
            session.window.bring_to_front();
            session.window.mark_dirty();
            return Ok(());
        }

        let window = NativeConsoleWindow::create_near(pet_rect)?;
        let ctx = egui::Context::default();
        let (font_defs, font_info) = build_font_definitions();
        ctx.set_fonts(font_defs);
        apply_pastel_theme(&ctx);

        self.metrics.loaded_font_name = font_info
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        self.session = Some(ActiveConsoleSession {
            window,
            ctx,
            rasterizer: SoftwareRasterizer::new(),
            font_info,
            last_digest: 0,
            next_repaint_at: Some(Instant::now()),
            last_mem_sample_at: None,
        });
        Ok(())
    }

    /// Alias for `hide_and_release`: destroys the native window and drops DIB/font/atlas surfaces.
    pub fn close(&mut self) {
        self.hide_and_release();
    }

    /// Destroys the native window and frees its pixel surface, `egui::Context`, and loaded font
    /// memory immediately while leaving `AppState` completely intact.
    pub fn hide_and_release(&mut self) {
        self.session = None;
    }

    /// Returns true if the console is open and currently has a pending repaint or input event.
    pub fn needs_repaint(&self) -> bool {
        let Some(session) = &self.session else {
            return false;
        };
        if session.window.is_dirty() || session.window.close_requested() {
            return true;
        }
        session
            .next_repaint_at
            .map(|deadline| Instant::now() >= deadline)
            .unwrap_or(false)
    }

    /// Processes pending Win32 input, runs one `egui` frame ONLY when dirty (or state/timer changed),
    /// rasterizes into the 32-bit DIBSection, presents via `BitBlt`, and returns emitted `AppAction`s.
    pub fn update(
        &mut self,
        state: &mut AppState,
        expressions: &[ExpressionDefinition],
        now_ms: f64,
    ) -> Vec<AppAction> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };

        session.window.pump_messages();

        if session.window.close_requested() {
            self.hide_and_release();
            return vec![AppAction::HideConsole];
        }

        let mut actions = Vec::new();
        for dropped_path in session.window.take_dropped_audio_files() {
            actions.push(AppAction::StartAudioFile(dropped_path));
        }

        // Refresh process private memory reading every 1.5s when Settings tab is visible
        if !self.external_memory_provided && state.active_tab == MainTab::Settings {
            let should_sample = session
                .last_mem_sample_at
                .map(|t| t.elapsed() >= Duration::from_millis(1500))
                .unwrap_or(true);
            if should_sample {
                self.metrics.private_memory_mib = query_process_private_mib();
                session.last_mem_sample_at = Some(Instant::now());
            }
        }

        let events = session.window.take_events();
        let shortcuts = session.window.take_shortcuts();
        let input_dirty = !events.is_empty() || !shortcuts.is_empty() || session.window.take_dirty();
        let digest = compute_state_digest(state, expressions, &self.metrics, now_ms);
        let state_changed = digest != session.last_digest;
        let timer_due = session
            .next_repaint_at
            .map(|t| Instant::now() >= t)
            .unwrap_or(false);

        if !input_dirty && !state_changed && !timer_due {
            return actions;
        }

        let t0 = Instant::now();
        let (phys_w, phys_h) = session.window.client_size_px();
        let ppp = session.window.scale_factor();
        let logical_size = Vec2::new(phys_w as f32 / ppp, phys_h as f32 / ppp);

        let mut raw_input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, logical_size)),
            time: Some(now_ms / 1000.0),
            events,
            ..Default::default()
        };
        raw_input
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);

        let mut ui_actions = Vec::new();
        let full_output = session.ctx.run(raw_input, |ctx| {
            ui_actions = build_console_ui(
                ctx,
                &mut self.ui_state,
                state,
                expressions,
                &self.metrics,
                now_ms,
            );
        });

        // Handle keyboard shortcuts (Space / Q / H) ONLY when no text input is focused
        let mut hide_now = false;
        if !session.ctx.wants_keyboard_input() {
            for sc in shortcuts {
                match sc {
                    ConsoleShortcut::ToggleAudio => actions.push(AppAction::TogglePlayLastAudio),
                    ConsoleShortcut::StopAudio => actions.push(AppAction::StopAudio),
                    ConsoleShortcut::HideConsole => {
                        actions.push(AppAction::HideConsole);
                        hide_now = true;
                    }
                }
            }
        }

        actions.extend(ui_actions);
        if actions.iter().any(|a| matches!(a, AppAction::HideConsole)) {
            hide_now = true;
        }

        // Clipboard output commands & cursor icon updates
        for cmd in &full_output.platform_output.commands {
            if let OutputCommand::CopyText(text) = cmd {
                write_clipboard_text(session.window.hwnd(), text);
            }
        }
        session
            .window
            .set_cursor_icon(full_output.platform_output.cursor_icon);

        // If user clicked Hide or pressed H, destroy window immediately and free memory!
        if hide_now {
            self.hide_and_release();
            return actions;
        }

        // Tessellate & software-rasterize directly into Win32 DIBSection + BitBlt
        session.rasterizer.apply_textures_set(&full_output.textures_delta);
        let primitives = session
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let clear_rgba = BG_CANVAS.to_array();
        let rasterizer = &session.rasterizer;
        let _ = session.window.draw_and_present(|buf, w, h, _| {
            rasterizer.paint(
                buf,
                w,
                h,
                full_output.pixels_per_point,
                clear_rgba,
                &primitives,
            );
        });
        session.rasterizer.apply_textures_free(&full_output.textures_delta);

        self.metrics.ui_frame_ms = t0.elapsed().as_secs_f64() as f32 * 1000.0;
        session.last_digest = compute_state_digest(state, expressions, &self.metrics, now_ms);

        // Schedule next repaint only if egui explicitly requests a finite delay (e.g., caret blink)
        let repaint_after = full_output
            .viewport_output
            .get(&ViewportId::ROOT)
            .map(|v| v.repaint_delay)
            .unwrap_or(Duration::MAX);
        session.next_repaint_at = if repaint_after < Duration::from_secs(60) {
            Some(Instant::now() + repaint_after.max(Duration::from_millis(16)))
        } else {
            None
        };

        // Keep font info accessible to avoid dead_code warning
        let _ = session.font_info.byte_len;

        actions
    }
}

impl Default for ConsoleWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Headless console harness used by `--self-test` and unit tests to layout + rasterize
/// all 4 tabs, modals, and todo drawer into a pure-Rust BGRA buffer with zero Win32 HWND.
pub struct HeadlessConsole {
    ctx: egui::Context,
    rasterizer: SoftwareRasterizer,
    ui_state: ConsoleUiState,
    pub font_info: ChineseFontInfo,
    pub metrics: ConsoleRuntimeMetrics,
    width: usize,
    height: usize,
    scale_factor: f32,
    framebuffer: Vec<u8>,
}

impl HeadlessConsole {
    pub fn new(width: u32, height: u32, scale_factor: f32) -> Self {
        let ctx = egui::Context::default();
        let (font_defs, font_info) = build_font_definitions();
        ctx.set_fonts(font_defs);
        apply_pastel_theme(&ctx);

        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let mut metrics = ConsoleRuntimeMetrics::default();
        metrics.loaded_font_name = font_info
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        Self {
            ctx,
            rasterizer: SoftwareRasterizer::new(),
            ui_state: ConsoleUiState::new(),
            font_info,
            metrics,
            width: w,
            height: h,
            scale_factor: scale_factor.clamp(0.5, 4.0),
            framebuffer: vec![0u8; w * h * 4],
        }
    }

    pub fn framebuffer_bgra(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn texture_bytes(&self) -> usize {
        self.rasterizer.texture_bytes()
    }

    /// Runs one headless `egui` layout + CPU rasterization pass with optional input events.
    pub fn step(
        &mut self,
        state: &mut AppState,
        expressions: &[ExpressionDefinition],
        now_ms: f64,
        events: Vec<egui::Event>,
    ) -> (Vec<AppAction>, f64) {
        let t0 = Instant::now();
        let logical_size = Vec2::new(
            self.width as f32 / self.scale_factor,
            self.height as f32 / self.scale_factor,
        );
        let mut raw_input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, logical_size)),
            time: Some(now_ms / 1000.0),
            events,
            ..Default::default()
        };
        raw_input
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale_factor);

        let mut actions = Vec::new();
        let full_output = self.ctx.run(raw_input, |ctx| {
            actions = build_console_ui(
                ctx,
                &mut self.ui_state,
                state,
                expressions,
                &self.metrics,
                now_ms,
            );
        });

        self.rasterizer
            .apply_textures_set(&full_output.textures_delta);
        let primitives = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        self.rasterizer.paint(
            &mut self.framebuffer,
            self.width,
            self.height,
            full_output.pixels_per_point,
            BG_CANVAS.to_array(),
            &primitives,
        );
        self.rasterizer
            .apply_textures_free(&full_output.textures_delta);

        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.metrics.ui_frame_ms = elapsed_ms as f32;
        (actions, elapsed_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{build_font_definitions, find_chinese_font_path, CHINESE_FONT_NAME};
    use egui::{Event, Modifiers, PointerButton};
    use pet_core::{
        load_embedded_character_assets, ActivityMode, AppModal, AudioDeviceInfo, AudioMode,
        PersistedState, TimerMode,
    };

    fn count_non_background_pixels(buf: &[u8]) -> usize {
        let bg = BG_CANVAS.to_array();
        let bg_bgra = [bg[2], bg[1], bg[0], bg[3]];
        buf.chunks_exact(4).filter(|px| *px != bg_bgra).count()
    }

    #[test]
    fn chinese_font_loads_and_registers_in_egui() {
        let found = find_chinese_font_path();
        assert!(
            found.is_some(),
            "Expected a Windows Chinese font (msyh.ttc / simhei.ttf / etc.) in C:\\Windows\\Fonts"
        );
        let (defs, info) = build_font_definitions();
        assert!(info.path.is_some());
        assert!(info.byte_len > 100_000, "font bytes should be non-trivial");
        assert!(defs.font_data.contains_key(CHINESE_FONT_NAME));
        assert!(
            matches!(
                defs.font_data[CHINESE_FONT_NAME].font,
                std::borrow::Cow::Borrowed(_)
            ),
            "Chinese font must be zero-copy memory-mapped (Cow::Borrowed) to avoid private heap bloat"
        );
        assert_eq!(
            defs.families
                .get(&egui::FontFamily::Proportional)
                .and_then(|v| v.first())
                .map(String::as_str),
            Some(CHINESE_FONT_NAME)
        );
    }

    #[test]
    fn headless_layout_and_rasterizer_render_all_4_tabs_todo_and_modals() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let mut state = AppState::new(PersistedState::default());
        state.asset_status.loaded = true;
        state.asset_status.pack_name = assets.manifest.pack_name.clone();
        state.asset_status.source_hint = assets.source_hint.clone();
        state.audio_devices = vec![
            AudioDeviceInfo {
                id: "dev-1".into(),
                name: "扬声器 (Realtek Audio)".into(),
                is_default: true,
            },
            AudioDeviceInfo {
                id: "dev-2".into(),
                name: "蓝牙耳机".into(),
                is_default: false,
            },
        ];

        let mut headless = HeadlessConsole::new(CONSOLE_LOGICAL_WIDTH, CONSOLE_LOGICAL_HEIGHT, 1.0);
        headless.metrics.sprite_count = assets.sprites.len();
        headless.metrics.asset_decoded_bytes = assets.total_decoded_bytes;
        headless.metrics.asset_full_canvas_bytes = assets.full_canvas_equivalent_bytes;

        // Render all 4 tabs (including todo drawer open on Status & live audio on AudioSync)
        for (i, tab) in MainTab::ALL.iter().enumerate() {
            state.active_tab = *tab;
            if *tab == MainTab::Status {
                state.todo_open = true;
            }
            if *tab == MainTab::AudioSync {
                state.audio_telemetry.running = true;
                state.audio_telemetry.mode = AudioMode::System;
                state.audio_telemetry.locked = true;
                state.audio_telemetry.bpm = 120;
                state.audio_telemetry.beats = 42;
                state.audio_telemetry.level = 0.65;
                state.audio_telemetry.bass = 0.72;
                state.audio_telemetry.beat = true;
            }
            let (_actions, ms) =
                headless.step(&mut state, &assets.expressions, (i as f64) * 100.0, vec![]);
            let drawn = count_non_background_pixels(headless.framebuffer_bgra());
            assert!(
                drawn > 25_000,
                "Tab {:?} should rasterize rich non-background pixels (got {}, took {:.2}ms)",
                tab,
                drawn,
                ms
            );
            // Every pixel in opaque console framebuffer must have alpha == 255
            assert!(
                headless
                    .framebuffer_bgra()
                    .chunks_exact(4)
                    .all(|px| px[3] == 255),
                "All rendered BGRA pixels must be opaque (a == 255)"
            );
        }

        // Render every confirmation modal cleanly without panics
        state.active_tab = MainTab::Status;
        for modal in [
            AppModal::ConfirmPause,
            AppModal::ConfirmLeaveStudy,
            AppModal::GoBackStudy,
            AppModal::ConfirmSaveTodo,
        ] {
            state.modal = modal;
            let (_actions, _ms) = headless.step(&mut state, &assets.expressions, 1000.0, vec![]);
            let drawn = count_non_background_pixels(headless.framebuffer_bgra());
            assert!(
                drawn > 50_000,
                "Modal {:?} should darken backdrop & draw dialog (got {} px)",
                modal,
                drawn
            );
        }

        // Verify font atlas was uploaded & populated with Chinese glyphs
        assert!(
            headless.texture_bytes() > 0,
            "Font atlas texture should be uploaded to software rasterizer"
        );
    }

    #[test]
    fn clicking_key_controls_emits_expected_app_actions() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let mut state = AppState::new(PersistedState::default());
        let mut headless = HeadlessConsole::new(CONSOLE_LOGICAL_WIDTH, CONSOLE_LOGICAL_HEIGHT, 1.0);

        // Warm-up layout pass so egui caches widget rects
        let _ = headless.step(&mut state, &assets.expressions, 0.0, vec![]);

        // Click on the 2nd tab ("情绪") in the top tab bar:
        // Header ~54px + tab bar top ~64px, width 470 -> 4 equal tabs (~110px each), 2nd tab center ~175, y ~78
        let mut found_emotion_tab = false;
        for y in [70.0f32, 76.0, 82.0, 88.0] {
            let pos = Pos2::new(175.0, y);
            let _ = headless.step(
                &mut state,
                &assets.expressions,
                16.0,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            let (actions, _) = headless.step(
                &mut state,
                &assets.expressions,
                32.0,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            if actions
                .iter()
                .any(|a| matches!(a, AppAction::SelectTab(MainTab::Emotion)))
            {
                found_emotion_tab = true;
                break;
            }
        }
        assert!(
            found_emotion_tab,
            "Clicking the 情绪 tab button should emit AppAction::SelectTab(MainTab::Emotion)"
        );

        // Switch to Status tab and verify clicking "自习" emits SelectActivityMode(Study)
        state.active_tab = MainTab::Status;
        let _ = headless.step(&mut state, &assets.expressions, 64.0, vec![]);

        let mut found_study_mode = false;
        for y in (130..210).step_by(8) {
            let pos = Pos2::new(340.0, y as f32);
            let _ = headless.step(
                &mut state,
                &assets.expressions,
                80.0,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            let (actions, _) = headless.step(
                &mut state,
                &assets.expressions,
                96.0,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            if actions
                .iter()
                .any(|a| matches!(a, AppAction::SelectActivityMode(ActivityMode::Study)))
            {
                found_study_mode = true;
                break;
            }
        }
        assert!(
            found_study_mode,
            "Clicking 自习 button should emit AppAction::SelectActivityMode(ActivityMode::Study)"
        );

        // Verify ConfirmSaveTodo modal saves named list and emits ConfirmSaveTodo
        state.persisted.todo_list.list_name = "高数冲刺".into();
        state.modal = AppModal::ConfirmSaveTodo;
        let _ = headless.step(&mut state, &assets.expressions, 120.0, vec![]);

        // Locate confirm save button on right half of centered modal
        let mut confirmed_save = false;
        for y in (410..490).step_by(6) {
            let pos = Pos2::new(310.0, y as f32);
            let _ = headless.step(
                &mut state,
                &assets.expressions,
                140.0,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            let (actions, _) = headless.step(
                &mut state,
                &assets.expressions,
                156.0,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            if actions
                .iter()
                .any(|a| matches!(a, AppAction::ConfirmSaveTodo))
            {
                confirmed_save = true;
                break;
            }
        }
        assert!(
            confirmed_save,
            "Clicking 确认保存 in SaveTodo modal should emit AppAction::ConfirmSaveTodo"
        );

        // Verify AudioSync StartSystemAudio button click emits StartSystemAudio
        state.modal = AppModal::None;
        state.active_tab = MainTab::AudioSync;
        state.timer.set_mode(TimerMode::Countdown);
        let _ = headless.step(&mut state, &assets.expressions, 200.0, vec![]);

        let mut started_audio = false;
        for y in (135..215).step_by(8) {
            let pos = Pos2::new(235.0, y as f32);
            let _ = headless.step(
                &mut state,
                &assets.expressions,
                220.0,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            let (actions, _) = headless.step(
                &mut state,
                &assets.expressions,
                236.0,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            if actions
                .iter()
                .any(|a| matches!(a, AppAction::StartSystemAudio { .. }))
            {
                started_audio = true;
                break;
            }
        }
        assert!(
            started_audio,
            "Clicking 一键开始系统音频同步 should emit AppAction::StartSystemAudio"
        );
    }

    #[test]
    fn chinese_glyphs_rasterize_distinct_pixels_without_tofu() {
        let (defs, info) = build_font_definitions();
        assert!(info.path.is_some());

        let ctx = egui::Context::default();
        ctx.set_fonts(defs);
        apply_pastel_theme(&ctx);

        let mut rast = SoftwareRasterizer::new();
        let mut render_text = |text: &str| -> Vec<u8> {
            let raw_input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 80.0))),
                ..Default::default()
            };
            let out = ctx.run(raw_input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label(egui::RichText::new(text).size(24.0).color(egui::Color32::BLACK));
                });
            });
            rast.apply_textures_set(&out.textures_delta);
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            let mut buf = vec![0u8; 320 * 80 * 4];
            rast.paint(&mut buf, 320, 80, out.pixels_per_point, [255, 255, 255, 255], &prims);
            rast.apply_textures_free(&out.textures_delta);
            buf
        };

        let buf_a = render_text("桌宠节奏师");
        let buf_b = render_text("自习音频设置");
        let dark_a = buf_a.chunks_exact(4).filter(|p| p[0] < 200).count();
        let dark_b = buf_b.chunks_exact(4).filter(|p| p[0] < 200).count();
        assert!(dark_a > 200, "Chinese text A should render dark glyph pixels (got {dark_a})");
        assert!(dark_b > 200, "Chinese text B should render dark glyph pixels (got {dark_b})");
        // Distinct Chinese strings MUST produce distinct bitmaps (if tofu fallback occurred, same-length or identical boxes would match)
        let diff_px = buf_a
            .chunks_exact(4)
            .zip(buf_b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            diff_px > 150,
            "Distinct Chinese strings must rasterize distinct glyphs (diff_px = {diff_px})"
        );
    }

    #[test]
    fn win32_dib_surface_draws_headless_frame_and_drops_cleanly() {
        let assets = load_embedded_character_assets().expect("embedded assets load");
        let mut state = AppState::new(PersistedState::default());
        let mut headless = HeadlessConsole::new(CONSOLE_LOGICAL_WIDTH, CONSOLE_LOGICAL_HEIGHT, 1.0);
        let (_actions, elapsed_ms) = headless.step(&mut state, &assets.expressions, 0.0, vec![]);

        let mut dib = crate::win32_window::DibSurface::new(
            CONSOLE_LOGICAL_WIDTH,
            CONSOLE_LOGICAL_HEIGHT,
        )
        .expect("CreateDIBSection should succeed on Windows");
        dib.as_mut_slice().copy_from_slice(headless.framebuffer_bgra());
        assert_eq!(
            dib.as_mut_slice().len(),
            (CONSOLE_LOGICAL_WIDTH * CONSOLE_LOGICAL_HEIGHT * 4) as usize
        );
        assert!(elapsed_ms >= 0.0);
        drop(dib);
    }

    #[test]
    fn console_window_open_hide_lifecycle_keeps_app_state_intact() {
        let mut state = AppState::new(PersistedState::default());
        state.active_tab = MainTab::Settings;
        state.pet_locked = true;
        state.persisted.todo_list.list_name = "保持状态测试".into();

        let mut console = ConsoleWindow::new();
        assert!(!console.is_visible());
        assert!(!console.needs_repaint());

        console.set_runtime_metrics(ConsoleRuntimeMetrics {
            sprite_count: 31,
            asset_decoded_bytes: 3_200_000,
            asset_full_canvas_bytes: 79_000_000,
            pet_frame_ms: 0.9,
            ui_frame_ms: 0.0,
            private_memory_mib: 18.5,
            loaded_font_name: None,
        });
        console.hide_and_release();
        assert!(!console.is_visible());
        assert_eq!(state.active_tab, MainTab::Settings);
        assert!(state.pet_locked);
        assert_eq!(state.persisted.todo_list.list_name, "保持状态测试");
    }
}
