//! 设置项。默认值和取值范围在 `model::settings`、`model::theme`、`model::shortcuts`。

use crate::clock::WallMs;
use crate::command::{AudioCommand, PetCommand, SettingsCommand};
use crate::effect::Effect;
use crate::model::settings::{
    clamp_console_opacity, clamp_pet_opacity, clamp_pet_scale, clamp_random_interval,
};
use crate::model::shortcuts::{ShortcutAction, Shortcuts};
use crate::model::theme::normalize_hex_color;
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
        SettingsCommand::SetTheme(preset) => {
            state.settings.theme = preset;
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
                    state.accent_error = false;
                }
                None => {
                    state.accent_error = true;
                }
            },
        },
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
        SettingsCommand::SetFontFamily(family) => {
            let cleaned = family
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty());
            state.settings.font_family = cleaned;
            state.font_error = None;
        }
        SettingsCommand::PickCustomFont => {
            state.font_error = None;
            fx.push(Effect::PickFontFile);
        }
        SettingsCommand::ResetFont => {
            state.settings.font_family = None;
            state.settings.custom_font_path = None;
            state.font_error = None;
        }
        SettingsCommand::FontsListed(fonts) => {
            state.available_fonts = fonts;
        }
        SettingsCommand::CustomFontLoaded { path, family } => {
            state.settings.custom_font_path = Some(path);
            state.settings.font_family = Some(family.clone());
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
