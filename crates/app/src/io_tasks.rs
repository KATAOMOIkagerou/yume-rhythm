//! 文件选择对话框、外接 Markdown 清单读写与本地字体加载。
//!
//! 只负责系统 I/O 与结果包装，不写业务规则（业务规则在 `yume-core`）。

use std::path::Path;
use yume_core::Command;
use yume_core::command::{SettingsCommand, TodoCommand};
use yume_core::model::todo_md::TodoMdError;

pub fn pick_todo_md_import(owner: Option<isize>) -> Option<Command> {
    let path = yume_win::pick_open_file(owner, "导入 Markdown 待办清单", "Markdown 文档", "*.md")?;
    Some(load_todo_md(&path.to_string_lossy()))
}

pub fn pick_todo_md_export(owner: Option<isize>, list_name: &str) -> Option<Command> {
    let suggested = sanitize_filename(list_name);
    let default_name = if suggested.is_empty() {
        "待办清单.md".to_owned()
    } else {
        format!("{suggested}.md")
    };
    let path = yume_win::pick_save_file(
        owner,
        "另存为外置 Markdown 待办清单",
        &default_name,
        "md",
        "Markdown 文档",
        "*.md",
    )?;
    Some(Command::Todo(TodoCommand::MdBound {
        path: path.to_string_lossy().into_owned(),
    }))
}

#[must_use]
pub fn load_todo_md(path: &str) -> Command {
    match yume_win::storage::read_text(Path::new(path)) {
        Ok(Some(content)) => Command::Todo(TodoCommand::MdLoaded {
            path: path.to_owned(),
            content,
        }),
        Ok(None) => Command::Todo(TodoCommand::MdFailed(TodoMdError::NotFound)),
        Err(e) => {
            log::warn!("读取外接 Markdown 失败（{path}）：{e}");
            Command::Todo(TodoCommand::MdFailed(TodoMdError::ReadFailed))
        }
    }
}

pub fn save_todo_md(path: &str, content: &str) -> Result<(), TodoMdError> {
    yume_win::storage::write_text_atomic(Path::new(path), content).map_err(|e| {
        log::warn!("写入外接 Markdown 失败（{path}）：{e}");
        TodoMdError::WriteFailed
    })
}

pub fn pick_font_file(owner: Option<isize>) -> Option<Command> {
    let path = yume_win::pick_open_file(
        owner,
        "导入本地字体文件",
        "字体文件 (*.ttf;*.otf;*.ttc)",
        "*.ttf;*.otf;*.ttc",
    )?;
    Some(load_custom_font(&path.to_string_lossy()))
}

#[must_use]
pub fn load_custom_font(path: &str) -> Command {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("读取字体文件失败（{path}）：{e}");
            return Command::Settings(SettingsCommand::CustomFontFailed(
                "无法读取该字体文件".into(),
            ));
        }
    };
    match yume_console::fonts::register_font_bytes(bytes) {
        Some(reg) => {
            log::info!("已加载外部字体：{}（{}）", reg.family, path);
            Command::Settings(SettingsCommand::CustomFontLoaded {
                path: path.to_owned(),
                family: reg.family,
                has_cjk: reg.has_cjk,
            })
        }
        None => Command::Settings(SettingsCommand::CustomFontFailed(
            "文件不是有效的 TrueType / OpenType 字体".into(),
        )),
    }
}

fn sanitize_filename(name: &str) -> String {
    name.trim()
        .chars()
        .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .take(40)
        .collect()
}

/// 选择并复制控制台背景图片到 `%APPDATA%\韵梦桌伴\background.<ext>`。
#[must_use]
pub fn pick_background_image(owner: Option<isize>, user_dir: Option<&Path>) -> Option<Command> {
    let src = yume_win::pick_open_file(
        owner,
        "选择控制台背景图片",
        "图片 (*.png;*.jpg;*.jpeg)",
        "*.png;*.jpg;*.jpeg",
    )?;
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| matches!(e.as_str(), "png" | "jpg" | "jpeg"));
    let Some(ext) = ext else {
        return Some(Command::Settings(SettingsCommand::BackgroundImageFailed(
            "仅支持 PNG 或 JPG 图片".into(),
        )));
    };
    let dest = if let Some(dir) = user_dir {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return Some(Command::Settings(SettingsCommand::BackgroundImageFailed(
                format!("创建数据目录失败：{e}"),
            )));
        }
        for old in ["png", "jpg", "jpeg"] {
            let _ = std::fs::remove_file(dir.join(format!("background.{old}")));
        }
        let dest = dir.join(format!("background.{ext}"));
        if src != dest
            && let Err(e) = std::fs::copy(&src, &dest)
        {
            return Some(Command::Settings(SettingsCommand::BackgroundImageFailed(
                format!("复制背景图片失败：{e}"),
            )));
        }
        dest
    } else {
        src
    };
    Some(Command::Settings(SettingsCommand::BackgroundImageLoaded {
        path: dest.to_string_lossy().into_owned(),
    }))
}
