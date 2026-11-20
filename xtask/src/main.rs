//! 开发检查入口：`cargo xtask check`。规则说明见 docs/架构.md 第 12 节。
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask 是命令行工具，需要输出"
)]

mod arch;
mod lines;

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 在 Linux 上可以编译运行测试的 crate。其余 crate 只能在 Windows 上运行，
/// Linux 上用 `--target x86_64-pc-windows-msvc` 做类型检查。
const HOST_CRATES: &[&str] = &["yume-core", "yume-beat", "yume-character", "xtask"];
const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";

fn main() -> Result<()> {
    let root = workspace_root()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("check") => check(&root),
        Some("arch") => arch::check(&root),
        Some("lines") => lines::check(&root),
        Some(other) => bail!("未知子命令：{other}。可用：check、arch、lines"),
    }
}

fn check(root: &Path) -> Result<()> {
    step("格式", || cargo(root, &["fmt", "--all", "--check"]))?;
    step("依赖方向", || arch::check(root))?;
    step("文件长度", || lines::check(root))?;
    if cfg!(windows) {
        step("clippy", || {
            cargo(
                root,
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            )
        })?;
        step("测试", || cargo(root, &["test", "--workspace"]))?;
    } else {
        let mut clippy = vec!["clippy", "--all-targets"];
        let mut test = vec!["test"];
        for name in HOST_CRATES {
            clippy.extend(["-p", name]);
            test.extend(["-p", name]);
        }
        clippy.extend(["--", "-D", "warnings"]);
        step("clippy（本机）", || cargo(root, &clippy))?;
        step("clippy（Windows 目标，仅类型检查）", || {
            cargo(
                root,
                &[
                    "clippy",
                    "--workspace",
                    "--target",
                    WINDOWS_TARGET,
                    "--",
                    "-D",
                    "warnings",
                ],
            )
        })?;
        step("测试（本机）", || cargo(root, &test))?;
    }
    println!("\n全部检查通过。");
    Ok(())
}

fn step(name: &str, run: impl FnOnce() -> Result<()>) -> Result<()> {
    println!("\n== {name}");
    run().with_context(|| format!("检查未通过：{name}"))
}

fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo).args(args).current_dir(root).status()?;
    if !status.success() {
        bail!("cargo {} 失败", args.join(" "));
    }
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.parent()
        .map(Path::to_path_buf)
        .context("找不到工作区根目录")
}
