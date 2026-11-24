//! 设置项。默认值和取值范围在 `model::settings`、`model::theme`、`model::shortcuts`。

use crate::clock::WallMs;
use crate::command::{AudioCommand, PetCommand, SettingsCommand};
use crate::effect::Effect;
use crate::model::settings::{
    FONT_SIZE_DEFAULT, clamp_background_dim, clamp_console_opacity, clamp_element_opacity,
    clamp_font_size, clamp_pet_opacity, clamp_pet_scale, clamp_random_interval,
};
use crate::model::shortcuts::{ShortcutAction, Shortcuts};
use crate::model::theme::{
    AccentPreset, BackgroundMode, clamp_corner_radius, is_dark_background, normalize_hex_color,
};
use crate::state::AppState;
use crate::update::{audio, pet};

pub(super) fn handle(
    state: &mut AppState,
    cmd: SettingsCommand,
    now: WallMs,
    fx: &mut Vec<Effect>,
) {
    match cmd {
        SettingsCommand::SetEndSound(on) => state.settings.end_sound = on,
        SettingsCommand::SetRandomEnabled(on) => state.settings.random_enabled = on,
        SettingsCommand::SetRandomInterval(seconds) => {
            state.settings.random_interval_s = clamp_random_interval(seconds);
        }
        SettingsCommand::SetShowBpm(on) => state.settings.show_bpm = on,
        SettingsCommand::SetAutoFollow(on) => state.settings.auto_follow = on,
        SettingsCommand::SetPetScale(scale) => state.settings.pet_scale = clamp_pet_scale(scale),
        SettingsCommand::SetAutostart(on) => state.settings.autostart = on,
        SettingsCommand::ToggleAutostart => state.settings.autostart = !state.settings.autostart,
        SettingsCommand::SetHideOnFullscreen(on) => state.settings.hide_on_fullscreen = on,
        SettingsCommand::SetBackgroundMode(mode) => {
            let prev_dark = is_dark_background(
                state.settings.background,
                state.settings.custom_background.as_deref(),
                state.settings.custom_bg_dark,
            );
            state.settings.background = mode;
            if mode != BackgroundMode::Custom {
                state.settings.custom_background = None;
            }
            state.settings.custom_bg_dark = match mode {
                BackgroundMode::Light => false,
                BackgroundMode::Dark => true,
                BackgroundMode::Custom => prev_dark,
            };
            state.bg_error = false;
        }
        SettingsCommand::SetCustomBackground(raw) => match raw {
            None => {
                state.settings.custom_background = None;
                state.bg_error = false;
            }
            Some(text) if text.trim().is_empty() => {
                state.settings.custom_background = None;
                state.bg_error = false;
            }
            Some(text) => match normalize_hex_color(&text) {
                Some(hex) => {
                    state.settings.custom_bg_dark = is_dark_background(
                        BackgroundMode::Custom,
                        Some(&hex),
                        state.settings.custom_bg_dark,
                    );
                    state.settings.custom_background = Some(hex);
                    state.settings.background = BackgroundMode::Custom;
                    state.bg_error = false;
                }
                None => {
                    state.bg_error = true;
                }
            },
        },
        SettingsCommand::SetBgOpacity(opacity) => {
            state.settings.bg_opacity = clamp_element_opacity(opacity);
        }
        SettingsCommand::SetCardOpacity(opacity) => {
            state.settings.card_opacity = clamp_element_opacity(opacity);
        }
        SettingsCommand::SetAccent(preset) => {
            state.settings.accent = preset;
            if preset != AccentPreset::Custom {
                state.settings.last_preset_accent = preset;
            }
            state.accent_error = false;
        }
        SettingsCommand::SetCustomAccent(raw) => match raw {
            None => {
                state.settings.custom_accent = None;
                state.accent_error = false;
            }
            Some(text) if text.trim().is_empty() => {
                state.settings.custom_accent = None;
                state.accent_error = false;
            }
            Some(text) => match normalize_hex_color(&text) {
                Some(hex) => {
                    state.settings.custom_accent = Some(hex);
                    state.settings.accent = AccentPreset::Custom;
                    state.accent_error = false;
                }
                None => {
                    state.accent_error = true;
                }
            },
        },
        SettingsCommand::SetCornerMode(mode) => {
            state.settings.corner_mode = mode;
        }
        SettingsCommand::SetCustomCornerRadius(px) => {
            state.settings.custom_corner_radius = clamp_corner_radius(px);
        }
        SettingsCommand::PickBackgroundImage => {
            state.bg_image_error = None;
            fx.push(Effect::PickBackgroundImage);
        }
        SettingsCommand::BackgroundImageLoaded { path } => {
            state.settings.background_image = Some(path.clone());
            state.bg_image_error = None;
            fx.push(Effect::ApplyBackgroundImage { path: Some(path) });
        }
        SettingsCommand::BackgroundImageFailed(msg) => {
            state.bg_image_error = Some(msg);
            state.settings.background_image = None;
        }
        SettingsCommand::ClearBackgroundImage => {
            state.settings.background_image = None;
            state.bg_image_error = None;
            fx.push(Effect::ApplyBackgroundImage { path: None });
        }
        SettingsCommand::SetBackgroundDim(dim) => {
            state.settings.background_dim = clamp_background_dim(dim);
        }
        SettingsCommand::SetConsoleOpacity(opacity) => {
            state.settings.console_opacity = clamp_console_opacity(opacity);
        }
        SettingsCommand::SetPetOpacity(opacity) => {
            state.settings.pet_opacity = clamp_pet_opacity(opacity);
        }
        SettingsCommand::SetShortcut { action, binding } => {
            match state.settings.shortcuts.set(action, &binding) {
                Ok(()) => state.shortcut_error = None,
                Err(err) => state.shortcut_error = Some(err),
            }
        }
        SettingsCommand::ClearShortcut(action) => {
            state.settings.shortcuts.clear(action);
            state.shortcut_error = None;
        }
        SettingsCommand::ResetShortcuts => {
            state.settings.shortcuts = Shortcuts::default();
            state.shortcut_error = None;
        }
        SettingsCommand::GlobalHotkeysFailed(failed) => {
            state.global_hotkeys_failed = failed;
        }
        SettingsCommand::ConsoleKeyPressed {
            ctrl,
            alt,
            shift,
            key,
        } => {
            if let Some(action) = state.settings.shortcuts.match_local(ctrl, alt, shift, &key) {
                match action {
                    ShortcutAction::LocalAudioToggle => {
                        audio::handle(state, AudioCommand::Toggle, now, fx);
                    }
                    ShortcutAction::LocalAudioStop => {
                        audio::handle(state, AudioCommand::Stop, now, fx);
                    }
                    ShortcutAction::LocalHideConsole => {
                        pet::handle(state, PetCommand::HideConsole, fx);
                    }
                    ShortcutAction::GlobalToggleConsole
                    | ShortcutAction::GlobalToggleLocked
                    | ShortcutAction::GlobalToggleAudio => {}
                }
            }
        }
        SettingsCommand::SetZhFontFamily(family) => {
            state.settings.zh_font_family = clean_font(family);
            state.font_error = None;
        }
        SettingsCommand::SetEnFontFamily(family) => {
            state.settings.en_font_family = clean_font(family);
            state.font_error = None;
        }
        SettingsCommand::SetFontSize(px) => {
            state.settings.font_size = clamp_font_size(px);
        }
        SettingsCommand::PickCustomFont => {
            state.font_error = None;
            fx.push(Effect::PickFontFile);
        }
        SettingsCommand::ResetFont => {
            state.settings.zh_font_family = None;
            state.settings.en_font_family = None;
            state.settings.font_size = FONT_SIZE_DEFAULT;
            state.settings.custom_font_paths.clear();
            state.font_error = None;
        }
        SettingsCommand::FontsListed(fonts) => {
            state.available_fonts = fonts;
        }
        SettingsCommand::CustomFontLoaded {
            path,
            family,
            has_cjk,
        } => {
            if !state.settings.custom_font_paths.iter().any(|p| p == &path) {
                state.settings.custom_font_paths.push(path);
            }
            if has_cjk {
                state.settings.zh_font_family = Some(family.clone());
            } else {
                state.settings.en_font_family = Some(family.clone());
            }
            if !state.available_fonts.iter().any(|f| f == &family) {
                state.available_fonts.insert(0, family);
            }
            state.font_error = None;
        }
        SettingsCommand::CustomFontFailed(msg) => {
            state.font_error = Some(msg);
        }
    }
}

fn clean_font(family: Option<String>) -> Option<String> {
    family
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}
