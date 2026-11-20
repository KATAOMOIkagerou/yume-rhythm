//! 依赖方向检查。白名单必须与 docs/架构.md 第 2 节的表格一致，改这里就要改文档。

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// 每个内部 crate 允许依赖的内部 crate。
const ALLOWED: &[(&str, &[&str])] = &[
    ("yume-core", &[]),
    ("yume-beat", &[]),
    ("yume-character", &["yume-core", "yume-beat"]),
    ("yume-win", &[]),
    ("yume-audio", &[]),
    ("yume-console", &["yume-core"]),
    (
        "yume-app",
        &[
            "yume-core",
            "yume-beat",
            "yume-character",
            "yume-win",
            "yume-audio",
            "yume-console",
        ],
    ),
    ("xtask", &[]),
];

/// L0 纯逻辑 crate：必须 `#![forbid(unsafe_code)]`。
const PURE: &[&str] = &["yume-core", "yume-beat", "yume-character"];

pub fn check(root: &Path) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        bail!("cargo metadata 失败");
    }
    let meta: Value = serde_json::from_slice(&output.stdout)?;
    let packages = meta["packages"]
        .as_array()
        .context("metadata 缺少 packages")?;
    let internal: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();

    let mut errors = Vec::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default();
        let Some((_, allowed)) = ALLOWED.iter().find(|(n, _)| *n == name) else {
            errors.push(format!(
                "{name}：新 crate 未登记。先在 docs/架构.md 第 2 节和 xtask/src/arch.rs 登记"
            ));
            continue;
        };
        for dep in package["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            if internal.contains(&dep_name) && !allowed.contains(&dep_name) {
                errors.push(format!(
                    "{name} 不允许依赖 {dep_name}（见 docs/架构.md 第 2 节）"
                ));
            }
        }
        if PURE.contains(&name) {
            check_forbid_unsafe(package, name, &mut errors)?;
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }
    println!("依赖方向符合 docs/架构.md 第 2 节。");
    Ok(())
}

fn check_forbid_unsafe(package: &Value, name: &str, errors: &mut Vec<String>) -> Result<()> {
    let manifest = package["manifest_path"]
        .as_str()
        .context("缺少 manifest_path")?;
    let lib = Path::new(manifest).with_file_name("src").join("lib.rs");
    let text = std::fs::read_to_string(&lib).with_context(|| format!("读取 {}", lib.display()))?;
    if !text.contains("#![forbid(unsafe_code)]") {
        errors.push(format!(
            "{name}：L0 crate 的 lib.rs 必须写 #![forbid(unsafe_code)]"
        ));
    }
    Ok(())
}
