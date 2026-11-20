//! 编译 `ui/app.slint`。固定用 fluent-light 风格，不随系统深浅色变化（配色由 `theme.slint` 决定）。

fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-light".into());
    if let Err(e) = slint_build::compile_with_config("ui/app.slint", config) {
        // 构建脚本只能通过 panic 报错。
        #[allow(clippy::panic, reason = "构建脚本失败时必须中止编译")]
        {
            panic!("Slint 编译失败：{e}");
        }
    }
}
