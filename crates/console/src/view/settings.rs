//! 设置页视图投影：主题预设、透明度、字体列表、快捷键与角色信息。

use super::file_name;
use yume_core::AppState;
use yume_core::model::audio::AudioError;
use yume_core::model::pet::CharacterStatus;
use yume_core::model::shortcuts::{ShortcutAction, ShortcutError, display_label};
use yume_core::model::theme::{Rgb, ThemePreset, resolve_palette};
use yume_core::model::todo_md::TodoMdError;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub end_sound: bool,
    pub pet_scale: f32,
    pub pet_scale_text: String,
    pub pet_opacity: f32,
    pub pet_opacity_text: String,
    pub console_opacity: f32,
    pub console_opacity_text: String,
    pub custom_accent: String,
    pub accent_error: bool,
    pub shortcut_error: String,
    pub font_index: i32,
    pub font_status: String,
    pub font_failed: bool,
    pub locked: bool,
    pub hide_on_fullscreen: bool,
    pub autostart: bool,
    pub character_index: i32,
    pub character_status: String,
    pub character_failed: bool,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemePresetView {
    pub key: String,
    pub name: String,
    pub swatch: Rgb,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutItem {
    pub id: String,
    pub name: String,
    pub key_text: String,
    pub is_global: bool,
    pub can_clear: bool,
    pub failed: bool,
}

pub(super) fn project_settings(state: &AppState, font_families: &[Option<String>]) -> Settings {
    let s = state.settings();
    let c = state.character();
    let (character_status, character_failed) = match &c.status {
        CharacterStatus::Loading => ("正在载入角色…".to_owned(), false),
        CharacterStatus::Ready => (String::new(), false),
        CharacterStatus::Failed(message) => (format!("角色载入失败：{message}"), true),
    };
    let (font_status, font_failed) = if let Some(err) = state.font_error() {
        (format!("字体加载失败：{err}"), true)
    } else if let Some(path) = s.custom_font_path.as_deref() {
        (format!("已加载外部字体：{}", file_name(path)), false)
    } else {
        (String::new(), false)
    };
    let font_index = font_families
        .iter()
        .position(|f| f == &s.font_family)
        .and_then(|i| i32::try_from(i).ok())
        .unwrap_or(0);

    Settings {
        end_sound: s.end_sound,
        pet_scale: s.pet_scale,
        pet_scale_text: format!("{:.0}%", s.pet_scale * 100.0),
        pet_opacity: s.pet_opacity,
        pet_opacity_text: format!("{:.0}%", s.pet_opacity * 100.0),
        console_opacity: s.console_opacity,
        console_opacity_text: format!("{:.0}%", s.console_opacity * 100.0),
        custom_accent: s.custom_accent.clone().unwrap_or_default(),
        accent_error: state.accent_error(),
        shortcut_error: state
            .shortcut_error()
            .map(shortcut_error_text)
            .unwrap_or_default(),
        font_index,
        font_status,
        font_failed,
        locked: state.pet().locked,
        hide_on_fullscreen: s.hide_on_fullscreen,
        autostart: s.autostart,
        character_index: c
            .available
            .iter()
            .position(|i| i.id == s.character)
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(-1),
        character_status,
        character_failed,
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
    }
}

pub(super) fn theme_presets(state: &AppState) -> Vec<ThemePresetView> {
    let active = state.settings().theme;
    ThemePreset::ALL
        .into_iter()
        .map(|preset| {
            let p = resolve_palette(preset, None);
            ThemePresetView {
                key: preset.key().to_owned(),
                name: theme_name(preset).to_owned(),
                swatch: if preset.is_dark() { p.bg } else { p.accent },
                active: preset == active,
            }
        })
        .collect()
}

pub(super) fn shortcut_items(state: &AppState) -> Vec<ShortcutItem> {
    let s = &state.settings().shortcuts;
    let failed = state.global_hotkeys_failed();
    ShortcutAction::ALL
        .into_iter()
        .map(|action| {
            let raw = s.get(action);
            ShortcutItem {
                id: action.id().to_owned(),
                name: shortcut_name(action).to_owned(),
                key_text: display_label(raw),
                is_global: action.is_global(),
                can_clear: !raw.is_empty(),
                failed: failed.contains(&action),
            }
        })
        .collect()
}

pub(super) fn font_options(state: &AppState) -> (Vec<String>, Vec<Option<String>>) {
    let mut labels = vec!["默认（微软雅黑 UI）".to_owned()];
    let mut families: Vec<Option<String>> = vec![None];
    for font in state.available_fonts() {
        labels.push(font.clone());
        families.push(Some(font.clone()));
    }
    if let Some(selected) = state.settings().font_family.as_ref()
        && !families.iter().any(|f| f.as_deref() == Some(selected))
    {
        labels.push(selected.clone());
        families.push(Some(selected.clone()));
    }
    (labels, families)
}

pub(super) fn characters(state: &AppState) -> Vec<String> {
    state
        .character()
        .available
        .iter()
        .map(|c| c.name.clone())
        .collect()
}

fn theme_name(preset: ThemePreset) -> &'static str {
    match preset {
        ThemePreset::Sakura => "樱花粉",
        ThemePreset::Sky => "晴空蓝",
        ThemePreset::Lavender => "丁香紫",
        ThemePreset::Mint => "薄荷绿",
        ThemePreset::Peach => "暖橘色",
        ThemePreset::Night => "深色暗夜",
    }
}

fn shortcut_name(action: ShortcutAction) -> &'static str {
    match action {
        ShortcutAction::GlobalToggleConsole => "打开 / 隐藏控制台",
        ShortcutAction::GlobalToggleLocked => "锁定 / 解锁桌宠",
        ShortcutAction::GlobalToggleAudio => "全局开始 / 停止听歌",
        ShortcutAction::LocalAudioToggle => "开始 / 停止听歌",
        ShortcutAction::LocalAudioStop => "停止听歌",
        ShortcutAction::LocalHideConsole => "隐藏控制台",
    }
}

fn shortcut_error_text(err: &ShortcutError) -> String {
    match err {
        ShortcutError::Invalid => "不支持这个按键，请使用字母、数字、空格或 F1–F12。".into(),
        ShortcutError::NeedModifier => "全局快捷键必须包含 Ctrl、Alt 或 Shift 修饰键。".into(),
        ShortcutError::Duplicate(other) => {
            format!("与「{}」重复，请换一个按键组合。", shortcut_name(*other))
        }
    }
}

pub(super) fn todo_md_error_text(err: TodoMdError) -> &'static str {
    match err {
        TodoMdError::NotFound => "找不到外接的 Markdown 文件，可能已被移动或删除。",
        TodoMdError::ReadFailed => "无法读取外接的 Markdown 文件，请检查权限或编码。",
        TodoMdError::WriteFailed => "写回外接 Markdown 失败，文件可能正被其他程序占用。",
    }
}

pub(super) fn audio_error_text(e: AudioError) -> &'static str {
    match e {
        AudioError::DeviceUnavailable => "找不到这个输出设备，可能已经断开。换一个设备再试试。",
        AudioError::UnsupportedFile => "不支持这种音频格式。可以试试 mp3、wav、ogg、flac 或 m4a。",
        AudioError::FileUnreadable => "这个文件读不出来，可能已损坏或被占用。",
        AudioError::Other => "音频出了点问题，请稍后再试。",
    }
}
