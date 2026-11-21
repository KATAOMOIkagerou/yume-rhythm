//! 待办清单（功能清单第 10 节）。内置保存由组装层比较 `persisted()` 自动完成，
//! 外接 Markdown 清单同步由 `todo_md_dirty_seq` 驱动自动回写。

use crate::command::TodoCommand;
use crate::effect::Effect;
use crate::model::todo_md::parse_md;
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: TodoCommand, fx: &mut Vec<Effect>) {
    match cmd {
        TodoCommand::Rename(name) => {
            if state.todo.rename(&name) {
                mark_md_dirty(state);
            }
        }
        TodoCommand::Add => {
            state.todo.add();
            mark_md_dirty(state);
        }
        TodoCommand::Remove(id) => {
            if state.todo.remove(id) {
                mark_md_dirty(state);
            }
        }
        TodoCommand::SetText { id, text } => {
            if state.todo.set_text(id, &text) {
                mark_md_dirty(state);
            }
        }
        TodoCommand::SetDone { id, done } => {
            if state.todo.set_done(id, done) {
                mark_md_dirty(state);
            }
        }
        TodoCommand::PickImportMd => {
            state.todo_md_error = None;
            fx.push(Effect::PickTodoMdImport);
        }
        TodoCommand::PickExportMd => {
            state.todo_md_error = None;
            fx.push(Effect::PickTodoMdExport);
        }
        TodoCommand::ReloadMd => {
            if let Some(path) = state.settings.todo_md_path.clone() {
                state.todo_md_error = None;
                fx.push(Effect::LoadTodoMd { path });
            }
        }
        TodoCommand::UnbindMd => {
            state.settings.todo_md_path = None;
            state.todo_md_doc = None;
            state.todo_md_error = None;
        }
        TodoCommand::MdLoaded { path, content } => {
            let (list, doc) = parse_md(&content);
            state.todo = list;
            state.todo_md_doc = Some(doc);
            state.settings.todo_md_path = Some(path);
            state.todo_md_error = None;
        }
        TodoCommand::MdBound { path } => {
            state.settings.todo_md_path = Some(path);
            state.todo_md_doc = None;
            state.todo_md_error = None;
            state.todo_md_dirty_seq = state.todo_md_dirty_seq.saturating_add(1);
        }
        TodoCommand::MdFailed(err) => {
            state.todo_md_error = Some(err);
        }
    }
}

fn mark_md_dirty(state: &mut AppState) {
    if state.settings.todo_md_path.is_some() {
        state.todo_md_dirty_seq = state.todo_md_dirty_seq.saturating_add(1);
    }
}
