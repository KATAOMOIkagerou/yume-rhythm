use crate::audio_contract::{
    clamp_sensitivity, AudioCommand, AudioDeviceInfo, AudioMode, AudioTelemetry, DEFAULT_DEMO_BPM,
    DEFAULT_SENSITIVITY,
};
use crate::clock::FastRng;
use crate::expressions::{ExpressionDefinition, ExpressionType};
use crate::manifest::CharacterManifest;
use crate::settings::{
    ActivityMode, EventContextKey, PersistedState, RandomEventType, TodoItem,
    MAX_RANDOM_INTERVAL_SECONDS, MIN_RANDOM_INTERVAL_SECONDS,
};
use crate::timer::{StudyTimer, TimerMode, TimerState};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainTab {
    #[default]
    Status,
    Emotion,
    AudioSync,
    Settings,
}

impl MainTab {
    pub const ALL: [MainTab; 4] = [
        MainTab::Status,
        MainTab::Emotion,
        MainTab::AudioSync,
        MainTab::Settings,
    ];

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Status => "状态",
            Self::Emotion => "情绪",
            Self::AudioSync => "音频同步",
            Self::Settings => "设置",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppModal {
    #[default]
    None,
    ConfirmPause,
    ConfirmLeaveStudy,
    GoBackStudy,
    ConfirmSaveTodo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingPauseReason {
    Pause,
    LeaveStudy,
}

#[derive(Debug, Clone, Default)]
pub struct AssetStatus {
    pub loaded: bool,
    pub pack_name: String,
    pub source_hint: String,
    pub error: Option<String>,
}

/// Resolved layer filenames for the current character frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFaceLayers {
    pub effective_expression_id: String,
    pub base_animation: Option<String>,
    pub eyes: String,
    pub mouth: String,
    pub overlay: Option<String>,
    pub active_event: Option<RandomEventType>,
    pub is_dozing: bool,
    pub suppress_blink: bool,
}

/// All user / hotkey / tray / UI actions dispatched into `AppState`.
#[derive(Debug, Clone)]
pub enum AppAction {
    ToggleLock,
    SetLocked(bool),
    ShowConsole,
    HideConsole,
    ToggleConsole,
    Quit,
    ReloadAssets,
    SelectTab(MainTab),
    SelectActivityMode(ActivityMode),
    SetTimerMode(TimerMode),
    SetStudyMinutes(u32),
    StartOrResumeTimer,
    RequestPauseTimer,
    ConfirmPauseTimer,
    RejectPauseTimer,
    DismissGoBackModal,
    ResetTimer,
    ToggleEventSwitch(EventContextKey, RandomEventType),
    ToggleTodoDrawer,
    SetTodoListName(String),
    ToggleTodoItem(String),
    UpdateTodoItemText { id: String, text: String },
    AddTodoItem,
    RemoveTodoItem(String),
    RequestSaveTodo,
    ConfirmSaveTodo,
    CancelSaveTodo,
    SelectExpression(String),
    SetEndSoundEnabled(bool),
    SetRandomEnabled(bool),
    SetRandomIntervalSeconds(u32),
    StartSystemAudio { device_id: Option<String> },
    StartAudioFile(PathBuf),
    StartDemoAudio,
    StopAudio,
    TogglePlayLastAudio,
    SetSensitivity(f32),
    ToggleAutoCheckBpm,
    ToggleAutoFollow,
    OpenSystemSoundSettings,
    RefreshAudioDevices,
}

/// Side-effects emitted by `AppState::dispatch` / `AppState::tick` for `pet-app` to execute.
#[derive(Debug, Clone)]
pub enum AppEffect {
    Audio(AudioCommand),
    PersistState,
    ReloadAssets,
    ApplyPetLock(bool),
    ShowConsole,
    HideConsole,
    OpenSystemSoundSettings,
    Quit,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub persisted: PersistedState,
    pub pet_locked: bool,
    pub console_visible: bool,
    pub active_tab: MainTab,
    pub activity_mode: ActivityMode,
    pub timer: StudyTimer,
    pub modal: AppModal,
    pending_pause_reason: PendingPauseReason,
    pub todo_open: bool,
    pub todo_saved_until_ms: Option<f64>,
    pub selected_expression_id: String,
    pub base_expression_id: String,
    pub random_expression: Option<ExpressionDefinition>,
    random_next_trigger_ms: Option<f64>,
    random_end_ms: Option<f64>,
    random_pool_key: String,
    pub auto_check_bpm: bool,
    pub auto_follow: bool,
    pub sensitivity: f32,
    pub audio_telemetry: AudioTelemetry,
    pub audio_devices: Vec<AudioDeviceInfo>,
    pub selected_device_id: Option<String>,
    pub last_audio_mode: AudioMode,
    pub last_audio_file: Option<PathBuf>,
    pub asset_status: AssetStatus,
    pub event_switches_expanded: bool,
    next_todo_seq: u64,
    rng: FastRng,
}

impl AppState {
    pub fn new(mut persisted: PersistedState) -> Self {
        persisted.sanitize();
        let study_minutes = persisted.study_minutes;
        Self {
            persisted,
            pet_locked: false,
            console_visible: true,
            active_tab: MainTab::Status,
            activity_mode: ActivityMode::Chill,
            timer: StudyTimer::new(study_minutes),
            modal: AppModal::None,
            pending_pause_reason: PendingPauseReason::Pause,
            todo_open: false,
            todo_saved_until_ms: None,
            selected_expression_id: "neutral".into(),
            base_expression_id: "neutral".into(),
            random_expression: None,
            random_next_trigger_ms: None,
            random_end_ms: None,
            random_pool_key: String::new(),
            auto_check_bpm: true,
            auto_follow: true,
            sensitivity: DEFAULT_SENSITIVITY,
            audio_telemetry: AudioTelemetry::default(),
            audio_devices: Vec::new(),
            selected_device_id: None,
            last_audio_mode: AudioMode::System,
            last_audio_file: None,
            asset_status: AssetStatus::default(),
            event_switches_expanded: false,
            next_todo_seq: 4,
            rng: FastRng::from_entropy(),
        }
    }

    pub fn music_running(&self) -> bool {
        self.audio_telemetry.running
    }

    pub fn todo_saved_notice(&self, now_ms: f64) -> bool {
        self.todo_saved_until_ms
            .map(|until| now_ms < until)
            .unwrap_or(false)
    }

    pub fn silence_hint_text(&self) -> Option<&'static str> {
        if self.audio_telemetry.error.is_none() && self.audio_telemetry.silence_hint {
            Some("似乎没有捕获到声音：请确认音乐正在播放，并检查系统输出设备。")
        } else {
            None
        }
    }

    pub fn dispatch(
        &mut self,
        action: AppAction,
        expressions: &[ExpressionDefinition],
        now_ms: f64,
        wall_now_ms: i64,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match action {
            AppAction::ToggleLock => {
                self.pet_locked = !self.pet_locked;
                effects.push(AppEffect::ApplyPetLock(self.pet_locked));
            }
            AppAction::SetLocked(locked) => {
                if self.pet_locked != locked {
                    self.pet_locked = locked;
                    effects.push(AppEffect::ApplyPetLock(locked));
                }
            }
            AppAction::ShowConsole => {
                self.console_visible = true;
                effects.push(AppEffect::ShowConsole);
            }
            AppAction::HideConsole => {
                self.console_visible = false;
                effects.push(AppEffect::HideConsole);
            }
            AppAction::ToggleConsole => {
                self.console_visible = !self.console_visible;
                if self.console_visible {
                    effects.push(AppEffect::ShowConsole);
                } else {
                    effects.push(AppEffect::HideConsole);
                }
            }
            AppAction::Quit => {
                effects.push(AppEffect::Audio(AudioCommand::Shutdown));
                effects.push(AppEffect::Quit);
            }
            AppAction::ReloadAssets => {
                effects.push(AppEffect::ReloadAssets);
            }
            AppAction::SelectTab(tab) => {
                self.active_tab = tab;
            }
            AppAction::SelectActivityMode(next_mode) => {
                if next_mode != self.activity_mode {
                    if next_mode == ActivityMode::Chill
                        && self.activity_mode == ActivityMode::Study
                        && self.timer.state == TimerState::Running
                        && (self.timer.mode == TimerMode::Stopwatch || self.timer.remaining_ms > 0)
                    {
                        self.pending_pause_reason = PendingPauseReason::LeaveStudy;
                        self.modal = AppModal::ConfirmLeaveStudy;
                    } else {
                        self.activity_mode = next_mode;
                        self.reset_random_schedule(now_ms);
                    }
                }
            }
            AppAction::SetTimerMode(mode) => {
                self.timer.set_mode(mode);
            }
            AppAction::SetStudyMinutes(mins) => {
                let clamped = self.timer.set_study_minutes(mins);
                self.persisted.study_minutes = clamped;
                effects.push(AppEffect::PersistState);
            }
            AppAction::StartOrResumeTimer => {
                if self.timer.state != TimerState::Running {
                    self.activity_mode = ActivityMode::Study;
                    self.timer.start_or_resume(wall_now_ms);
                }
            }
            AppAction::RequestPauseTimer => {
                if self.timer.state == TimerState::Running {
                    self.pending_pause_reason = PendingPauseReason::Pause;
                    if self.timer.mode == TimerMode::Stopwatch {
                        let completed = self.timer.pause(wall_now_ms);
                        if completed && self.persisted.settings.end_sound_enabled {
                            effects.push(AppEffect::Audio(AudioCommand::PlayCompletionChime));
                        }
                    } else {
                        self.modal = AppModal::ConfirmPause;
                    }
                }
            }
            AppAction::ConfirmPauseTimer => {
                let completed = self.timer.pause(wall_now_ms);
                if completed && self.persisted.settings.end_sound_enabled {
                    effects.push(AppEffect::Audio(AudioCommand::PlayCompletionChime));
                }
                if self.pending_pause_reason == PendingPauseReason::LeaveStudy {
                    self.activity_mode = ActivityMode::Chill;
                    self.reset_random_schedule(now_ms);
                }
                self.modal = AppModal::None;
            }
            AppAction::RejectPauseTimer => {
                self.modal = AppModal::GoBackStudy;
            }
            AppAction::DismissGoBackModal => {
                self.modal = AppModal::None;
            }
            AppAction::ResetTimer => {
                self.timer.reset();
            }
            AppAction::ToggleEventSwitch(ctx, ev) => {
                if self.persisted.event_switches.toggle(ctx, ev) {
                    self.reset_random_schedule(now_ms);
                    effects.push(AppEffect::PersistState);
                }
            }
            AppAction::ToggleTodoDrawer => {
                self.todo_open = !self.todo_open;
            }
            AppAction::SetTodoListName(name) => {
                self.persisted.todo_list.list_name = name.chars().take(40).collect();
            }
            AppAction::ToggleTodoItem(id) => {
                if let Some(item) = self
                    .persisted
                    .todo_list
                    .items
                    .iter_mut()
                    .find(|i| i.id == id)
                {
                    item.done = !item.done;
                }
            }
            AppAction::UpdateTodoItemText { id, text } => {
                if let Some(item) = self
                    .persisted
                    .todo_list
                    .items
                    .iter_mut()
                    .find(|i| i.id == id)
                {
                    item.text = text.chars().take(60).collect();
                }
            }
            AppAction::AddTodoItem => {
                let seq = self.next_todo_seq;
                self.next_todo_seq += 1;
                self.persisted.todo_list.items.push(TodoItem {
                    id: format!("todo-{wall_now_ms}-{seq}"),
                    text: String::new(),
                    done: false,
                });
            }
            AppAction::RemoveTodoItem(id) => {
                if self.persisted.todo_list.items.len() > 1 {
                    self.persisted.todo_list.items.retain(|i| i.id != id);
                }
            }
            AppAction::RequestSaveTodo => {
                self.modal = AppModal::ConfirmSaveTodo;
            }
            AppAction::ConfirmSaveTodo => {
                self.persisted.todo_list.sanitize();
                self.todo_saved_until_ms = Some(now_ms + 4000.0);
                self.modal = AppModal::None;
                effects.push(AppEffect::PersistState);
            }
            AppAction::CancelSaveTodo => {
                self.modal = AppModal::None;
            }
            AppAction::SelectExpression(id) => {
                let active_id = self
                    .effective_expression(expressions)
                    .map(|e| e.id.as_str())
                    .unwrap_or("neutral")
                    .to_string();
                self.random_expression = None;
                self.random_end_ms = None;
                let next_id = if active_id == id {
                    "neutral".to_string()
                } else {
                    id
                };
                self.selected_expression_id = next_id.clone();
                if expressions
                    .iter()
                    .find(|e| e.id == next_id)
                    .map(|e| e.kind == ExpressionType::Base)
                    .unwrap_or(false)
                {
                    self.base_expression_id = next_id;
                }
            }
            AppAction::SetEndSoundEnabled(enabled) => {
                self.persisted.settings.end_sound_enabled = enabled;
                effects.push(AppEffect::PersistState);
            }
            AppAction::SetRandomEnabled(enabled) => {
                self.persisted.settings.random_enabled = enabled;
                self.reset_random_schedule(now_ms);
                effects.push(AppEffect::PersistState);
            }
            AppAction::SetRandomIntervalSeconds(secs) => {
                self.persisted.settings.random_interval_seconds =
                    secs.clamp(MIN_RANDOM_INTERVAL_SECONDS, MAX_RANDOM_INTERVAL_SECONDS);
                self.reset_random_schedule(now_ms);
                effects.push(AppEffect::PersistState);
            }
            AppAction::StartSystemAudio { device_id } => {
                self.selected_device_id = device_id.clone();
                self.last_audio_mode = AudioMode::System;
                self.audio_telemetry.busy = true;
                self.audio_telemetry.error = None;
                effects.push(AppEffect::Audio(AudioCommand::SetSensitivity(
                    self.sensitivity,
                )));
                effects.push(AppEffect::Audio(AudioCommand::StartSystem { device_id }));
            }
            AppAction::StartAudioFile(path) => {
                self.last_audio_mode = AudioMode::File;
                self.last_audio_file = Some(path.clone());
                self.audio_telemetry.busy = true;
                self.audio_telemetry.error = None;
                effects.push(AppEffect::Audio(AudioCommand::SetSensitivity(
                    self.sensitivity,
                )));
                effects.push(AppEffect::Audio(AudioCommand::StartFile { path }));
            }
            AppAction::StartDemoAudio => {
                self.last_audio_mode = AudioMode::Demo;
                self.audio_telemetry.busy = true;
                self.audio_telemetry.error = None;
                effects.push(AppEffect::Audio(AudioCommand::SetSensitivity(
                    self.sensitivity,
                )));
                effects.push(AppEffect::Audio(AudioCommand::StartDemo {
                    bpm: DEFAULT_DEMO_BPM,
                }));
            }
            AppAction::StopAudio => {
                self.audio_telemetry = AudioTelemetry {
                    sensitivity: self.sensitivity,
                    ..AudioTelemetry::default()
                };
                effects.push(AppEffect::Audio(AudioCommand::Stop));
            }
            AppAction::TogglePlayLastAudio => {
                if self.music_running() {
                    return self.dispatch(AppAction::StopAudio, expressions, now_ms, wall_now_ms);
                }
                if self.last_audio_mode == AudioMode::File {
                    if let Some(path) = self.last_audio_file.clone() {
                        return self.dispatch(
                            AppAction::StartAudioFile(path),
                            expressions,
                            now_ms,
                            wall_now_ms,
                        );
                    }
                }
                if self.last_audio_mode == AudioMode::Demo {
                    return self.dispatch(AppAction::StartDemoAudio, expressions, now_ms, wall_now_ms);
                }
                let device_id = self.selected_device_id.clone();
                return self.dispatch(
                    AppAction::StartSystemAudio { device_id },
                    expressions,
                    now_ms,
                    wall_now_ms,
                );
            }
            AppAction::SetSensitivity(val) => {
                let clamped = clamp_sensitivity(val);
                self.sensitivity = clamped;
                self.audio_telemetry.sensitivity = clamped;
                effects.push(AppEffect::Audio(AudioCommand::SetSensitivity(clamped)));
            }
            AppAction::ToggleAutoCheckBpm => {
                self.auto_check_bpm = !self.auto_check_bpm;
            }
            AppAction::ToggleAutoFollow => {
                self.auto_follow = !self.auto_follow;
            }
            AppAction::OpenSystemSoundSettings => {
                effects.push(AppEffect::OpenSystemSoundSettings);
            }
            AppAction::RefreshAudioDevices => {
                effects.push(AppEffect::Audio(AudioCommand::RefreshDevices));
            }
        }
        effects
    }

    /// Periodic state tick: advances study timer, expires/triggers random expressions, clears notices.
    pub fn tick(
        &mut self,
        expressions: &[ExpressionDefinition],
        now_ms: f64,
        wall_now_ms: i64,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();

        if self.timer.tick(wall_now_ms) {
            self.modal = AppModal::None;
            if self.persisted.settings.end_sound_enabled {
                effects.push(AppEffect::Audio(AudioCommand::PlayCompletionChime));
            }
        }

        if let Some(until) = self.todo_saved_until_ms {
            if now_ms >= until {
                self.todo_saved_until_ms = None;
            }
        }

        // Expire temporary random expression and restore manual selection
        if let Some(end_at) = self.random_end_ms {
            if now_ms >= end_at {
                self.random_expression = None;
                self.random_end_ms = None;
            }
        }

        // Evaluate eligible random expression pool
        let music_running = self.music_running();
        let eligible: Vec<&ExpressionDefinition> = expressions
            .iter()
            .filter(|entry| {
                entry.random
                    && entry
                        .event
                        .map(|ev| {
                            self.persisted
                                .event_switches
                                .is_event_allowed(ev, self.activity_mode, music_running)
                        })
                        .unwrap_or(true)
            })
            .collect();

        let pool_key: String = eligible
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>()
            .join("|");

        if pool_key != self.random_pool_key {
            self.random_pool_key = pool_key;
            self.reset_random_schedule(now_ms);
        }

        if !self.persisted.settings.random_enabled || eligible.is_empty() {
            self.random_next_trigger_ms = None;
            return effects;
        }

        let interval_ms = f64::from(self.persisted.settings.random_interval_seconds) * 1000.0;
        let next_at = match self.random_next_trigger_ms {
            Some(ts) => ts,
            None => {
                let ts = now_ms + interval_ms;
                self.random_next_trigger_ms = Some(ts);
                ts
            }
        };

        if now_ms >= next_at {
            let pick = eligible[self.rng.next_usize(eligible.len())].clone();
            let dur = f64::from(pick.duration_ms);
            self.random_expression = Some(pick);
            self.random_end_ms = Some(now_ms + dur);
            self.random_next_trigger_ms = Some(now_ms + interval_ms);
        }

        effects
    }

    pub fn effective_expression<'a>(
        &'a self,
        expressions: &'a [ExpressionDefinition],
    ) -> Option<&'a ExpressionDefinition> {
        let music_running = self.music_running();
        let allowed = |entry: &'a ExpressionDefinition| -> bool {
            entry
                .event
                .map(|ev| {
                    self.persisted
                        .event_switches
                        .is_event_allowed(ev, self.activity_mode, music_running)
                })
                .unwrap_or(true)
        };

        if let Some(rand_expr) = self.random_expression.as_ref() {
            if allowed(rand_expr) {
                return Some(rand_expr);
            }
        }
        let manual = expressions
            .iter()
            .find(|entry| entry.id == self.selected_expression_id);
        match manual {
            Some(entry) if music_running && entry.event == Some(RandomEventType::Doze) => None,
            other => other,
        }
    }

    pub fn resolve_face_layers(
        &self,
        manifest: &CharacterManifest,
        expressions: &[ExpressionDefinition],
        is_blinking: bool,
    ) -> ResolvedFaceLayers {
        let expr = self.effective_expression(expressions);
        let base = match expr {
            Some(e) if e.kind == ExpressionType::Base => Some(e),
            _ => expressions
                .iter()
                .find(|entry| entry.id == self.base_expression_id),
        };
        let accessory = match expr {
            Some(e) if e.kind == ExpressionType::Accessory => Some(&e.resources),
            _ => None,
        };
        let base_animation = base.and_then(|b| b.resources.animation.clone());
        let active_event = expr.and_then(|e| {
            if self.music_running() && e.event == Some(RandomEventType::Doze) {
                None
            } else {
                e.event
            }
        });
        let is_dozing = !self.music_running() && active_event == Some(RandomEventType::Doze);
        let suppress_blink = base.map(|b| b.id == "happy").unwrap_or(false)
            || matches!(
                active_event,
                Some(RandomEventType::Love | RandomEventType::Doze)
            );

        let eyes = accessory
            .and_then(|a| a.eyes.clone())
            .unwrap_or_else(|| {
                if is_blinking && !suppress_blink && base_animation.is_none() {
                    manifest.blink_eyes.clone()
                } else {
                    base.and_then(|b| b.resources.eyes.clone())
                        .unwrap_or_else(|| manifest.expressions.neutral.eyes.clone())
                }
            });

        let mouth = accessory
            .and_then(|a| a.mouth.clone())
            .or_else(|| base.and_then(|b| b.resources.mouth.clone()))
            .unwrap_or_else(|| manifest.expressions.neutral.mouth.clone());

        let overlay = accessory.and_then(|a| a.overlay.clone().or_else(|| a.animation.clone()));

        ResolvedFaceLayers {
            effective_expression_id: expr
                .map(|e| e.id.clone())
                .unwrap_or_else(|| "neutral".into()),
            base_animation,
            eyes,
            mouth,
            overlay,
            active_event,
            is_dozing,
            suppress_blink,
        }
    }

    fn reset_random_schedule(&mut self, now_ms: f64) {
        self.random_expression = None;
        self.random_end_ms = None;
        if self.persisted.settings.random_enabled {
            let interval_ms = f64::from(self.persisted.settings.random_interval_seconds) * 1000.0;
            self.random_next_trigger_ms = Some(now_ms + interval_ms);
        } else {
            self.random_next_trigger_ms = None;
        }
    }
}
