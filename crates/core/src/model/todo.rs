//! 待办清单（功能清单第 10 节）。改为自动保存：每次修改都会触发持久化。

use serde::{Deserialize, Serialize};

pub const NAME_MAX_CHARS: usize = 40;
pub const TEXT_MAX_CHARS: usize = 60;
const DEFAULT_ITEMS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub done: bool,
}

/// 一份待办清单。至少保留一行。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoList {
    name: String,
    items: Vec<TodoItem>,
}

impl Default for TodoList {
    /// 默认三行空白。
    fn default() -> Self {
        let items = (1..=DEFAULT_ITEMS).map(|id| TodoItem {
            id: id as u32,
            text: String::new(),
            done: false,
        });
        Self {
            name: String::new(),
            items: items.collect(),
        }
    }
}

impl TodoList {
    /// 从持久化数据重建，并修正越界或重复的内容。
    #[must_use]
    pub fn from_parts(name: String, items: Vec<TodoItem>) -> Self {
        let mut list = Self {
            name: truncate(&name, NAME_MAX_CHARS),
            items: Vec::new(),
        };
        for mut item in items {
            if list.items.iter().any(|i| i.id == item.id) {
                item.id = list.next_id();
            }
            item.text = truncate(&item.text, TEXT_MAX_CHARS);
            list.items.push(item);
        }
        if list.items.is_empty() {
            return Self {
                name: list.name,
                ..Self::default()
            };
        }
        list
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn items(&self) -> &[TodoItem] {
        &self.items
    }

    /// 是否允许删除行（多于一行时）。
    #[must_use]
    pub fn can_remove(&self) -> bool {
        self.items.len() > 1
    }

    /// 以下修改方法都返回是否有变化，有变化才需要保存。
    pub fn rename(&mut self, name: &str) -> bool {
        replace(&mut self.name, truncate(name, NAME_MAX_CHARS))
    }

    pub fn set_text(&mut self, id: u32, text: &str) -> bool {
        self.item_mut(id)
            .is_some_and(|item| replace(&mut item.text, truncate(text, TEXT_MAX_CHARS)))
    }

    pub fn set_done(&mut self, id: u32, done: bool) -> bool {
        self.item_mut(id)
            .is_some_and(|item| replace(&mut item.done, done))
    }

    /// 在末尾添加一行空白，返回新行 id。
    pub fn add(&mut self) -> u32 {
        let id = self.next_id();
        self.items.push(TodoItem {
            id,
            text: String::new(),
            done: false,
        });
        id
    }

    pub fn remove(&mut self, id: u32) -> bool {
        if !self.can_remove() {
            return false;
        }
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() != before
    }

    fn item_mut(&mut self, id: u32) -> Option<&mut TodoItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    fn next_id(&self) -> u32 {
        self.items
            .iter()
            .map(|i| i.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    let changed = *slot != value;
    *slot = value;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_three_blank_rows() {
        let list = TodoList::default();
        assert_eq!(list.items().len(), 3);
        assert!(list.items().iter().all(|i| i.text.is_empty() && !i.done));
    }

    #[test]
    fn keeps_at_least_one_row() {
        let mut list = TodoList::default();
        let ids: Vec<u32> = list.items().iter().map(|i| i.id).collect();
        assert!(list.remove(ids[0]));
        assert!(list.remove(ids[1]));
        assert!(!list.can_remove());
        assert!(!list.remove(ids[2]));
        assert_eq!(list.items().len(), 1);
    }

    #[test]
    fn limits_are_counted_in_characters() {
        let mut list = TodoList::default();
        list.rename(&"清".repeat(50));
        assert_eq!(list.name().chars().count(), NAME_MAX_CHARS);
        let id = list.add();
        list.set_text(id, &"字".repeat(80));
        assert_eq!(
            list.items().last().map(|i| i.text.chars().count()),
            Some(TEXT_MAX_CHARS)
        );
    }

    #[test]
    fn from_parts_repairs_duplicates_and_empty() {
        let dup = vec![
            TodoItem {
                id: 1,
                text: "a".into(),
                done: false,
            },
            TodoItem {
                id: 1,
                text: "b".into(),
                done: true,
            },
        ];
        let list = TodoList::from_parts(String::new(), dup);
        assert_ne!(list.items()[0].id, list.items()[1].id);
        assert_eq!(TodoList::from_parts("x".into(), vec![]).items().len(), 3);
    }

    #[test]
    fn unchanged_edits_report_no_change() {
        let mut list = TodoList::default();
        let id = list.items()[0].id;
        assert!(!list.set_text(id, ""));
        assert!(list.set_done(id, true));
        assert!(!list.set_done(id, true));
    }
}
