//! 待办清单（功能清单第 10 节）。保存由组装层比较 `persisted()` 自动完成。

use crate::command::TodoCommand;
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: TodoCommand) {
    let todo = &mut state.todo;
    match cmd {
        TodoCommand::Rename(name) => {
            todo.rename(&name);
        }
        TodoCommand::Add => {
            todo.add();
        }
        TodoCommand::Remove(id) => {
            todo.remove(id);
        }
        TodoCommand::SetText { id, text } => {
            todo.set_text(id, &text);
        }
        TodoCommand::SetDone { id, done } => {
            todo.set_done(id, done);
        }
    }
}
