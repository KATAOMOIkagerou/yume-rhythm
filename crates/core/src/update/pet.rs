//! 桌宠窗口与控制台（功能清单第 1–3 节）。窗口属性由组装层根据 `derive::pet_window` 应用。

use crate::command::PetCommand;
use crate::effect::Effect;
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: PetCommand, fx: &mut Vec<Effect>) {
    let pet = &mut state.pet;
    match cmd {
        PetCommand::SetLocked(locked) => pet.locked = locked,
        PetCommand::ToggleLocked => pet.locked = !pet.locked,
        PetCommand::ShowConsole => {
            pet.console_visible = true;
            fx.push(Effect::FocusConsole);
        }
        PetCommand::HideConsole => pet.console_visible = false,
        PetCommand::ToggleConsole => {
            pet.console_visible = !pet.console_visible;
            if pet.console_visible {
                fx.push(Effect::FocusConsole);
            }
        }
        PetCommand::Moved(position) => state.settings.pet_position = Some(position),
        PetCommand::FullscreenChanged(active) => pet.fullscreen_active = active,
        PetCommand::OpenSoundSettings => fx.push(Effect::OpenSoundSettings),
        PetCommand::Quit => fx.push(Effect::Quit),
    }
}
