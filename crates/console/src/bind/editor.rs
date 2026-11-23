//! 表情编辑器的 Slint 推送与回调绑定。

use crate::ui::{self, Store};
use crate::view::{EditorDraftView, EditorRow, EditorView};
use std::rc::Rc;
use yume_core::Command;
use yume_core::command::EditorCommand;
use yume_core::model::editor::AssetSlot;
use yume_core::model::events::EventContext;
use yume_core::model::expression::ExpressionKind;

pub(super) fn push_editor(v: &EditorView) -> ui::EditorView {
    ui::EditorView {
        open: v.open,
        warning: v.warning.as_str().into(),
        canvas_hint: v.canvas_hint.as_str().into(),
    }
}

pub(super) fn push_row(r: &EditorRow) -> ui::EditorRow {
    ui::EditorRow {
        id: r.id.as_str().into(),
        name: r.name.as_str().into(),
        builtin: r.builtin,
        primary: r.primary,
        summary: r.summary.as_str().into(),
    }
}

pub(super) fn push_draft(d: &EditorDraftView) -> ui::EditorDraftView {
    ui::EditorDraftView {
        active: d.active,
        is_new: d.is_new,
        builtin: d.builtin,
        primary: d.primary,
        name: d.name.as_str().into(),
        eyes_text: d.eyes_text.as_str().into(),
        mouth_text: d.mouth_text.as_str().into(),
        overlay_text: d.overlay_text.as_str().into(),
        has_eyes: d.has_eyes,
        has_mouth: d.has_mouth,
        has_overlay: d.has_overlay,
        chill: d.chill,
        study: d.study,
        music: d.music,
        music_locked: d.music_locked,
        random: d.random,
        duration_s: d.duration_s,
        error: d.error.as_str().into(),
        saving: d.saving,
        title: d.title.as_str().into(),
    }
}

pub(super) fn wire(store: &Store<'_>, dispatch: &Rc<dyn Fn(Command)>) {
    let send = |f: fn() -> EditorCommand| {
        let d = dispatch.clone();
        move || d(Command::Editor(f()))
    };
    store.on_editor_open(send(|| EditorCommand::Open));
    store.on_editor_close(send(|| EditorCommand::Close));
    store.on_editor_new(send(|| EditorCommand::NewDraft));
    let d = dispatch.clone();
    store.on_editor_edit(move |id| d(Command::Editor(EditorCommand::EditExisting(id.into()))));
    store.on_editor_cancel(send(|| EditorCommand::CancelDraft));
    let d = dispatch.clone();
    store.on_editor_set_kind(move |primary| {
        let kind = if primary {
            ExpressionKind::Base
        } else {
            ExpressionKind::Accessory
        };
        d(Command::Editor(EditorCommand::SetKind(kind)));
    });
    let d = dispatch.clone();
    store.on_editor_set_name(move |name| d(Command::Editor(EditorCommand::SetName(name.into()))));
    let d = dispatch.clone();
    store.on_editor_pick_asset(move |slot| {
        if let Some(s) = asset_slot(slot) {
            d(Command::Editor(EditorCommand::PickAsset(s)));
        }
    });
    let d = dispatch.clone();
    store.on_editor_clear_asset(move |slot| {
        if let Some(s) = asset_slot(slot) {
            d(Command::Editor(EditorCommand::ClearAsset(s)));
        }
    });
    let d = dispatch.clone();
    store.on_editor_set_condition(move |ctx, on| {
        if let Some(context) = event_context(ctx) {
            d(Command::Editor(EditorCommand::SetCondition { context, on }));
        }
    });
    let d = dispatch.clone();
    store.on_editor_set_random(move |on| d(Command::Editor(EditorCommand::SetRandom(on))));
    let d = dispatch.clone();
    store.on_editor_set_duration_s(move |s| {
        d(Command::Editor(EditorCommand::SetDurationMs(
            i64::from(s) * 1000,
        )));
    });
    store.on_editor_reset_rule(send(|| EditorCommand::ResetRule));
    store.on_editor_save(send(|| EditorCommand::Save));
    let d = dispatch.clone();
    store.on_editor_delete(move |id| d(Command::Editor(EditorCommand::Delete(id.into()))));
}

const fn asset_slot(index: i32) -> Option<AssetSlot> {
    match index {
        0 => Some(AssetSlot::Eyes),
        1 => Some(AssetSlot::Mouth),
        2 => Some(AssetSlot::Overlay),
        _ => None,
    }
}

const fn event_context(index: i32) -> Option<EventContext> {
    match index {
        0 => Some(EventContext::Chill),
        1 => Some(EventContext::Study),
        2 => Some(EventContext::Music),
        _ => None,
    }
}
