//! 表情选择与随机事件开关（功能清单第 6、7 节）。

use super::split_expressions;
use crate::command::ExpressionCommand;
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: ExpressionCommand) {
    match cmd {
        ExpressionCommand::Select(id) => {
            let (expressions, situation) = split_expressions(state);
            expressions.select(&id, &situation);
        }
        ExpressionCommand::SetEventSwitch { context, event, on } => {
            state.switches.set(context, event, on);
        }
    }
}
