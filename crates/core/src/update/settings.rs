//! 设置项。默认值和取值范围在 `model::settings`。

use crate::command::SettingsCommand;
use crate::model::settings::{clamp_pet_scale, clamp_random_interval};
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: SettingsCommand) {
    let s = &mut state.settings;
    match cmd {
        SettingsCommand::SetEndSound(on) => s.end_sound = on,
        SettingsCommand::SetRandomEnabled(on) => s.random_enabled = on,
        SettingsCommand::SetRandomInterval(seconds) => {
            s.random_interval_s = clamp_random_interval(seconds);
        }
        SettingsCommand::SetShowBpm(on) => s.show_bpm = on,
        SettingsCommand::SetAutoFollow(on) => s.auto_follow = on,
        SettingsCommand::SetPetScale(scale) => s.pet_scale = clamp_pet_scale(scale),
        SettingsCommand::SetAutostart(on) => s.autostart = on,
        SettingsCommand::ToggleAutostart => s.autostart = !s.autostart,
        SettingsCommand::SetHideOnFullscreen(on) => s.hide_on_fullscreen = on,
    }
}
