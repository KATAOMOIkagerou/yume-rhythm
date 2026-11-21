//! L2 Slint 控制台。把 `AppState` 投影成界面数据，把用户操作翻译成 `Command`。
//!
//! - `view`：纯投影（`AppState` → `View`），可单测，不依赖 Slint。
//! - `bind`：把 `View` 推给 Slint，把回调翻译成 `Command`。
//! - `fonts`：基于 Slint fontique 枚举系统字体并动态注册本地 `.ttf`/`.otf` 字体。
//! - `ui/*.slint`：界面本身，只读数据、只调回调。
//!
//! 界面基于 Slint 构建（Made with Slint），见设置页「关于」。

mod bind;
pub mod fonts;
pub mod view;

#[allow(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::allow_attributes_without_reason,
    clippy::todo,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::dbg_macro,
    reason = "Slint 生成的代码"
)]
mod ui {
    slint::include_modules!();
}

pub use bind::Console;
pub use ui::ConsoleWindow;
