//! 外接 Markdown 待办清单的无损解析与回写（功能清单第 10 节）。
//!
//! 只同步首个 `# ` 标题与 `- [ ]` / `- [x]` 任务行，保留文件中的其他标题、空行与普通正文。

use crate::model::todo::{TodoItem, TodoList};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoMdError {
    NotFound,
    ReadFailed,
    WriteFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MdLine {
    Raw(String),
    Heading { prefix: String },
    Task { prefix: String },
}

/// 外部 `.md` 文件的结构模板，用于在保存时保留非任务内容和原有缩进/换行风格。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoMdDoc {
    lines: Vec<MdLine>,
    crlf: bool,
    trailing_newline: bool,
}

/// 解析外部 Markdown 文档，返回 `TodoList` 与用于无损回写的 `TodoMdDoc`。
#[must_use]
pub fn parse_md(content: &str) -> (TodoList, TodoMdDoc) {
    let text = content.strip_prefix('\u{feff}').unwrap_or(content);
    let crlf = text.contains("\r\n");
    let trailing_newline = text.ends_with('\n');
    let mut lines = Vec::new();
    let mut name = String::new();
    let mut has_heading = false;
    let mut items = Vec::new();
    let mut next_id = 1u32;

    for raw_line in text.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if !has_heading && let Some((prefix, title)) = parse_h1(line) {
            has_heading = true;
            name = title.to_owned();
            lines.push(MdLine::Heading {
                prefix: prefix.to_owned(),
            });
            continue;
        }
        if let Some((prefix, done, task_text)) = parse_task_line(line) {
            items.push(TodoItem {
                id: next_id,
                text: task_text.to_owned(),
                done,
            });
            next_id = next_id.saturating_add(1);
            lines.push(MdLine::Task {
                prefix: prefix.to_owned(),
            });
            continue;
        }
        lines.push(MdLine::Raw(line.to_owned()));
    }

    let list = TodoList::from_parts(name, items);
    let doc = TodoMdDoc {
        lines,
        crlf,
        trailing_newline,
    };
    (list, doc)
}

/// 将当前 `TodoList` 渲染为 Markdown 文本。存在 `doc` 时原地保留非清单段落。
#[must_use]
pub fn render_md(doc: Option<&TodoMdDoc>, list: &TodoList) -> String {
    let Some(doc) = doc else {
        return render_fresh_md(list);
    };

    let nl = if doc.crlf { "\r\n" } else { "\n" };
    let mut out_lines: Vec<String> = Vec::with_capacity(doc.lines.len() + list.items().len());
    let mut item_iter = list.items().iter();
    let mut last_task_pos: Option<usize> = None;
    let mut saw_heading = false;

    for line in &doc.lines {
        match line {
            MdLine::Raw(raw) => out_lines.push(raw.clone()),
            MdLine::Heading { prefix } => {
                saw_heading = true;
                if list.name().is_empty() {
                    out_lines.push(format!("{prefix}待办清单"));
                } else {
                    out_lines.push(format!("{prefix}{}", list.name()));
                }
            }
            MdLine::Task { prefix } => {
                if let Some(item) = item_iter.next() {
                    out_lines.push(format_task(prefix, item));
                    last_task_pos = Some(out_lines.len());
                }
            }
        }
    }

    // 多出的新增条目插在最后一条任务之后（或文档末尾）。
    let extra: Vec<String> = item_iter.map(|item| format_task("- ", item)).collect();
    if !extra.is_empty() {
        let insert_at = last_task_pos.unwrap_or(out_lines.len());
        out_lines.splice(insert_at..insert_at, extra);
    }

    if !saw_heading && !list.name().is_empty() {
        out_lines.insert(0, format!("# {}", list.name()));
        if out_lines.len() > 1 && !out_lines[1].is_empty() {
            out_lines.insert(1, String::new());
        }
    }

    let mut rendered = out_lines.join(nl);
    if doc.trailing_newline || rendered.is_empty() {
        rendered.push_str(nl);
    }
    rendered
}

fn render_fresh_md(list: &TodoList) -> String {
    let mut out = String::new();
    let title = if list.name().is_empty() {
        "待办清单"
    } else {
        list.name()
    };
    out.push_str("# ");
    out.push_str(title);
    out.push_str("\n\n");
    for item in list.items() {
        out.push_str(&format_task("- ", item));
        out.push('\n');
    }
    out
}

fn format_task(prefix: &str, item: &TodoItem) -> String {
    let mark = if item.done { 'x' } else { ' ' };
    if item.text.is_empty() {
        format!("{prefix}[{mark}]")
    } else {
        format!("{prefix}[{mark}] {}", item.text)
    }
}

/// 匹配一级标题 `# 标题`（不匹配 `## `）。
fn parse_h1(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("# ")?;
    if rest.starts_with('#') {
        return None;
    }
    Some(("# ", rest.trim()))
}

/// 匹配 `- [ ] ...`、`- [x] ...`、`* [X] ...`（允许行首缩进）。
fn parse_task_line(line: &str) -> Option<(&str, bool, &str)> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = line.get(indent_len..)?;
    let bullet_len = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        2
    } else {
        return None;
    };
    let after_bullet = rest.get(bullet_len..)?;
    let (done, after_box) = if let Some(r) = after_bullet.strip_prefix("[ ]") {
        (false, r)
    } else {
        let r = after_bullet
            .strip_prefix("[x]")
            .or_else(|| after_bullet.strip_prefix("[X]"))?;
        (true, r)
    };
    let text = match after_box.strip_prefix(' ') {
        Some(t) => t,
        None if after_box.is_empty() => "",
        None => return None,
    };
    let prefix = line.get(..indent_len + bullet_len)?;
    Some((prefix, done, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_preserving_prose_and_crlf() {
        let src = "---\r\ntags: [study]\r\n---\r\n\r\n# 今日计划\r\n\r\n> 保持专注\r\n\r\n- [ ] 写高数\r\n* [x] 背单词\r\n\r\n## 随手记\r\n今晚记得早睡。\r\n";
        let (mut list, doc) = parse_md(src);
        assert_eq!(list.name(), "今日计划");
        assert_eq!(list.items().len(), 2);
        assert!(!list.items()[0].done);
        assert_eq!(list.items()[0].text, "写高数");
        assert!(list.items()[1].done);
        assert_eq!(list.items()[1].text, "背单词");

        let first_id = list.items()[0].id;
        list.set_done(first_id, true);
        let new_id = list.add();
        list.set_text(new_id, "复盘错题");

        let out = render_md(Some(&doc), &list);
        let expected = "---\r\ntags: [study]\r\n---\r\n\r\n# 今日计划\r\n\r\n> 保持专注\r\n\r\n- [x] 写高数\r\n* [x] 背单词\r\n- [ ] 复盘错题\r\n\r\n## 随手记\r\n今晚记得早睡。\r\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn fresh_markdown_renders_standard_checklist() {
        let mut list = TodoList::default();
        list.rename("本周目标");
        let id = list.items()[0].id;
        list.set_text(id, "重构设置页");
        list.set_done(id, true);
        let out = render_md(None, &list);
        assert!(out.starts_with("# 本周目标\n\n- [x] 重构设置页\n"));
    }
}
