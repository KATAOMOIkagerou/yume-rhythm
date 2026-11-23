//! 表情编辑器状态迁移（功能清单第 6、7 节）。

use super::apply_rules;
use crate::command::EditorCommand;
use crate::effect::Effect;
use crate::model::editor::{
    AssetChoice, CustomExpressionSpec, Draft, DraftTarget, EditorError, next_custom_id,
    validate_draft,
};
use crate::model::events::EventContext;
use crate::model::expr_rules::{Conditions, ExpressionRule, clamp_duration_ms, default_rule};
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: EditorCommand, fx: &mut Vec<Effect>) {
    match cmd {
        EditorCommand::Open => state.editor.open = true,
        EditorCommand::Close => {
            state.editor.open = false;
            state.editor.draft = None;
            state.editor.saving = false;
        }
        EditorCommand::NewDraft => {
            state.editor.open = true;
            state.editor.saving = false;
            state.editor.draft = Some(Draft::new_blank());
        }
        EditorCommand::EditExisting(id) => {
            if let Some(meta) = state.expressions.find(&id) {
                state.editor.open = true;
                state.editor.saving = false;
                state.editor.draft = Some(Draft::from_existing(meta));
            }
        }
        EditorCommand::CancelDraft => {
            state.editor.draft = None;
            state.editor.saving = false;
        }
        EditorCommand::SetKind(kind) => {
            if let Some(draft) = &mut state.editor.draft
                && draft.target == DraftTarget::New
            {
                draft.kind = kind;
                draft.error = None;
            }
        }
        EditorCommand::SetName(name) => {
            if let Some(draft) = &mut state.editor.draft
                && !draft.builtin
            {
                draft.name = name;
                draft.error = None;
            }
        }
        EditorCommand::PickAsset(slot) => {
            if let Some(draft) = &mut state.editor.draft
                && !draft.builtin
            {
                draft.error = None;
                fx.push(Effect::PickExpressionAsset { slot });
            }
        }
        EditorCommand::AssetPicked { slot, path } => {
            if let Some(draft) = &mut state.editor.draft
                && !draft.builtin
            {
                *draft.slot_mut(slot) = Some(AssetChoice::Import(path));
                draft.error = None;
            }
        }
        EditorCommand::AssetRejected(msg) => {
            if let Some(draft) = &mut state.editor.draft {
                draft.error = Some(EditorError::AssetRejected(msg));
            }
        }
        EditorCommand::ClearAsset(slot) => {
            if let Some(draft) = &mut state.editor.draft
                && !draft.builtin
            {
                *draft.slot_mut(slot) = None;
                draft.error = None;
            }
        }
        EditorCommand::SetCondition { context, on } => {
            if let Some(draft) = &mut state.editor.draft
                && !(draft.doze_locked && context == EventContext::Music)
            {
                draft.conditions.set(context, on);
            }
        }
        EditorCommand::SetRandom(on) => {
            if let Some(draft) = &mut state.editor.draft {
                draft.random = on;
            }
        }
        EditorCommand::SetDurationMs(ms) => {
            if let Some(draft) = &mut state.editor.draft {
                draft.duration_ms = clamp_duration_ms(ms);
            }
        }
        EditorCommand::ResetRule => reset_draft_rule(state),
        EditorCommand::Save => save_draft(state, fx),
        EditorCommand::Delete(id) => {
            let is_custom = state.raw_catalog.iter().any(|m| m.id == id && m.custom);
            if !is_custom {
                return;
            }
            let character = state.settings.character.clone();
            state.expression_rules.remove(&character, &id);
            if state
                .editor
                .draft
                .as_ref()
                .is_some_and(|d| matches!(&d.target, DraftTarget::Existing(cur) if cur == &id))
            {
                state.editor.draft = None;
                state.editor.saving = false;
            }
            fx.push(Effect::DeleteCustomExpression { character, id });
        }
        EditorCommand::Saved => {
            state.editor.draft = None;
            state.editor.saving = false;
        }
        EditorCommand::SaveFailed(msg) => {
            state.editor.saving = false;
            if let Some(draft) = &mut state.editor.draft {
                draft.error = Some(EditorError::SaveFailed(msg));
            }
        }
    }
}

fn reset_draft_rule(state: &mut AppState) {
    let Some(draft) = &mut state.editor.draft else {
        return;
    };
    match &draft.target {
        DraftTarget::New => {
            draft.conditions = Conditions::ALL_ON;
            draft.random = true;
            draft.duration_ms = 3200;
        }
        DraftTarget::Existing(id) => {
            if let Some(raw) = state.raw_catalog.iter().find(|m| m.id == *id) {
                let rule = default_rule(raw, &state.switches);
                draft.conditions = rule.conditions;
                draft.random = rule.random;
                draft.duration_ms = rule.duration_ms;
            }
        }
    }
}

fn save_draft(state: &mut AppState, fx: &mut Vec<Effect>) {
    if state.editor.saving {
        return;
    }
    let Some(draft) = state.editor.draft.as_ref() else {
        return;
    };
    if let Err(err) = validate_draft(draft, &state.raw_catalog) {
        if let Some(d) = &mut state.editor.draft {
            d.error = Some(err);
        }
        return;
    }

    let mut conditions = draft.conditions;
    if draft.doze_locked {
        conditions.music = false;
    }
    let duration_ms = clamp_duration_ms(i64::from(draft.duration_ms));
    let rule = ExpressionRule {
        conditions,
        random: draft.random,
        duration_ms,
    };
    let id = match &draft.target {
        DraftTarget::New => next_custom_id(&state.raw_catalog),
        DraftTarget::Existing(existing_id) => existing_id.clone(),
    };
    let character = state.settings.character.clone();
    state.expression_rules.set(&character, &id, rule);

    if draft.builtin {
        state.editor.draft = None;
        state.editor.saving = false;
        apply_rules(state);
        return;
    }

    let spec = CustomExpressionSpec {
        character,
        id,
        name: draft.name.trim().to_owned(),
        kind: draft.kind,
        eyes: draft.eyes.clone(),
        mouth: draft.mouth.clone(),
        overlay: draft.overlay.clone(),
        random: draft.random,
        duration_ms,
    };
    state.editor.saving = true;
    fx.push(Effect::SaveCustomExpression(spec));
}
