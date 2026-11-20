//! 文件长度检查，防止出现巨型文件。上限见 docs/架构.md 第 12 节。

use anyhow::{Result, bail};
use std::path::Path;

const LIMITS: &[(&str, usize)] = &[("rs", 500), ("slint", 400)];

pub fn check(root: &Path) -> Result<()> {
    let mut errors = Vec::new();
    for dir in ["crates", "xtask"] {
        walk(&root.join(dir), root, &mut errors)?;
    }
    if !errors.is_empty() {
        bail!("以下文件超长，请按职责拆分模块：\n{}", errors.join("\n"));
    }
    println!("文件长度符合要求。");
    Ok(())
}

fn walk(dir: &Path, root: &Path, errors: &mut Vec<String>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, root, errors)?;
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let Some((_, limit)) = LIMITS.iter().find(|(e, _)| *e == ext) else {
            continue;
        };
        let count = std::fs::read_to_string(&path)?.lines().count();
        if count > *limit {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            errors.push(format!("  {}：{count} 行，上限 {limit}", rel.display()));
        }
    }
    Ok(())
}
