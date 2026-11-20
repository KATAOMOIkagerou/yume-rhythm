//! L0 业务状态与规则。见 docs/架构.md 第 3 节。
//!
//! - `AppState`：全部业务状态，外部只读。
//! - `Command` → `update` → `Effect`：唯一的修改路径。
//! - `derive`：由状态推出的持续性输出（桌宠场景、窗口属性）。
//! - `persist`：持久化数据结构与宽松读取。
#![forbid(unsafe_code)]

pub mod clock;
pub mod command;
pub mod derive;
pub mod effect;
pub mod model;
pub mod persist;
mod state;
mod update;

pub use clock::{Ctx, Random, SplitMix64, WallMs};
pub use command::Command;
pub use effect::Effect;
pub use state::AppState;
pub use update::update;
