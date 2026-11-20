//! 角色包的选择与加载回报（功能清单第 12 节，换角色）。

use crate::command::CharacterCommand;
use crate::effect::Effect;
use crate::model::pet::CharacterStatus;
use crate::persist::is_valid_character_id;
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: CharacterCommand, fx: &mut Vec<Effect>) {
    match cmd {
        CharacterCommand::Select(id) => {
            if !is_valid_character_id(&id) {
                return;
            }
            let unchanged =
                id == state.settings.character && state.character.status == CharacterStatus::Ready;
            if unchanged {
                return;
            }
            state.settings.character.clone_from(&id);
            state.character.status = CharacterStatus::Loading;
            fx.push(Effect::LoadCharacter { id });
        }
        CharacterCommand::Reload => {
            state.character.status = CharacterStatus::Loading;
            fx.extend([
                Effect::ScanCharacters,
                Effect::LoadCharacter {
                    id: state.settings.character.clone(),
                },
            ]);
        }
        CharacterCommand::Scanned(list) => state.character.available = list,
        CharacterCommand::Loaded {
            id,
            name,
            expressions,
        } => {
            if id == state.settings.character {
                state.character.status = CharacterStatus::Ready;
                state.character.name = name;
                state.expressions.set_catalog(expressions);
            }
        }
        CharacterCommand::LoadFailed { id, message } => {
            if id == state.settings.character {
                state.character.status = CharacterStatus::Failed(message);
            }
        }
    }
}
