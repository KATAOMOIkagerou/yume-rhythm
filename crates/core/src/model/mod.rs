//! 各业务领域的子状态。每个子状态只管自己的规则，跨领域的协调写在 `update`。

pub mod audio;
pub mod events;
pub mod expression;
pub mod pet;
pub mod settings;
pub mod shortcuts;
pub mod theme;
pub mod timer;
pub mod todo;
pub mod todo_md;
