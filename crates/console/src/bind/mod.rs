//! 把 `View` 写进 Slint，把界面回调翻译成 `Command`。
//!
//! 只推送有变化的部分；列表原地更新（不整体替换），避免正在输入的待办失去焦点。

mod callbacks;
mod push;

use crate::ui::{self, ConsoleWindow, Store, Theme};
use crate::view::{self, View};
use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use yume_core::command::PetCommand;
use yume_core::{AppState, Command, WallMs};

/// 控制台窗口。只能在 Slint 事件循环所在的线程上使用。
pub struct Console {
    window: ConsoleWindow,
    /// 最近一次推送的内容。回调用它把下标换成 id。
    shown: Rc<RefCell<Option<View>>>,
    models: Models,
}

struct Models {
    todo_items: Rc<VecModel<ui::TodoRow>>,
    base: Rc<VecModel<ui::ExpressionChip>>,
    accessory: Rc<VecModel<ui::ExpressionChip>>,
    event_columns: Rc<VecModel<SharedString>>,
    event_rows: Rc<VecModel<ui::EventRow>>,
    devices: Rc<VecModel<SharedString>>,
    characters: Rc<VecModel<SharedString>>,
    theme_presets: Rc<VecModel<ui::ThemePresetOption>>,
    shortcuts: Rc<VecModel<ui::ShortcutItem>>,
    fonts: Rc<VecModel<SharedString>>,
}

impl Console {
    /// 创建窗口（不显示）。`dispatch` 收到用户操作产生的命令。
    ///
    /// # Errors
    /// 无法创建窗口（例如没有可用的图形后端）时返回错误。
    pub fn new(dispatch: Rc<dyn Fn(Command)>) -> Result<Self, slint::PlatformError> {
        let window = ConsoleWindow::new()?;
        let models = Models {
            todo_items: Rc::new(VecModel::default()),
            base: Rc::new(VecModel::default()),
            accessory: Rc::new(VecModel::default()),
            event_columns: Rc::new(VecModel::default()),
            event_rows: Rc::new(VecModel::default()),
            devices: Rc::new(VecModel::default()),
            characters: Rc::new(VecModel::default()),
            theme_presets: Rc::new(VecModel::default()),
            shortcuts: Rc::new(VecModel::default()),
            fonts: Rc::new(VecModel::default()),
        };
        let store = window.global::<Store>();
        store.set_todo_items(ModelRc::from(models.todo_items.clone()));
        store.set_base_expressions(ModelRc::from(models.base.clone()));
        store.set_accessory_expressions(ModelRc::from(models.accessory.clone()));
        store.set_event_columns(ModelRc::from(models.event_columns.clone()));
        store.set_event_rows(ModelRc::from(models.event_rows.clone()));
        store.set_devices(ModelRc::from(models.devices.clone()));
        store.set_characters(ModelRc::from(models.characters.clone()));
        store.set_theme_presets(ModelRc::from(models.theme_presets.clone()));
        store.set_shortcuts(ModelRc::from(models.shortcuts.clone()));
        store.set_fonts(ModelRc::from(models.fonts.clone()));

        let shown = Rc::new(RefCell::new(None));
        callbacks::wire(&store, &dispatch, &shown);
        let close = dispatch.clone();
        window.window().on_close_requested(move || {
            close(Command::Pet(PetCommand::HideConsole));
            CloseRequestResponse::HideWindow
        });
        Ok(Self {
            window,
            shown,
            models,
        })
    }

    #[must_use]
    pub fn window(&self) -> &ConsoleWindow {
        &self.window
    }

    /// 显示或隐藏窗口。
    ///
    /// # Errors
    /// 窗口系统拒绝时返回错误。
    pub fn set_visible(&self, visible: bool) -> Result<(), slint::PlatformError> {
        if visible {
            self.window.show()
        } else {
            self.window.hide()
        }
    }

    /// 控制台的原生窗口句柄（Windows 上是 HWND）。窗口显示过之后才有。
    #[must_use]
    pub fn native_handle(&self) -> Option<isize> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let handle = self.window.window().window_handle();
        match handle.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        }
    }

    /// 界面字体。默认是 Windows 的微软雅黑。
    pub fn set_font_family(&self, family: &str) {
        self.window.global::<Theme>().set_font_family(family.into());
    }

    /// 切换到第几个标签页（0 专注，1 表情，2 音乐，3 设置）。
    pub fn set_tab(&self, tab: i32) {
        self.window.set_tab(tab);
    }

    /// 按当前状态刷新界面。
    pub fn render(&self, state: &AppState, now: WallMs) {
        let next = view::project(state, now);
        let prev = self.shown.borrow_mut().take().unwrap_or_default();
        let store = self.window.global::<Store>();
        let theme = self.window.global::<Theme>();
        let m = &self.models;

        if prev.palette != next.palette
            && let Some(palette) = next.palette
        {
            push::apply_palette(&theme, palette);
        }
        if prev.font_family != next.font_family && !next.font_family.is_empty() {
            theme.set_font_family(next.font_family.as_str().into());
        }
        if prev.header != next.header {
            store.set_header(push::header(&next));
        }
        if prev.focus != next.focus {
            store.set_focus(push::focus(&next));
        }
        if prev.todo != next.todo {
            store.set_todo(push::todo(&next));
        }
        if prev.todo_items != next.todo_items {
            sync(&m.todo_items, next.todo_items.iter().map(push::todo_row));
        }
        if prev.emotion != next.emotion {
            store.set_emotion(push::emotion(&next));
        }
        if prev.base_expressions != next.base_expressions {
            sync(&m.base, next.base_expressions.iter().map(push::chip));
        }
        if prev.accessory_expressions != next.accessory_expressions {
            sync(
                &m.accessory,
                next.accessory_expressions.iter().map(push::chip),
            );
        }
        if prev.event_columns != next.event_columns {
            sync(
                &m.event_columns,
                next.event_columns.iter().map(SharedString::from),
            );
        }
        if prev.event_rows != next.event_rows {
            sync(&m.event_rows, next.event_rows.iter().map(push::event_row));
        }
        if prev.devices != next.devices {
            sync(&m.devices, next.devices.iter().map(SharedString::from));
        }
        if prev.music != next.music {
            store.set_music(push::music(&next));
        }
        if prev.characters != next.characters {
            sync(
                &m.characters,
                next.characters.iter().map(SharedString::from),
            );
        }
        if prev.theme_presets != next.theme_presets {
            sync(
                &m.theme_presets,
                next.theme_presets.iter().map(push::theme_preset),
            );
        }
        if prev.shortcuts != next.shortcuts {
            sync(&m.shortcuts, next.shortcuts.iter().map(push::shortcut_item));
        }
        if prev.fonts != next.fonts {
            sync(&m.fonts, next.fonts.iter().map(SharedString::from));
        }
        if prev.settings != next.settings {
            store.set_settings(push::settings(&next));
        }
        if prev.dialog != next.dialog {
            store.set_dialog(next.dialog);
        }
        *self.shown.borrow_mut() = Some(next);
    }
}

/// 原地更新列表：只改变化的行，多删少补。
fn sync<T: Clone + PartialEq + 'static>(model: &VecModel<T>, rows: impl Iterator<Item = T>) {
    let mut len = 0;
    for (i, row) in rows.enumerate() {
        len = i + 1;
        if i < model.row_count() {
            if model.row_data(i).as_ref() != Some(&row) {
                model.set_row_data(i, row);
            }
        } else {
            model.push(row);
        }
    }
    while model.row_count() > len {
        model.remove(model.row_count() - 1);
    }
}
