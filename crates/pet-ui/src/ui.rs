use crate::theme::{self, *};
use egui::{Align, Align2, Color32, CornerRadius, Id, Layout, Margin, RichText, SliderClamping, Stroke, StrokeKind, Ui, Vec2};
use pet_core::{
    ActivityMode, AppAction, AppModal, AppState, AudioMode, EventContextKey, ExpressionDefinition,
    ExpressionType, MainTab, RandomEventType, TimerMode, TimerState, TodoItem, TodoList,
    DEFAULT_SENSITIVITY, MAX_SENSITIVITY, MIN_SENSITIVITY,
};
use std::path::PathBuf;

/// Optional runtime performance & asset memory telemetry fed from `pet-app` into Tab 4 (设置).
#[derive(Debug, Clone, Default)]
pub struct ConsoleRuntimeMetrics {
    pub sprite_count: usize,
    pub asset_decoded_bytes: usize,
    pub asset_full_canvas_bytes: usize,
    pub pet_frame_ms: f32,
    pub ui_frame_ms: f32,
    pub private_memory_mib: f32,
    pub loaded_font_name: Option<String>,
}

/// Ephemeral UI-only state (does NOT duplicate business state in `AppState`).
pub struct ConsoleUiState {
    custom_minutes_text: String,
    last_synced_minutes: u32,
    advanced_audio_open: bool,
    saved_todo_lists: Vec<TodoList>,
    saved_lists_loaded: bool,
    saved_lists_path: Option<PathBuf>,
    last_bar_peaks: [f32; 24],
}

impl ConsoleUiState {
    pub fn new() -> Self {
        Self {
            custom_minutes_text: String::new(),
            last_synced_minutes: 0,
            advanced_audio_open: false,
            saved_todo_lists: Vec::new(),
            saved_lists_loaded: false,
            saved_lists_path: if cfg!(test) {
                None
            } else {
                Some(pet_core::resolve_data_dir().join("saved_todo_lists.tsv"))
            },
            last_bar_peaks: [0.0; 24],
        }
    }

    fn ensure_saved_lists_loaded(&mut self) {
        if self.saved_lists_loaded {
            return;
        }
        self.saved_lists_loaded = true;
        if let Some(path) = &self.saved_lists_path {
            if let Ok(raw) = std::fs::read_to_string(path) {
                self.saved_todo_lists = parse_saved_todo_lists(&raw);
            }
        }
    }

    fn persist_saved_lists(&self) {
        if let Some(path) = &self.saved_lists_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let data = serialize_saved_todo_lists(&self.saved_todo_lists);
            let _ = std::fs::write(path, data);
        }
    }

    pub fn save_named_todo_snapshot(&mut self, list: &TodoList) {
        self.ensure_saved_lists_loaded();
        let mut snapshot = list.clone();
        snapshot.sanitize();
        let name = snapshot.list_name.trim();
        if name.is_empty() {
            snapshot.list_name = format!("清单 #{}", self.saved_todo_lists.len() + 1);
        } else {
            snapshot.list_name = name.to_string();
        }
        if let Some(existing) = self
            .saved_todo_lists
            .iter_mut()
            .find(|l| l.list_name == snapshot.list_name)
        {
            *existing = snapshot;
        } else {
            self.saved_todo_lists.insert(0, snapshot);
            self.saved_todo_lists.truncate(12);
        }
        self.persist_saved_lists();
    }
}

impl Default for ConsoleUiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders the entire 4-tab console UI for one `egui` frame and collects emitted `AppAction`s.
pub fn build_console_ui(
    ctx: &egui::Context,
    ui_state: &mut ConsoleUiState,
    state: &mut AppState,
    expressions: &[ExpressionDefinition],
    metrics: &ConsoleRuntimeMetrics,
    now_ms: f64,
) -> Vec<AppAction> {
    ui_state.ensure_saved_lists_loaded();
    if ui_state.last_synced_minutes != state.timer.study_minutes {
        ui_state.last_synced_minutes = state.timer.study_minutes;
        ui_state.custom_minutes_text = state.timer.study_minutes.to_string();
    }

    let mut actions = Vec::new();
    let modal_active = state.modal != AppModal::None;

    // Top pastel header + 4-tab navigation bar
    egui::TopBottomPanel::top("console_top_panel")
        .frame(
            egui::Frame::new()
                .fill(BG_HEADER)
                .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
                .inner_margin(Margin::symmetric(14, 10)),
        )
        .show(ctx, |ui| {
            ui.add_enabled_ui(!modal_active, |ui| {
                render_header(ui, state, &mut actions);
                ui.add_space(8.0);
                render_tab_bar(ui, state, &mut actions);
            });
        });

    // Main scrollable content area
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(BG_CANVAS)
                .inner_margin(Margin::symmetric(14, 12)),
        )
        .show(ctx, |ui| {
            ui.add_enabled_ui(!modal_active, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match state.active_tab {
                        MainTab::Status => render_status_tab(ui, ui_state, state, now_ms, &mut actions),
                        MainTab::Emotion => render_emotion_tab(ui, state, expressions, &mut actions),
                        MainTab::AudioSync => render_audio_tab(ui, ui_state, state, &mut actions),
                        MainTab::Settings => render_settings_tab(ui, state, metrics, &mut actions),
                    });
            });
        });

    // Frosted confirmation modal overlay (when active)
    if modal_active {
        render_modal_overlay(ctx, ui_state, state, &mut actions);
    }

    actions
}

fn render_header(ui: &mut Ui, state: &AppState, actions: &mut Vec<AppAction>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("桌宠节奏师")
                        .size(16.5)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                let (status_text, bg, fg) = if state.audio_telemetry.running {
                    if state.audio_telemetry.locked {
                        ("节拍已锁定", ACCENT_MINT_SOFT, ACCENT_MINT)
                    } else {
                        ("音频同步中", ACCENT_PURPLE_SOFT, ACCENT_PURPLE)
                    }
                } else if state.activity_mode == ActivityMode::Study {
                    ("自习模式", ACCENT_PINK_SOFT, ACCENT_PINK_DEEP)
                } else {
                    ("休闲模式", BG_CARD, TEXT_SECONDARY)
                };
                theme::badge(ui, status_text, bg, fg);
                if state.pet_locked {
                    theme::badge(ui, "已锁定", ACCENT_AMBER_SOFT, ACCENT_AMBER);
                }
            });
            ui.label(
                RichText::new("纯原生 Rust 控制台 · 零 GPU 零 WebView2")
                    .size(11.0)
                    .color(TEXT_SECONDARY),
            );
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::secondary_button(ui, "隐藏", Vec2::new(48.0, 26.0))
                .on_hover_text("隐藏并释放控制台内存 (H)")
                .clicked()
            {
                actions.push(AppAction::HideConsole);
            }
            if theme::secondary_button(ui, "重载素材", Vec2::new(70.0, 26.0))
                .on_hover_text("热重载外置画手素材")
                .clicked()
            {
                actions.push(AppAction::ReloadAssets);
            }
        });
    });
}

fn render_tab_bar(ui: &mut Ui, state: &AppState, actions: &mut Vec<AppAction>) {
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            let count = MainTab::ALL.len() as f32;
            let total_w = ui.available_width();
            let spacing = 4.0;
            let btn_w = ((total_w - spacing * (count - 1.0)) / count).max(60.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                for tab in MainTab::ALL {
                    let active = state.active_tab == tab;
                    let fill = if active { ACCENT_PINK } else { Color32::TRANSPARENT };
                    let text_col = if active { Color32::WHITE } else { TEXT_SECONDARY };
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(tab.label_zh())
                                .size(13.0)
                                .strong()
                                .color(text_col),
                        )
                        .fill(fill)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(9))
                        .min_size(Vec2::new(btn_w, 30.0)),
                    );
                    if resp.clicked() && !active {
                        actions.push(AppAction::SelectTab(tab));
                    }
                }
            });
        });
}

// ============================================================================
// Tab 1 — 状态 (Status): Activity Mode + Bidirectional Study Timer + Todo Drawer
// ============================================================================

fn render_status_tab(
    ui: &mut Ui,
    ui_state: &mut ConsoleUiState,
    state: &mut AppState,
    now_ms: f64,
    actions: &mut Vec<AppAction>,
) {
    // Card 1: Activity Mode (休闲 / 自习)
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        theme::section_title(ui, "当前状态", Some("切换桌宠桌面的休闲与自习场景"));
        ui.add_space(8.0);
        let btn_w = ((ui.available_width() - 8.0) * 0.5).max(100.0);
        ui.horizontal(|ui| {
            if theme::pill_button(
                ui,
                "☕ 休闲",
                state.activity_mode == ActivityMode::Chill,
                ACCENT_PURPLE,
                Vec2::new(btn_w, 36.0),
            )
            .clicked()
            {
                actions.push(AppAction::SelectActivityMode(ActivityMode::Chill));
            }
            if theme::pill_button(
                ui,
                "📖 自习",
                state.activity_mode == ActivityMode::Study,
                ACCENT_PINK,
                Vec2::new(btn_w, 36.0),
            )
            .clicked()
            {
                actions.push(AppAction::SelectActivityMode(ActivityMode::Study));
            }
        });
    });

    ui.add_space(10.0);

    // Card 2: Bidirectional Study Timer
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::section_title(ui, "自习计时器", None);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (st_label, st_bg, st_fg) = match state.timer.state {
                    TimerState::Idle => ("待机", BG_SOFT, TEXT_SECONDARY),
                    TimerState::Running => ("专注中", ACCENT_PINK_SOFT, ACCENT_PINK_DEEP),
                    TimerState::Paused => ("已暂停", ACCENT_AMBER_SOFT, ACCENT_AMBER),
                    TimerState::Completed => ("已完成！", ACCENT_MINT_SOFT, ACCENT_MINT),
                };
                theme::badge(ui, st_label, st_bg, st_fg);
            });
        });

        ui.add_space(8.0);

        // Countdown / Stopwatch mode switcher (disabled while running or paused)
        let can_switch_mode = !matches!(state.timer.state, TimerState::Running | TimerState::Paused);
        let half_w = ((ui.available_width() - 8.0) * 0.5).max(90.0);
        ui.add_enabled_ui(can_switch_mode, |ui| {
            ui.horizontal(|ui| {
                if theme::pill_button(
                    ui,
                    "⏳ 倒计时",
                    state.timer.mode == TimerMode::Countdown,
                    ACCENT_PINK,
                    Vec2::new(half_w, 30.0),
                )
                .clicked()
                {
                    actions.push(AppAction::SetTimerMode(TimerMode::Countdown));
                }
                if theme::pill_button(
                    ui,
                    "⏱ 正计时",
                    state.timer.mode == TimerMode::Stopwatch,
                    ACCENT_PURPLE,
                    Vec2::new(half_w, 30.0),
                )
                .clicked()
                {
                    actions.push(AppAction::SetTimerMode(TimerMode::Stopwatch));
                }
            });
        });
        if !can_switch_mode {
            ui.label(
                RichText::new("计时进行或暂停时不可切换模式，重置后可切换")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
        }

        // Large monospace clock display
        ui.add_space(8.0);
        theme::soft_panel_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let clock_color = match state.timer.state {
                    TimerState::Running => ACCENT_PINK_DEEP,
                    TimerState::Completed => ACCENT_MINT,
                    TimerState::Paused => ACCENT_AMBER,
                    TimerState::Idle => TEXT_PRIMARY,
                };
                ui.label(
                    RichText::new(state.timer.format_display())
                        .monospace()
                        .size(44.0)
                        .strong()
                        .color(clock_color),
                );
                let mode_caption = match state.timer.mode {
                    TimerMode::Countdown => format!("目标专注时长 {} 分钟", state.timer.study_minutes),
                    TimerMode::Stopwatch => "正计时累计专注时间".to_string(),
                };
                ui.label(RichText::new(mode_caption).size(11.5).color(TEXT_SECONDARY));

                if state.timer.mode == TimerMode::Countdown {
                    ui.add_space(6.0);
                    let progress = state.timer.progress_ratio();
                    draw_pastel_progress_bar(ui, progress, ACCENT_PINK, 7.0);
                }
            });
        });

        // Presets + custom minutes input (Countdown mode)
        if state.timer.mode == TimerMode::Countdown {
            ui.add_space(8.0);
            ui.add_enabled_ui(can_switch_mode, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("常用时长").size(12.0).color(TEXT_SECONDARY));
                    for mins in [15u32, 25, 45, 60] {
                        let active = state.timer.study_minutes == mins;
                        if theme::pill_button(
                            ui,
                            &format!("{mins}分"),
                            active,
                            ACCENT_PINK,
                            Vec2::new(50.0, 26.0),
                        )
                        .clicked()
                        {
                            actions.push(AppAction::SetStudyMinutes(mins));
                        }
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("自定义分钟").size(12.0).color(TEXT_SECONDARY));
                    if theme::secondary_button(ui, "−", Vec2::new(26.0, 26.0)).clicked() {
                        let next = state.timer.study_minutes.saturating_sub(5).max(1);
                        actions.push(AppAction::SetStudyMinutes(next));
                    }
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut ui_state.custom_minutes_text)
                            .desired_width(58.0)
                            .hint_text("1~999"),
                    );
                    if theme::secondary_button(ui, "+", Vec2::new(26.0, 26.0)).clicked() {
                        let next = (state.timer.study_minutes + 5).min(999);
                        actions.push(AppAction::SetStudyMinutes(next));
                    }
                    let apply_clicked =
                        theme::secondary_button(ui, "应用", Vec2::new(48.0, 26.0)).clicked();
                    let enter_pressed =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if apply_clicked || enter_pressed {
                        if let Ok(parsed) = ui_state.custom_minutes_text.trim().parse::<u32>() {
                            actions.push(AppAction::SetStudyMinutes(parsed.clamp(1, 999)));
                        } else {
                            ui_state.custom_minutes_text = state.timer.study_minutes.to_string();
                        }
                    }
                });
            });
        }

        // Control buttons: 开始 / 暂停 / 继续 / 重置
        ui.add_space(10.0);
        let avail_w = ui.available_width();
        ui.horizontal(|ui| match state.timer.state {
            TimerState::Idle | TimerState::Completed => {
                let label = if state.timer.state == TimerState::Completed {
                    "▶ 再来一轮"
                } else {
                    "▶ 开始自习"
                };
                if theme::primary_button(ui, label, ACCENT_PINK, Vec2::new(avail_w, 36.0)).clicked()
                {
                    actions.push(AppAction::StartOrResumeTimer);
                }
            }
            TimerState::Running => {
                let w = ((avail_w - 8.0) * 0.5).max(80.0);
                if theme::primary_button(ui, "⏸ 暂停", ACCENT_AMBER, Vec2::new(w, 36.0)).clicked() {
                    actions.push(AppAction::RequestPauseTimer);
                }
                if theme::secondary_button(ui, "↺ 重置", Vec2::new(w, 36.0)).clicked() {
                    actions.push(AppAction::ResetTimer);
                }
            }
            TimerState::Paused => {
                let w = ((avail_w - 8.0) * 0.5).max(80.0);
                if theme::primary_button(ui, "▶ 继续", ACCENT_PINK, Vec2::new(w, 36.0)).clicked() {
                    actions.push(AppAction::StartOrResumeTimer);
                }
                if theme::secondary_button(ui, "↺ 重置", Vec2::new(w, 36.0)).clicked() {
                    actions.push(AppAction::ResetTimer);
                }
            }
        });
    });

    ui.add_space(10.0);

    // Card 3: Collapsible Todo Drawer (待办清单)
    render_todo_drawer(ui, ui_state, state, now_ms, actions);
}

fn render_todo_drawer(
    ui: &mut Ui,
    ui_state: &mut ConsoleUiState,
    state: &mut AppState,
    now_ms: f64,
    actions: &mut Vec<AppAction>,
) {
    let total = state.persisted.todo_list.items.len();
    let done = state
        .persisted
        .todo_list
        .items
        .iter()
        .filter(|i| i.done)
        .count();

    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let arrow = if state.todo_open { "▼" } else { "▶" };
            let header_btn = ui.add(
                egui::Button::new(
                    RichText::new(format!("{arrow} 待办清单"))
                        .size(14.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
            );
            if header_btn.clicked() {
                actions.push(AppAction::ToggleTodoDrawer);
            }

            theme::badge(
                ui,
                &format!("已完成 {done}/{total}"),
                if done == total && total > 0 {
                    ACCENT_MINT_SOFT
                } else {
                    ACCENT_PINK_SOFT
                },
                if done == total && total > 0 {
                    ACCENT_MINT
                } else {
                    ACCENT_PINK_DEEP
                },
            );

            if state.todo_saved_notice(now_ms) {
                theme::badge(ui, "已保存 ✓", ACCENT_MINT_SOFT, ACCENT_MINT);
            }
        });

        if !state.todo_open {
            return;
        }

        ui.add_space(8.0);

        // List name input
        let mut list_name = state.persisted.todo_list.list_name.clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new("清单名称").size(12.0).color(TEXT_SECONDARY));
            let resp = ui.add(
                egui::TextEdit::singleline(&mut list_name)
                    .desired_width(ui.available_width())
                    .hint_text("例如：今日自习任务（最多40字）"),
            );
            if resp.changed() {
                actions.push(AppAction::SetTodoListName(list_name));
            }
        });

        ui.add_space(6.0);

        // Todo items
        let can_delete = state.persisted.todo_list.items.len() > 1;
        let items_snapshot = state.persisted.todo_list.items.clone();
        for (idx, item) in items_snapshot.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut checked = item.done;
                if ui.checkbox(&mut checked, "").changed() {
                    actions.push(AppAction::ToggleTodoItem(item.id.clone()));
                }

                let mut text = item.text.clone();
                let edit_w = (ui.available_width() - 34.0).max(80.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .id_salt(("todo_item_input", &item.id))
                        .desired_width(edit_w)
                        .hint_text(format!("待办事项 {}...", idx + 1)),
                );
                if resp.changed() {
                    actions.push(AppAction::UpdateTodoItemText {
                        id: item.id.clone(),
                        text,
                    });
                }

                ui.add_enabled_ui(can_delete, |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(14.0).color(ACCENT_CORAL))
                                .fill(ACCENT_CORAL_SOFT)
                                .stroke(Stroke::NONE)
                                .corner_radius(CornerRadius::same(7))
                                .min_size(Vec2::new(24.0, 24.0)),
                        )
                        .on_hover_text("删除此项")
                        .clicked()
                    {
                        actions.push(AppAction::RemoveTodoItem(item.id.clone()));
                    }
                });
            });
        }

        ui.add_space(8.0);
        let half_w = ((ui.available_width() - 8.0) * 0.5).max(80.0);
        ui.horizontal(|ui| {
            if theme::secondary_button(ui, "+ 添加任务", Vec2::new(half_w, 30.0)).clicked() {
                actions.push(AppAction::AddTodoItem);
            }
            if theme::primary_button(ui, "💾 保存清单", ACCENT_PURPLE, Vec2::new(half_w, 30.0))
                .clicked()
            {
                actions.push(AppAction::RequestSaveTodo);
            }
        });

        // Saved named lists section (load & delete)
        if !ui_state.saved_todo_lists.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                RichText::new("已保存的清单（点击载入）")
                    .size(11.5)
                    .color(TEXT_SECONDARY),
            );
            ui.add_space(4.0);

            let mut load_idx: Option<usize> = None;
            let mut delete_idx: Option<usize> = None;
            ui.horizontal_wrapped(|ui| {
                for (idx, saved) in ui_state.saved_todo_lists.iter().enumerate() {
                    let title = if saved.list_name.trim().is_empty() {
                        format!("未命名清单 {}", idx + 1)
                    } else {
                        saved.list_name.clone()
                    };
                    egui::Frame::new()
                        .fill(ACCENT_PURPLE_SOFT)
                        .stroke(Stroke::new(1.0_f32, BORDER_STRONG))
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::symmetric(6, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(format!(
                                                "{title} ({})",
                                                saved.items.len()
                                            ))
                                            .size(11.5)
                                            .color(ACCENT_PURPLE),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE),
                                    )
                                    .clicked()
                                {
                                    load_idx = Some(idx);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("×").size(11.0).color(TEXT_SECONDARY),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE),
                                    )
                                    .on_hover_text("移除已保存清单")
                                    .clicked()
                                {
                                    delete_idx = Some(idx);
                                }
                            });
                        });
                }
            });

            if let Some(idx) = load_idx {
                if let Some(chosen) = ui_state.saved_todo_lists.get(idx).cloned() {
                    let name = chosen.list_name.clone();
                    state.persisted.todo_list = chosen;
                    state.persisted.todo_list.sanitize();
                    actions.push(AppAction::SetTodoListName(name));
                }
            }
            if let Some(idx) = delete_idx {
                if idx < ui_state.saved_todo_lists.len() {
                    ui_state.saved_todo_lists.remove(idx);
                    ui_state.persist_saved_lists();
                }
            }
        }
    });
}

// ============================================================================
// Tab 2 — 情绪 (Emotion): Dynamic Expressions + Random Controls + Event Matrix
// ============================================================================

fn render_emotion_tab(
    ui: &mut Ui,
    state: &AppState,
    expressions: &[ExpressionDefinition],
    actions: &mut Vec<AppAction>,
) {
    let active_id = state
        .effective_expression(expressions)
        .map(|e| e.id.as_str())
        .unwrap_or("neutral");

    // Card 1: Expression buttons grouped by 整体 (Base) & 配件 (Accessory)
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::section_title(ui, "表情切换", Some("再次点击当前表情可恢复自然"));
            if state.random_expression.is_some() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    theme::badge(ui, "随机触发中", ACCENT_PURPLE_SOFT, ACCENT_PURPLE);
                });
            }
        });

        let base_exprs: Vec<&ExpressionDefinition> = expressions
            .iter()
            .filter(|e| e.kind == ExpressionType::Base)
            .collect();
        let acc_exprs: Vec<&ExpressionDefinition> = expressions
            .iter()
            .filter(|e| e.kind == ExpressionType::Accessory)
            .collect();

        if !base_exprs.is_empty() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                theme::badge(ui, "整体", ACCENT_PINK_SOFT, ACCENT_PINK_DEEP);
                ui.label(
                    RichText::new("基础脸部表情与整体动画")
                        .size(11.5)
                        .color(TEXT_SECONDARY),
                );
            });
            ui.add_space(6.0);
            render_expression_grid(ui, &base_exprs, active_id, ACCENT_PINK, actions);
        }

        if !acc_exprs.is_empty() {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                theme::badge(ui, "配件", ACCENT_PURPLE_SOFT, ACCENT_PURPLE);
                ui.label(
                    RichText::new("保留底图表情并叠加特效配件")
                        .size(11.5)
                        .color(TEXT_SECONDARY),
                );
            });
            ui.add_space(6.0);
            render_expression_grid(ui, &acc_exprs, active_id, ACCENT_PURPLE, actions);
        }
    });

    ui.add_space(10.0);

    // Card 2: Random expression toggle + interval slider + per-event matrix
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        theme::section_title(ui, "随机情绪", Some("定时自动触发可爱小表情"));
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("开启自动随机表情").size(13.0).color(TEXT_PRIMARY));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let enabled = state.persisted.settings.random_enabled;
                if theme::toggle_switch(ui, enabled, true).clicked() {
                    actions.push(AppAction::SetRandomEnabled(!enabled));
                }
            });
        });

        ui.add_space(6.0);
        let mut interval_secs = state.persisted.settings.random_interval_seconds.clamp(15, 600);
        ui.add_enabled_ui(state.persisted.settings.random_enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("触发间隔").size(12.5).color(TEXT_SECONDARY));
                let slider = ui.add(
                    egui::Slider::new(&mut interval_secs, 15..=600)
                        .suffix(" 秒")
                        .clamping(SliderClamping::Always),
                );
                if slider.changed() {
                    actions.push(AppAction::SetRandomIntervalSeconds(interval_secs));
                }
            });
            let mins = interval_secs / 60;
            let rem = interval_secs % 60;
            let human = if mins > 0 {
                format!("约每 {mins} 分 {rem} 秒随机出现一次表情（范围 15~600 秒）")
            } else {
                format!("约每 {rem} 秒随机出现一次表情（范围 15~600 秒）")
            };
            ui.label(RichText::new(human).size(11.0).color(TEXT_MUTED));
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(
            RichText::new("分情景随机事件开关（思考 / 喜欢 / 震惊 / 打瞌睡）")
                .size(12.5)
                .strong()
                .color(TEXT_PRIMARY),
        );
        ui.label(
            RichText::new("音乐同步期间「打瞌睡」自动锁定关闭，防止跟着节拍睡着哦～")
                .size(11.0)
                .color(TEXT_SECONDARY),
        );
        ui.add_space(6.0);

        let ordered_events = [
            RandomEventType::Think,
            RandomEventType::Love,
            RandomEventType::Shock,
            RandomEventType::Doze,
        ];

        egui::Grid::new("event_switch_matrix_grid")
            .num_columns(4)
            .spacing(Vec2::new(10.0, 8.0))
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("事件").size(12.0).strong().color(TEXT_SECONDARY));
                for ctx_key in EventContextKey::ALL {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new(ctx_key.label_zh())
                                .size(12.0)
                                .strong()
                                .color(TEXT_SECONDARY),
                        );
                    });
                }
                ui.end_row();

                for ev in ordered_events {
                    ui.label(RichText::new(ev.label_zh()).size(12.5).color(TEXT_PRIMARY));
                    for ctx_key in EventContextKey::ALL {
                        let locked_off =
                            ctx_key == EventContextKey::Music && ev == RandomEventType::Doze;
                        let on = state.persisted.event_switches.get(ctx_key, ev);
                        ui.centered_and_justified(|ui| {
                            if locked_off {
                                theme::badge(ui, "禁眠", BG_SOFT, TEXT_MUTED);
                            } else {
                                let resp = theme::toggle_switch(ui, on, true);
                                if resp.clicked() {
                                    actions.push(AppAction::ToggleEventSwitch(ctx_key, ev));
                                }
                            }
                        });
                    }
                    ui.end_row();
                }
            });
    });
}

fn render_expression_grid(
    ui: &mut Ui,
    exprs: &[&ExpressionDefinition],
    active_id: &str,
    accent: Color32,
    actions: &mut Vec<AppAction>,
) {
    let cols = 3usize;
    let spacing = 8.0;
    let avail = ui.available_width();
    let btn_w = ((avail - spacing * (cols as f32 - 1.0)) / cols as f32).max(88.0);

    for chunk in exprs.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for expr in chunk {
                let is_active = expr.id == active_id;
                let tag = if expr.resources.animation.is_some() {
                    " ✦"
                } else {
                    ""
                };
                let label = format!("{}{}", expr.name, tag);
                let resp = theme::pill_button(ui, &label, is_active, accent, Vec2::new(btn_w, 34.0));
                if resp.clicked() {
                    actions.push(AppAction::SelectExpression(expr.id.clone()));
                }
            }
        });
        ui.add_space(6.0);
    }
}

// ============================================================================
// Tab 3 — 音频同步 (AudioSync): Loopback + BPM + 24-bar Meter + Devices + Adv
// ============================================================================

fn render_audio_tab(
    ui: &mut Ui,
    ui_state: &mut ConsoleUiState,
    state: &mut AppState,
    actions: &mut Vec<AppAction>,
) {
    let t = &state.audio_telemetry;

    // Card 1: Main control button + BPM + Beat count + 24-bar level meter + bass meter
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::section_title(ui, "实时节拍同步", None);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (badge_text, bg, fg) = if !t.running {
                    ("未启动", BG_SOFT, TEXT_SECONDARY)
                } else if t.locked {
                    ("已锁定", ACCENT_MINT_SOFT, ACCENT_MINT)
                } else {
                    ("搜索中…", ACCENT_AMBER_SOFT, ACCENT_AMBER)
                };
                theme::badge(ui, badge_text, bg, fg);

                if t.running {
                    let mode_name = match t.mode {
                        AudioMode::System => "系统回环",
                        AudioMode::File => "本地音频",
                        AudioMode::Demo => "演示鼓点",
                        AudioMode::Idle => "待机",
                    };
                    theme::badge(ui, mode_name, ACCENT_PURPLE_SOFT, ACCENT_PURPLE);
                }
            });
        });

        ui.add_space(10.0);

        // Prominent Start / Stop button
        let full_w = ui.available_width();
        if t.running {
            if theme::primary_button(ui, "■ 停止音频同步", ACCENT_CORAL, Vec2::new(full_w, 40.0))
                .clicked()
            {
                actions.push(AppAction::StopAudio);
            }
        } else {
            let label = if t.busy {
                "正在连接系统音频…"
            } else {
                "▶ 一键开始系统音频同步"
            };
            ui.add_enabled_ui(!t.busy, |ui| {
                if theme::primary_button(ui, label, ACCENT_PINK, Vec2::new(full_w, 40.0)).clicked()
                {
                    actions.push(AppAction::StartSystemAudio {
                        device_id: state.selected_device_id.clone(),
                    });
                }
            });
        }

        ui.add_space(10.0);

        // BPM & Beat Count readout panel
        theme::soft_panel_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("实时 BPM").size(11.5).color(TEXT_SECONDARY));
                    let bpm_text = if t.running && t.bpm > 0 {
                        format!("{}", t.bpm)
                    } else {
                        "--".to_string()
                    };
                    let bpm_col = if t.running && t.locked {
                        ACCENT_PINK_DEEP
                    } else {
                        TEXT_PRIMARY
                    };
                    ui.label(
                        RichText::new(bpm_text)
                            .monospace()
                            .size(34.0)
                            .strong()
                            .color(bpm_col),
                    );
                });

                ui.add_space(24.0);

                ui.vertical(|ui| {
                    ui.label(RichText::new("节拍计数").size(11.5).color(TEXT_SECONDARY));
                    ui.label(
                        RichText::new(format!("{}", t.beats))
                            .monospace()
                            .size(22.0)
                            .strong()
                            .color(ACCENT_PURPLE),
                    );
                    let hit_text = if t.beat {
                        "● 节拍命中"
                    } else if t.onset {
                        "○ 检测到起音"
                    } else {
                        "· 监听电平"
                    };
                    ui.label(
                        RichText::new(hit_text)
                            .size(11.0)
                            .color(if t.beat { ACCENT_PINK_DEEP } else { TEXT_MUTED }),
                    );
                });
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "实时电平（24 段） · {:3.0}%",
                    (t.level * 100.0).clamp(0.0, 100.0)
                ))
                .size(11.0)
                .color(TEXT_SECONDARY),
            );
            ui.add_space(4.0);
            draw_24_bar_level_meter(ui, &mut ui_state.last_bar_peaks, t.level, t.bass, t.beat, t.running);

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "低频能量 {:3.0}%",
                        (t.bass * 100.0).clamp(0.0, 100.0)
                    ))
                    .size(11.0)
                    .color(TEXT_SECONDARY),
                );
            });
            ui.add_space(3.0);
            draw_pastel_progress_bar(ui, t.bass.clamp(0.0, 1.0), ACCENT_PURPLE, 7.0);
        });

        // Drag-and-drop indicator & current file info
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(BG_INPUT)
            .stroke(Stroke::new(1.0_f32, BORDER_STRONG))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("🎵 拖入音频文件直接播放（MP3 / WAV / FLAC / OGG / M4A / AAC）")
                            .size(11.5)
                            .color(TEXT_SECONDARY),
                    );
                });
                let display_file = t.file_name.clone().or_else(|| {
                    state
                        .last_audio_file
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                });
                if let Some(fname) = display_file {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        theme::badge(ui, "当前文件", ACCENT_PINK_SOFT, ACCENT_PINK_DEEP);
                        ui.label(RichText::new(fname).size(12.0).strong().color(TEXT_PRIMARY));
                        if !t.running {
                            if let Some(path) = state.last_audio_file.clone() {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if theme::secondary_button(ui, "播放", Vec2::new(48.0, 24.0))
                                        .clicked()
                                    {
                                        actions.push(AppAction::StartAudioFile(path));
                                    }
                                });
                            }
                        }
                    });
                }
            });

        // Error or silence hint alert
        if let Some(err) = &t.error {
            ui.add_space(8.0);
            theme::alert_frame(ACCENT_CORAL_SOFT, ACCENT_CORAL).show(ui, |ui| {
                ui.label(
                    RichText::new(format!("⚠ 音频错误：{err}"))
                        .size(12.0)
                        .color(ACCENT_CORAL),
                );
            });
        } else if let Some(hint) = state.silence_hint_text() {
            ui.add_space(8.0);
            theme::alert_frame(ACCENT_AMBER_SOFT, ACCENT_AMBER).show(ui, |ui| {
                ui.label(RichText::new(format!("💡 {hint}")).size(12.0).color(ACCENT_AMBER));
            });
        }
    });

    ui.add_space(10.0);

    // Card 2: Output Device Selector
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        theme::section_title(ui, "系统输出设备", Some("选择 WASAPI 回环监听目标"));
        ui.add_space(8.0);

        let selected_label = match &state.selected_device_id {
            None => "系统默认输出设备".to_string(),
            Some(id) => state
                .audio_devices
                .iter()
                .find(|d| &d.id == id)
                .map(|d| {
                    if d.is_default {
                        format!("{}（默认）", d.name)
                    } else {
                        d.name.clone()
                    }
                })
                .unwrap_or_else(|| "系统默认输出设备".to_string()),
        };

        let mut newly_selected: Option<Option<String>> = None;
        egui::ComboBox::from_id_salt("audio_device_selector")
            .width(ui.available_width())
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(state.selected_device_id.is_none(), "系统默认输出设备")
                    .clicked()
                {
                    newly_selected = Some(None);
                }
                for dev in &state.audio_devices {
                    let is_sel = state.selected_device_id.as_deref() == Some(dev.id.as_str());
                    let label = if dev.is_default {
                        format!("{}（默认）", dev.name)
                    } else {
                        dev.name.clone()
                    };
                    if ui.selectable_label(is_sel, label).clicked() {
                        newly_selected = Some(Some(dev.id.clone()));
                    }
                }
            });

        if let Some(dev_opt) = newly_selected {
            state.selected_device_id = dev_opt.clone();
            if state.audio_telemetry.running && state.audio_telemetry.mode == AudioMode::System {
                actions.push(AppAction::StartSystemAudio { device_id: dev_opt });
            }
        }

        ui.add_space(8.0);
        let half_w = ((ui.available_width() - 8.0) * 0.5).max(100.0);
        ui.horizontal(|ui| {
            if theme::secondary_button(ui, "↻ 刷新设备", Vec2::new(half_w, 30.0)).clicked() {
                actions.push(AppAction::RefreshAudioDevices);
            }
            if theme::secondary_button(ui, "⚙ 打开系统声音设置", Vec2::new(half_w, 30.0)).clicked()
            {
                actions.push(AppAction::OpenSystemSoundSettings);
            }
        });
    });

    ui.add_space(10.0);

    // Card 3: Collapsible Advanced Settings (高级设置)
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let arrow = if ui_state.advanced_audio_open {
            "▼"
        } else {
            "▶"
        };
        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!("{arrow} 高级设置"))
                        .size(13.5)
                        .strong()
                        .color(TEXT_PRIMARY),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
            )
            .clicked()
        {
            ui_state.advanced_audio_open = !ui_state.advanced_audio_open;
        }

        if !ui_state.advanced_audio_open {
            return;
        }

        ui.add_space(8.0);

        // Sensitivity slider (1.05..=2.00, default 1.22)
        let mut sens = state.sensitivity.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY);
        ui.horizontal(|ui| {
            ui.label(RichText::new("节拍灵敏度").size(12.5).color(TEXT_PRIMARY));
            let slider = ui.add(
                egui::Slider::new(&mut sens, MIN_SENSITIVITY..=MAX_SENSITIVITY)
                    .fixed_decimals(2)
                    .step_by(0.01),
            );
            if slider.changed() {
                actions.push(AppAction::SetSensitivity(sens));
            }
            if theme::secondary_button(ui, "默认", Vec2::new(46.0, 24.0))
                .on_hover_text("恢复默认灵敏度 1.22")
                .clicked()
            {
                actions.push(AppAction::SetSensitivity(DEFAULT_SENSITIVITY));
            }
        });
        ui.label(
            RichText::new("数值越大越灵敏，数值越小越抗噪（推荐默认 1.22）")
                .size(11.0)
                .color(TEXT_MUTED),
        );

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("自动校准 BPM").size(12.5).color(TEXT_PRIMARY));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::toggle_switch(ui, state.auto_check_bpm, true).clicked() {
                    actions.push(AppAction::ToggleAutoCheckBpm);
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("自动跟随节拍点头").size(12.5).color(TEXT_PRIMARY));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::toggle_switch(ui, state.auto_follow, true).clicked() {
                    actions.push(AppAction::ToggleAutoFollow);
                }
            });
        });

        ui.add_space(8.0);
        let is_demo = t.running && t.mode == AudioMode::Demo;
        let demo_label = if is_demo {
            "■ 停止内置演示鼓点"
        } else {
            "🥁 启动演示鼓点 (112 BPM)"
        };
        let demo_fill = if is_demo { ACCENT_CORAL } else { ACCENT_PURPLE };
        if theme::primary_button(ui, demo_label, demo_fill, Vec2::new(ui.available_width(), 32.0))
            .clicked()
        {
            if is_demo {
                actions.push(AppAction::StopAudio);
            } else {
                actions.push(AppAction::StartDemoAudio);
            }
        }
    });
}

// ============================================================================
// Tab 4 — 设置 (Settings): Lock + End Sound + Hot Reload + Shortcuts + Perf
// ============================================================================

fn render_settings_tab(
    ui: &mut Ui,
    state: &AppState,
    metrics: &ConsoleRuntimeMetrics,
    actions: &mut Vec<AppAction>,
) {
    // Card 1: Pet Lock & Timer Chime
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        theme::section_title(ui, "常规设置", None);
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("桌宠锁定（点击穿透）")
                            .size(13.0)
                            .color(TEXT_PRIMARY),
                    );
                    theme::badge(ui, "Ctrl+Alt+L", BG_SOFT, TEXT_SECONDARY);
                });
                ui.label(
                    RichText::new("开启后鼠标点击将直接穿透桌宠窗口")
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::toggle_switch(ui, state.pet_locked, true).clicked() {
                    actions.push(AppAction::SetLocked(!state.pet_locked));
                }
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("倒计时完成提示音")
                        .size(13.0)
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new("自习倒计时归零时播放轻柔三音铃声")
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let enabled = state.persisted.settings.end_sound_enabled;
                if theme::toggle_switch(ui, enabled, true).clicked() {
                    actions.push(AppAction::SetEndSoundEnabled(!enabled));
                }
            });
        });
    });

    ui.add_space(10.0);

    // Card 2: External Artist Asset Pack & Hot Reload
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::section_title(ui, "角色素材包", Some("支持外置画手素材热重载"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if state.asset_status.error.is_some() {
                    theme::badge(ui, "加载异常", ACCENT_CORAL_SOFT, ACCENT_CORAL);
                } else if state.asset_status.loaded {
                    theme::badge(ui, "已加载", ACCENT_MINT_SOFT, ACCENT_MINT);
                } else {
                    theme::badge(ui, "未加载", BG_SOFT, TEXT_SECONDARY);
                }
            });
        });

        ui.add_space(8.0);
        theme::soft_panel_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let pack = if state.asset_status.pack_name.is_empty() {
                "默认角色包"
            } else {
                state.asset_status.pack_name.as_str()
            };
            let source = if state.asset_status.source_hint.is_empty() {
                "内置默认素材包"
            } else {
                state.asset_status.source_hint.as_str()
            };

            ui.label(RichText::new(format!("素材名称：{pack}")).size(12.0).color(TEXT_PRIMARY));
            ui.label(
                RichText::new(format!("加载来源：{source}"))
                    .size(11.5)
                    .color(TEXT_SECONDARY),
            );

            if metrics.sprite_count > 0 || metrics.asset_decoded_bytes > 0 {
                let decoded_mib = metrics.asset_decoded_bytes as f64 / (1024.0 * 1024.0);
                let full_mib = metrics.asset_full_canvas_bytes as f64 / (1024.0 * 1024.0);
                let saved_pct = if metrics.asset_full_canvas_bytes > 0 {
                    (1.0 - (metrics.asset_decoded_bytes as f64
                        / metrics.asset_full_canvas_bytes as f64))
                        * 100.0
                } else {
                    0.0
                };
                ui.label(
                    RichText::new(format!(
                        "图层数量：{} 个 · 紧致裁剪 {:.1} MiB（原画布 {:.1} MiB，节省 {:.0}%）",
                        metrics.sprite_count, decoded_mib, full_mib, saved_pct
                    ))
                    .size(11.5)
                    .color(ACCENT_PURPLE),
                );
            } else {
                ui.label(
                    RichText::new("紧致边界盒预乘裁剪已启用（大幅降低内存占用）")
                        .size(11.5)
                        .color(TEXT_MUTED),
                );
            }
        });

        if let Some(err) = &state.asset_status.error {
            ui.add_space(6.0);
            theme::alert_frame(ACCENT_CORAL_SOFT, ACCENT_CORAL).show(ui, |ui| {
                ui.label(
                    RichText::new(format!("⚠ 素材加载失败（已保留旧素材）：{err}"))
                        .size(11.5)
                        .color(ACCENT_CORAL),
                );
            });
        }

        ui.add_space(8.0);
        if theme::primary_button(
            ui,
            "🔄 热重载外置素材",
            ACCENT_PINK,
            Vec2::new(ui.available_width(), 34.0),
        )
        .clicked()
        {
            actions.push(AppAction::ReloadAssets);
        }
    });

    ui.add_space(10.0);

    // Card 3: Keyboard Shortcuts Cheatsheet
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        theme::section_title(ui, "快捷键速查", None);
        ui.add_space(8.0);

        let shortcuts = [
            ("Ctrl + Alt + C", "全局打开 / 隐藏控制台"),
            ("Ctrl + Alt + L", "全局锁定 / 解锁桌宠（点击穿透）"),
            ("Space", "开始 / 停止音频同步（未输入文字时）"),
            ("Q", "停止音频同步（未输入文字时）"),
            ("H", "隐藏并释放控制台窗口（未输入文字时）"),
        ];

        egui::Grid::new("shortcuts_grid")
            .num_columns(2)
            .spacing(Vec2::new(12.0, 6.0))
            .show(ui, |ui| {
                for (key, desc) in shortcuts {
                    theme::badge(ui, key, ACCENT_PINK_SOFT, ACCENT_PINK_DEEP);
                    ui.label(RichText::new(desc).size(12.0).color(TEXT_PRIMARY));
                    ui.end_row();
                }
            });
    });

    ui.add_space(10.0);

    // Card 4: Runtime Performance Summary
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::section_title(ui, "性能指标摘要", None);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                theme::badge(ui, "0 GPU · 0 WebView2", ACCENT_MINT_SOFT, ACCENT_MINT);
            });
        });
        ui.add_space(8.0);

        egui::Grid::new("perf_metrics_grid")
            .num_columns(2)
            .spacing(Vec2::new(16.0, 6.0))
            .show(ui, |ui| {
                ui.label(RichText::new("控制台光栅耗时").size(12.0).color(TEXT_SECONDARY));
                ui.label(
                    RichText::new(format!("{:.2} ms/帧（按需脏重绘）", metrics.ui_frame_ms))
                        .monospace()
                        .size(12.0)
                        .color(TEXT_PRIMARY),
                );
                ui.end_row();

                ui.label(RichText::new("桌宠合成耗时").size(12.0).color(TEXT_SECONDARY));
                let pet_ms = if metrics.pet_frame_ms > 0.0 {
                    format!("{:.2} ms/帧", metrics.pet_frame_ms)
                } else {
                    "< 2.00 ms/帧".to_string()
                };
                ui.label(RichText::new(pet_ms).monospace().size(12.0).color(TEXT_PRIMARY));
                ui.end_row();

                ui.label(RichText::new("进程私有内存").size(12.0).color(TEXT_SECONDARY));
                let mem_text = if metrics.private_memory_mib > 0.0 {
                    format!("{:.1} MiB（关闭控制台可释放 UI 内存）", metrics.private_memory_mib)
                } else {
                    "统计中…".to_string()
                };
                ui.label(RichText::new(mem_text).monospace().size(12.0).color(TEXT_PRIMARY));
                ui.end_row();

                ui.label(RichText::new("中文字体引擎").size(12.0).color(TEXT_SECONDARY));
                let font_name = metrics
                    .loaded_font_name
                    .as_deref()
                    .unwrap_or("默认内置字体");
                ui.label(RichText::new(font_name).size(12.0).color(TEXT_PRIMARY));
                ui.end_row();
            });
    });
}

// ============================================================================
// Confirmation Modals (AppModal)
// ============================================================================

fn render_modal_overlay(
    ctx: &egui::Context,
    ui_state: &mut ConsoleUiState,
    state: &AppState,
    actions: &mut Vec<AppAction>,
) {
    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("modal_backdrop"))
        .fixed_pos(screen.min)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(45, 28, 56, 115));
        });

    egui::Window::new("modal_dialog")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(340.0, 0.0))
        .order(egui::Order::Tooltip)
        .frame(
            egui::Frame::new()
                .fill(BG_CARD)
                .stroke(Stroke::new(1.5_f32, ACCENT_PINK))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(Margin::same(18)),
        )
        .show(ctx, |ui| match state.modal {
            AppModal::None => {}
            AppModal::ConfirmPause => {
                ui.label(
                    RichText::new("暂停一下吗？")
                        .size(16.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("当前专注进度会被保留，不过再坚持一小会儿效果更好哦～")
                        .size(12.5)
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let w = ((ui.available_width() - 8.0) * 0.5).max(80.0);
                    if theme::primary_button(ui, "继续专注", ACCENT_PINK, Vec2::new(w, 34.0))
                        .clicked()
                    {
                        actions.push(AppAction::RejectPauseTimer);
                    }
                    if theme::secondary_button(ui, "确认暂停", Vec2::new(w, 34.0)).clicked() {
                        actions.push(AppAction::ConfirmPauseTimer);
                    }
                });
            }
            AppModal::ConfirmLeaveStudy => {
                ui.label(
                    RichText::new("确定离开自习模式吗？")
                        .size(16.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("切换回休闲模式将暂停当前的自习计时。")
                        .size(12.5)
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let w = ((ui.available_width() - 8.0) * 0.5).max(80.0);
                    if theme::primary_button(ui, "留在自习", ACCENT_PINK, Vec2::new(w, 34.0))
                        .clicked()
                    {
                        actions.push(AppAction::RejectPauseTimer);
                    }
                    if theme::secondary_button(ui, "确认切换", Vec2::new(w, 34.0)).clicked() {
                        actions.push(AppAction::ConfirmPauseTimer);
                    }
                });
            }
            AppModal::GoBackStudy => {
                ui.label(
                    RichText::new("太棒了！继续加油 ✨")
                        .size(16.0)
                        .strong()
                        .color(ACCENT_PINK_DEEP),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("专注状态已保持，桌宠会一直在这里陪着你～")
                        .size(12.5)
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(14.0);
                if theme::primary_button(
                    ui,
                    "回到自习",
                    ACCENT_PINK,
                    Vec2::new(ui.available_width(), 34.0),
                )
                .clicked()
                {
                    actions.push(AppAction::DismissGoBackModal);
                }
            }
            AppModal::ConfirmSaveTodo => {
                ui.label(
                    RichText::new("保存待办清单")
                        .size(16.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("给这份清单起个名字，下次可以在待办抽屉里一键载入：")
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(6.0);
                let mut name = state.persisted.todo_list.list_name.clone();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut name)
                            .desired_width(ui.available_width())
                            .hint_text("例如：英语复习清单"),
                    )
                    .changed()
                {
                    actions.push(AppAction::SetTodoListName(name));
                }
                ui.add_space(6.0);
                let count = state.persisted.todo_list.items.len();
                ui.label(
                    RichText::new(format!("共 {count} 条待办事项"))
                        .size(11.5)
                        .color(TEXT_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let w = ((ui.available_width() - 8.0) * 0.5).max(80.0);
                    if theme::secondary_button(ui, "取消", Vec2::new(w, 34.0)).clicked() {
                        actions.push(AppAction::CancelSaveTodo);
                    }
                    if theme::primary_button(ui, "确认保存", ACCENT_PURPLE, Vec2::new(w, 34.0))
                        .clicked()
                    {
                        ui_state.save_named_todo_snapshot(&state.persisted.todo_list);
                        actions.push(AppAction::ConfirmSaveTodo);
                    }
                });
            }
        });
}

// ============================================================================
// Custom Painted Pastel Widgets (Progress Bar & 24-Bar Audio Level Meter)
// ============================================================================

fn draw_pastel_progress_bar(ui: &mut Ui, progress: f32, fill_color: Color32, height: f32) {
    let width = ui.available_width().max(40.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    let r = (height * 0.5) as u8;
    painter.rect_filled(rect, CornerRadius::same(r), Color32::from_rgb(242, 228, 238));
    let fill_w = (rect.width() * progress.clamp(0.0, 1.0)).max(0.0);
    if fill_w > 0.5 {
        let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        painter.rect_filled(fill_rect, CornerRadius::same(r), fill_color);
    }
}

fn draw_24_bar_level_meter(
    ui: &mut Ui,
    peaks: &mut [f32; 24],
    level: f32,
    bass: f32,
    beat: bool,
    running: bool,
) {
    let width = ui.available_width().max(120.0);
    let height = 38.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), BG_CARD);
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0_f32, BORDER_SOFT),
        StrokeKind::Inside,
    );

    let inner = rect.shrink2(Vec2::new(6.0, 5.0));
    let bars = 24usize;
    let gap = 3.0;
    let bar_w = ((inner.width() - gap * (bars as f32 - 1.0)) / bars as f32).max(2.0);
    let active_level = if running {
        (level * 1.35 + if beat { 0.14 } else { 0.0 }).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let active_bars = (active_level * bars as f32).round() as usize;

    for i in 0..bars {
        let x0 = inner.left() + i as f32 * (bar_w + gap);
        let frac = (i as f32 + 0.5) / bars as f32;
        // Subtle arch shaping using bass on left bars and overall level across spectrum
        let shape = if i < 8 {
            (bass * 0.9 + level * 0.5).clamp(0.0, 1.0)
        } else {
            (active_level * (1.08 - 0.35 * frac)).clamp(0.0, 1.0)
        };
        let target_h = if running && i < active_bars.max(if level > 0.01 { 1 } else { 0 }) {
            (0.22 + 0.78 * shape).clamp(0.15, 1.0)
        } else {
            0.12
        };
        // Smooth decay peak hold
        peaks[i] = if target_h >= peaks[i] {
            target_h
        } else {
            (peaks[i] * 0.82).max(target_h)
        };

        let lit = running && i < active_bars;
        let bar_h = (inner.height() * peaks[i]).max(3.0);
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(x0, inner.bottom() - bar_h),
            egui::pos2(x0 + bar_w, inner.bottom()),
        );
        let color = if !lit {
            Color32::from_rgb(241, 228, 238)
        } else if i < 12 {
            ACCENT_PINK
        } else if i < 19 {
            ACCENT_PURPLE
        } else {
            ACCENT_MINT
        };
        painter.rect_filled(bar_rect, CornerRadius::same(2), color);
    }
}

/// Computes a fast structural digest of `AppState` fields actually visible in the header
/// and currently active tab, so high-frequency audio/timer changes never trigger off-tab repaints.
pub fn compute_state_digest(
    state: &AppState,
    expressions: &[ExpressionDefinition],
    metrics: &ConsoleRuntimeMetrics,
    now_ms: f64,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();

    // Always-visible header & navigation state
    (state.active_tab as u8).hash(&mut h);
    (state.activity_mode as u8).hash(&mut h);
    (state.modal as u8).hash(&mut h);
    state.pet_locked.hash(&mut h);
    let t = &state.audio_telemetry;
    (t.mode as u8).hash(&mut h);
    t.running.hash(&mut h);
    t.locked.hash(&mut h);

    // Tab-specific visible fields only
    match state.active_tab {
        MainTab::Status => {
            state.todo_open.hash(&mut h);
            state.todo_saved_notice(now_ms).hash(&mut h);
            (state.timer.mode as u8).hash(&mut h);
            (state.timer.state as u8).hash(&mut h);
            state.timer.study_minutes.hash(&mut h);
            state.timer.format_display().hash(&mut h);
            ((state.timer.progress_ratio() * 200.0) as u32).hash(&mut h);

            state.persisted.todo_list.list_name.hash(&mut h);
            state.persisted.todo_list.items.len().hash(&mut h);
            for item in &state.persisted.todo_list.items {
                item.id.hash(&mut h);
                item.done.hash(&mut h);
                item.text.hash(&mut h);
            }
        }
        MainTab::Emotion => {
            state.selected_expression_id.hash(&mut h);
            state
                .random_expression
                .as_ref()
                .map(|e| e.id.as_str())
                .hash(&mut h);
            expressions.len().hash(&mut h);
            state.persisted.settings.random_enabled.hash(&mut h);
            state.persisted.settings.random_interval_seconds.hash(&mut h);
            for ctx_key in EventContextKey::ALL {
                for ev in RandomEventType::ALL {
                    state.persisted.event_switches.get(ctx_key, ev).hash(&mut h);
                }
            }
        }
        MainTab::AudioSync => {
            t.busy.hash(&mut h);
            t.bpm.hash(&mut h);
            t.beats.hash(&mut h);
            // Quantize to 24 discrete steps matching the 24-bar meter to avoid noisy sub-pixel churn.
            ((t.level.clamp(0.0, 1.0) * 24.0).round() as u32).hash(&mut h);
            ((t.bass.clamp(0.0, 1.0) * 24.0).round() as u32).hash(&mut h);
            t.silence_hint.hash(&mut h);
            t.file_name.hash(&mut h);
            t.error.hash(&mut h);
            ((state.sensitivity * 100.0).round() as u32).hash(&mut h);
            state.auto_check_bpm.hash(&mut h);
            state.auto_follow.hash(&mut h);
            state.audio_devices.len().hash(&mut h);
            state.selected_device_id.hash(&mut h);
        }
        MainTab::Settings => {
            state.persisted.settings.end_sound_enabled.hash(&mut h);
            state.asset_status.loaded.hash(&mut h);
            state.asset_status.pack_name.hash(&mut h);
            state.asset_status.source_hint.hash(&mut h);
            state.asset_status.error.hash(&mut h);
            metrics.sprite_count.hash(&mut h);
            metrics.asset_decoded_bytes.hash(&mut h);
            ((metrics.pet_frame_ms * 10.0) as u32).hash(&mut h);
            ((metrics.private_memory_mib * 10.0) as u32).hash(&mut h);
        }
    }

    // Modal-visible fields when a confirmation modal is open
    if state.modal == pet_core::AppModal::ConfirmSaveTodo {
        state.persisted.todo_list.list_name.hash(&mut h);
    }

    h.finish()
}

fn escape_field(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "")
}

fn unescape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn serialize_saved_todo_lists(lists: &[TodoList]) -> String {
    let mut out = String::new();
    for list in lists {
        out.push_str("LIST\t");
        out.push_str(&escape_field(&list.list_name));
        out.push('\n');
        for item in &list.items {
            out.push_str("ITEM\t");
            out.push(if item.done { '1' } else { '0' });
            out.push('\t');
            out.push_str(&escape_field(&item.id));
            out.push('\t');
            out.push_str(&escape_field(&item.text));
            out.push('\n');
        }
    }
    out
}

fn parse_saved_todo_lists(raw: &str) -> Vec<TodoList> {
    let mut lists: Vec<TodoList> = Vec::new();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("LIST\t") {
            lists.push(TodoList {
                list_name: unescape_field(rest),
                items: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("ITEM\t") {
            if let Some(current) = lists.last_mut() {
                let mut parts = rest.splitn(3, '\t');
                let done = parts.next() == Some("1");
                let id = parts.next().map(unescape_field).unwrap_or_default();
                let text = parts.next().map(unescape_field).unwrap_or_default();
                if !id.is_empty() {
                    current.items.push(TodoItem { id, text, done });
                }
            }
        }
    }
    for list in &mut lists {
        list.sanitize();
    }
    lists
}
