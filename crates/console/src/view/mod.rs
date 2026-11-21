//! 把 `AppState` 投影成界面要显示的内容。纯函数，不依赖 Slint，可以直接单测。
//!
//! 这里只做"怎么显示"：文案、格式、下标与 id 的对应。"能不能做"的判断调用 core 的方法，
//! 不在这里重新实现。

mod settings;
#[cfg(test)]
mod tests;

pub use settings::{Settings, ShortcutItem, ThemePresetView};

use yume_core::model::audio::{AudioSource, AudioStatus};
use yume_core::model::events::{ActivityMode, EventContext, EventSwitches, RandomEvent};
use yume_core::model::expression::ExpressionKind;
use yume_core::model::pet::{CharacterStatus, Dialog};
use yume_core::model::settings::DEFAULT_FONT_FAMILY;
use yume_core::model::shortcuts::Shortcuts;
use yume_core::model::theme::{ThemePalette, resolve_palette};
use yume_core::model::timer::{TimerMode, TimerStatus};
use yume_core::{AppState, WallMs};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub header: Header,
    pub focus: Focus,
    pub todo: Todo,
    pub todo_items: Vec<TodoRow>,
    pub emotion: Emotion,
    pub base_expressions: Vec<Chip>,
    pub accessory_expressions: Vec<Chip>,
    pub event_columns: Vec<String>,
    pub event_rows: Vec<EventRow>,
    pub music: Music,
    pub devices: Vec<String>,
    pub settings: Settings,
    pub characters: Vec<String>,
    pub theme_presets: Vec<ThemePresetView>,
    pub shortcuts: Vec<ShortcutItem>,
    pub fonts: Vec<String>,
    pub palette: Option<ThemePalette>,
    pub font_family: String,
    pub dialog: i32,
    /// 下拉框下标对应的设备 id。0 为系统默认（`None`）。
    pub device_ids: Vec<Option<String>>,
    pub character_ids: Vec<String>,
    /// 字体下拉框下标对应的字体家族名。0 为默认字体（`None`）。
    pub font_families: Vec<Option<String>>,
    /// 当前快捷键配置，供窗口按键回调判断是否消耗本地按键事件。
    pub shortcuts_config: Shortcuts,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub character: String,
    pub audio_text: String,
    pub audio_active: bool,
    pub locked: bool,
}

/// 计时器主按钮当前的含义。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimerPhase {
    #[default]
    Idle = 0,
    Running = 1,
    Paused = 2,
    Completed = 3,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Focus {
    pub study: bool,
    pub countdown: bool,
    pub mode_locked: bool,
    pub minutes: i32,
    pub minutes_editable: bool,
    pub time_text: String,
    pub status_text: String,
    pub phase: TimerPhase,
    pub progress: f32,
    pub show_progress: bool,
    pub can_reset: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Todo {
    pub name: String,
    pub can_remove: bool,
    pub done_count: i32,
    pub total: i32,
    pub md_bound: bool,
    pub md_label: String,
    pub md_error: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TodoRow {
    pub id: u32,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Emotion {
    pub current_name: String,
    pub current_random: bool,
    pub random_enabled: bool,
    pub random_interval: i32,
    pub doze_note: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chip {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventRow {
    pub name: String,
    /// 按 `EventContext::ALL` 的顺序。
    pub cells: Vec<EventCell>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventCell {
    pub on: bool,
    pub editable: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioPhase {
    #[default]
    Idle = 0,
    Starting = 1,
    Running = 2,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Music {
    pub phase: AudioPhase,
    pub status_text: String,
    pub source_text: String,
    pub button_text: String,
    pub show_bpm: bool,
    pub bpm_text: String,
    pub beat_locked: bool,
    pub beat_text: String,
    pub level: f32,
    pub bass: f32,
    pub can_shift: bool,
    pub silence_hint: bool,
    pub error_text: String,
    pub device_index: i32,
    pub auto_follow: bool,
}

#[must_use]
pub fn project(state: &AppState, now: WallMs) -> View {
    let device_ids: Vec<Option<String>> = std::iter::once(None)
        .chain(state.audio().devices.iter().map(|d| Some(d.id.clone())))
        .collect();
    let character_ids = state
        .character()
        .available
        .iter()
        .map(|c| c.id.clone())
        .collect();
    let (fonts, font_families) = settings::font_options(state);
    let (base_expressions, accessory_expressions) = expression_chips(state);
    let palette = resolve_palette(
        state.settings().theme,
        state.settings().custom_accent.as_deref(),
    );
    let font_family = state
        .settings()
        .font_family
        .clone()
        .unwrap_or_else(|| DEFAULT_FONT_FAMILY.to_owned());

    View {
        header: header(state),
        focus: focus(state, now),
        todo: todo(state),
        todo_items: state
            .todo()
            .items()
            .iter()
            .map(|i| TodoRow {
                id: i.id,
                text: i.text.clone(),
                done: i.done,
            })
            .collect(),
        emotion: emotion(state),
        base_expressions,
        accessory_expressions,
        event_columns: EventContext::ALL
            .iter()
            .map(|c| context_name(*c).into())
            .collect(),
        event_rows: event_rows(state.switches()),
        music: music(state, now, &device_ids),
        devices: std::iter::once("系统默认输出设备".to_owned())
            .chain(state.audio().devices.iter().map(|d| d.name.clone()))
            .collect(),
        settings: settings::project_settings(state, &font_families),
        characters: settings::characters(state),
        theme_presets: settings::theme_presets(state),
        shortcuts: settings::shortcut_items(state),
        fonts,
        palette: Some(palette),
        font_family,
        dialog: match state.dialog() {
            Dialog::None => 0,
            Dialog::ConfirmPause => 1,
            Dialog::ConfirmLeaveStudy => 2,
            Dialog::Encourage => 3,
        },
        device_ids,
        character_ids,
        font_families,
        shortcuts_config: state.settings().shortcuts.clone(),
    }
}

fn header(state: &AppState) -> Header {
    let audio = state.audio();
    let t = &audio.telemetry;
    let audio_text = match audio.status {
        AudioStatus::Idle => "未在听歌".to_owned(),
        AudioStatus::Starting => "连接中".to_owned(),
        AudioStatus::Running if state.settings().show_bpm && t.bpm > 0.0 => {
            format!("{:.0} BPM", t.bpm)
        }
        AudioStatus::Running => "同步中".to_owned(),
    };
    Header {
        character: display_character_name(state),
        audio_text,
        audio_active: audio.is_running(),
        locked: state.pet().locked,
    }
}

fn display_character_name(state: &AppState) -> String {
    let c = state.character();
    if !c.name.is_empty() {
        return c.name.clone();
    }
    match c.status {
        CharacterStatus::Loading => "正在载入…".into(),
        _ => "桌宠".into(),
    }
}

fn focus(state: &AppState, now: WallMs) -> Focus {
    let timer = state.timer();
    let countdown = timer.mode() == TimerMode::Countdown;
    let value = timer.value_at(now);
    let phase = match timer.status() {
        TimerStatus::Idle => TimerPhase::Idle,
        TimerStatus::Running => TimerPhase::Running,
        TimerStatus::Paused => TimerPhase::Paused,
        TimerStatus::Completed => TimerPhase::Completed,
    };
    let status_text = match (phase, countdown) {
        (TimerPhase::Idle, true) => "准备好了就开始吧",
        (TimerPhase::Idle, false) => "从零开始计时，随时可以暂停",
        (TimerPhase::Running, _) => "专注中",
        (TimerPhase::Paused, _) => "已暂停",
        (TimerPhase::Completed, _) => "本次自习时间到啦",
    };
    let total = timer.round_total_ms();
    #[allow(
        clippy::cast_precision_loss,
        reason = "毫秒数远小于 2^24 秒，进度精度足够"
    )]
    let progress = if total > 0 {
        (1.0 - value as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Focus {
        study: state.activity() == ActivityMode::Study,
        countdown,
        mode_locked: timer.is_active(),
        minutes: i32::try_from(timer.minutes()).unwrap_or(i32::MAX),
        minutes_editable: !timer.is_active(),
        time_text: clock_text(value, countdown),
        status_text: status_text.into(),
        phase,
        progress,
        show_progress: countdown && phase != TimerPhase::Idle,
        can_reset: matches!(phase, TimerPhase::Running | TimerPhase::Paused),
    }
}

/// 倒计时向上取整到秒（刚开始显示 25:00），正计时向下取整。超过一小时显示时。
#[must_use]
pub fn clock_text(ms: i64, round_up: bool) -> String {
    let ms = ms.max(0);
    let secs = if round_up {
        (ms + 999) / 1000
    } else {
        ms / 1000
    };
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn todo(state: &AppState) -> Todo {
    let list = state.todo();
    let done = list.items().iter().filter(|i| i.done).count();
    let md_path = state.settings().todo_md_path.as_deref();
    let md_bound = md_path.is_some();
    let md_label = match md_path {
        Some(p) => format!("外接：{}（自动回写）", file_name(p)),
        None => "修改会自动保存".to_owned(),
    };
    let md_error = state
        .todo_md_error()
        .map(settings::todo_md_error_text)
        .unwrap_or_default()
        .to_owned();
    Todo {
        name: list.name().to_owned(),
        can_remove: list.can_remove(),
        done_count: i32::try_from(done).unwrap_or(i32::MAX),
        total: i32::try_from(list.items().len()).unwrap_or(i32::MAX),
        md_bound,
        md_label,
        md_error,
    }
}

fn emotion(state: &AppState) -> Emotion {
    let situation = state.situation();
    let expressions = state.expressions();
    let settings = state.settings();
    Emotion {
        current_name: expressions
            .effective(&situation)
            .map_or_else(|| "自然".to_owned(), |m| m.name.clone()),
        current_random: expressions.showing_random(&situation),
        random_enabled: settings.random_enabled,
        random_interval: i32::try_from(settings.random_interval_s).unwrap_or(i32::MAX),
        doze_note: expressions
            .catalog()
            .iter()
            .any(|m| !situation.shows_manual(m)),
    }
}

fn expression_chips(state: &AppState) -> (Vec<Chip>, Vec<Chip>) {
    let situation = state.situation();
    let expressions = state.expressions();
    let effective = expressions.effective(&situation).map(|m| m.id.as_str());
    let base = expressions
        .effective_base(&situation)
        .map(|m| m.id.as_str());
    let (mut bases, mut accessories) = (Vec::new(), Vec::new());
    for meta in expressions.catalog() {
        let chip = Chip {
            id: meta.id.clone(),
            name: meta.name.clone(),
            active: match meta.kind {
                ExpressionKind::Base => base == Some(meta.id.as_str()),
                ExpressionKind::Accessory => effective == Some(meta.id.as_str()),
            },
            enabled: situation.shows_manual(meta),
        };
        match meta.kind {
            ExpressionKind::Base => bases.push(chip),
            ExpressionKind::Accessory => accessories.push(chip),
        }
    }
    (bases, accessories)
}

fn event_rows(switches: &EventSwitches) -> Vec<EventRow> {
    RandomEvent::ALL
        .iter()
        .map(|&event| EventRow {
            name: event_name(event).into(),
            cells: EventContext::ALL
                .iter()
                .map(|&ctx| EventCell {
                    on: switches.get(ctx, event),
                    editable: EventSwitches::is_editable(ctx, event),
                })
                .collect(),
        })
        .collect()
}

#[must_use]
pub fn event_name(event: RandomEvent) -> &'static str {
    match event {
        RandomEvent::Doze => "打瞌睡",
        RandomEvent::Think => "思考",
        RandomEvent::Shock => "震惊",
        RandomEvent::Love => "喜欢",
    }
}

fn context_name(ctx: EventContext) -> &'static str {
    match ctx {
        EventContext::Chill => "休闲",
        EventContext::Study => "自习",
        EventContext::Music => "听歌",
    }
}

fn music(state: &AppState, now: WallMs, device_ids: &[Option<String>]) -> Music {
    let audio = state.audio();
    let settings = state.settings();
    let t = &audio.telemetry;
    let phase = match audio.status {
        AudioStatus::Idle => AudioPhase::Idle,
        AudioStatus::Starting => AudioPhase::Starting,
        AudioStatus::Running => AudioPhase::Running,
    };
    let device_name = settings
        .audio_device
        .as_ref()
        .and_then(|id| audio.devices.iter().find(|d| &d.id == id))
        .map_or("系统默认设备", |d| d.name.as_str());
    let source_text = match (&audio.source, phase) {
        (_, AudioPhase::Idle) | (None, _) => String::new(),
        (Some(AudioSource::System), _) => format!("电脑声音 · {device_name}"),
        (Some(AudioSource::File { path }), _) => format!("文件 · {}", file_name(path)),
    };
    Music {
        phase,
        status_text: match phase {
            AudioPhase::Idle => "未开始",
            AudioPhase::Starting => "连接中…",
            AudioPhase::Running => "同步中",
        }
        .into(),
        source_text,
        button_text: match phase {
            AudioPhase::Idle => "开始监听电脑声音",
            AudioPhase::Starting | AudioPhase::Running => "停止",
        }
        .into(),
        show_bpm: settings.show_bpm,
        bpm_text: if audio.is_running() && t.bpm > 0.0 {
            format!("{:.0}", t.bpm)
        } else {
            "--".into()
        },
        beat_locked: audio.is_running() && t.beat_locked,
        beat_text: if audio.is_running() {
            format!("已跟拍 {} 次", t.beats)
        } else {
            String::new()
        },
        level: t.level.clamp(0.0, 1.0),
        bass: t.bass.clamp(0.0, 1.0),
        can_shift: audio.is_running() && t.beat_locked,
        silence_hint: audio.silence_hint(now),
        error_text: audio
            .error
            .map(settings::audio_error_text)
            .unwrap_or_default()
            .into(),
        device_index: device_ids
            .iter()
            .position(|id| id == &settings.audio_device)
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(0),
        auto_follow: settings.auto_follow,
    }
}

pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}
