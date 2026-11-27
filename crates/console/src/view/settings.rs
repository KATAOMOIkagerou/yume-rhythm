//! 设置页视图投影：背景模式/透明度/图片、强调色、圆角、窗口透明度、双语字体与字号、快捷键与角色信息。

use super::file_name;
use crate::fonts::{order_en_fonts, order_zh_fonts};
use yume_core::AppState;
use yume_core::model::audio::AudioError;
use yume_core::model::pet::CharacterStatus;
use yume_core::model::shortcuts::{ShortcutAction, ShortcutError, display_label};
use yume_core::model::theme::{AccentPreset, BackgroundMode, CornerMode, Rgb, parse_hex_color};
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
    pub bg_opacity: f32,
    pub bg_opacity_text: String,
    pub card_opacity: f32,
    pub card_opacity_text: String,
    pub background_index: i32,
    pub custom_background: String,
    pub bg_error: bool,
    pub accent_custom_open: bool,
    pub custom_accent: String,
    pub accent_error: bool,
    pub corner_index: i32,
    pub corner_radius: i32,
    pub corner_radius_text: String,
    pub has_bg_image: bool,
    pub bg_image_name: String,
    pub bg_dim: f32,
    pub bg_dim_text: String,
    pub bg_image_error: String,
    pub shortcut_error: String,
    pub zh_font_index: i32,
    pub en_font_index: i32,
    pub zh_font_label: String,
    pub en_font_label: String,
    pub font_size: i32,
    pub font_size_text: String,
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
pub struct AccentPresetView {
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

pub(super) fn project_settings(
    state: &AppState,
    zh_fonts: &[String],
    en_fonts: &[String],
    zh_font_families: &[Option<String>],
    en_font_families: &[Option<String>],
) -> Settings {
    let s = state.settings();
    let c = state.character();
    let (character_status, character_failed) = match &c.status {
        CharacterStatus::Loading => ("正在载入角色…".to_owned(), false),
        CharacterStatus::Ready => (String::new(), false),
        CharacterStatus::Failed(message) => (format!("角色载入失败：{message}"), true),
    };
    let (font_status, font_failed) = if let Some(err) = state.font_error() {
        (format!("字体加载失败：{err}"), true)
    } else if !s.custom_font_paths.is_empty() {
        let names = s
            .custom_font_paths
            .iter()
            .map(|p| file_name(p))
            .collect::<Vec<_>>()
            .join("、");
        (format!("已加载外部字体：{names}"), false)
    } else {
        (String::new(), false)
    };
    let zh_font_index = font_index_of(zh_font_families, s.zh_font_family.as_ref());
    let en_font_index = font_index_of(en_font_families, s.en_font_family.as_ref());
    let zh_font_label = usize::try_from(zh_font_index)
        .ok()
        .and_then(|i| zh_fonts.get(i))
        .cloned()
        .unwrap_or_else(|| "默认字体（微软雅黑 UI）".to_owned());
    let en_font_label = usize::try_from(en_font_index)
        .ok()
        .and_then(|i| en_fonts.get(i))
        .cloned()
        .unwrap_or_else(|| "默认字体（Segoe UI）".to_owned());
    let background_index = match s.background {
        BackgroundMode::Light => 0,
        BackgroundMode::Dark => 1,
        BackgroundMode::Custom => 2,
    };
    let corner_index = match s.corner_mode {
        CornerMode::Round => 0,
        CornerMode::Off => 1,
        CornerMode::Custom => 2,
    };

    Settings {
        end_sound: s.end_sound,
        pet_scale: s.pet_scale,
        pet_scale_text: format!("{:.0}%", s.pet_scale * 100.0),
        pet_opacity: s.pet_opacity,
        pet_opacity_text: format!("{:.0}%", s.pet_opacity * 100.0),
        console_opacity: s.console_opacity,
        console_opacity_text: format!("{:.0}%", s.console_opacity * 100.0),
        bg_opacity: s.bg_opacity,
        bg_opacity_text: format!("{:.0}%", s.bg_opacity * 100.0),
        card_opacity: s.card_opacity,
        card_opacity_text: format!("{:.0}%", s.card_opacity * 100.0),
        background_index,
        custom_background: s.custom_background.clone().unwrap_or_default(),
        bg_error: state.bg_error(),
        accent_custom_open: s.accent == AccentPreset::Custom,
        custom_accent: s.custom_accent.clone().unwrap_or_default(),
        accent_error: state.accent_error(),
        corner_index,
        corner_radius: i32::try_from(s.custom_corner_radius).unwrap_or(14),
        corner_radius_text: format!("{} px", s.custom_corner_radius),
        has_bg_image: s.background_image.is_some(),
        bg_image_name: s
            .background_image
            .as_deref()
            .map(file_name)
            .unwrap_or_default()
            .to_owned(),
        bg_dim: s.background_dim,
        bg_dim_text: format!("{:.0}%", s.background_dim * 100.0),
        bg_image_error: state.bg_image_error().unwrap_or_default().to_owned(),
        shortcut_error: state
            .shortcut_error()
            .map(shortcut_error_text)
            .unwrap_or_default(),
        zh_font_index,
        en_font_index,
        zh_font_label,
        en_font_label,
        font_size: i32::try_from(s.font_size).unwrap_or(13),
        font_size_text: format!("{} px", s.font_size),
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

fn font_index_of(list: &[Option<String>], selected: Option<&String>) -> i32 {
    list.iter()
        .position(|f| f.as_ref() == selected)
        .and_then(|i| i32::try_from(i).ok())
        .unwrap_or(0)
}

pub(super) fn accent_presets(state: &AppState) -> Vec<AccentPresetView> {
    let s = state.settings();
    let fallback = s.last_preset_accent.color_or(Rgb::new(0xd6, 0x55, 0x8c));
    let custom_swatch = s
        .custom_accent
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(fallback);
    AccentPreset::ALL
        .into_iter()
        .map(|preset| AccentPresetView {
            key: preset.key().to_owned(),
            name: accent_name(preset).to_owned(),
            swatch: preset.color_or(custom_swatch),
            active: preset == s.accent,
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

pub(super) fn zh_font_options(state: &AppState) -> (Vec<String>, Vec<Option<String>>) {
    build_font_options(
        "默认字体（微软雅黑 UI）",
        &order_zh_fonts(state.available_fonts()),
        state.settings().zh_font_family.as_deref(),
    )
}

pub(super) fn en_font_options(state: &AppState) -> (Vec<String>, Vec<Option<String>>) {
    build_font_options(
        "默认字体（Segoe UI）",
        &order_en_fonts(state.available_fonts()),
        state.settings().en_font_family.as_deref(),
    )
}

fn build_font_options(
    default_label: &str,
    ordered_fonts: &[String],
    selected: Option<&str>,
) -> (Vec<String>, Vec<Option<String>>) {
    let mut labels = vec![default_label.to_owned()];
    let mut families: Vec<Option<String>> = vec![None];
    for font in ordered_fonts {
        labels.push(font.clone());
        families.push(Some(font.clone()));
    }
    if let Some(sel) = selected
        && !families.iter().any(|f| f.as_deref() == Some(sel))
    {
        labels.push(sel.to_owned());
        families.push(Some(sel.to_owned()));
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

fn accent_name(preset: AccentPreset) -> &'static str {
    match preset {
        AccentPreset::Sakura => "樱花粉",
        AccentPreset::Sky => "晴空蓝",
        AccentPreset::Lavender => "丁香紫",
        AccentPreset::Sunset => "落日橘",
        AccentPreset::Mint => "薄荷绿",
        AccentPreset::Custom => "自定义",
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
