//! 把 `View` 写进 Slint，把界面回调翻译成 `Command`。
//!
//! 只推送有变化的部分；列表原地更新（不整体替换），避免正在输入的待办失去焦点。

mod callbacks;
mod editor;
mod push;

use crate::fonts::apply_font_stack;
use crate::ui::{self, ConsoleWindow, Store, Theme};
use crate::view::{self, View};
use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::Path;
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
    editor_rows: Rc<VecModel<ui::EditorRow>>,
    devices: Rc<VecModel<SharedString>>,
    characters: Rc<VecModel<SharedString>>,
    accent_presets: Rc<VecModel<ui::AccentOption>>,
    shortcuts: Rc<VecModel<ui::ShortcutItem>>,
    zh_fonts: Rc<VecModel<SharedString>>,
    en_fonts: Rc<VecModel<SharedString>>,
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
            editor_rows: Rc::new(VecModel::default()),
            devices: Rc::new(VecModel::default()),
            characters: Rc::new(VecModel::default()),
            accent_presets: Rc::new(VecModel::default()),
            shortcuts: Rc::new(VecModel::default()),
            zh_fonts: Rc::new(VecModel::default()),
            en_fonts: Rc::new(VecModel::default()),
        };
        let store = window.global::<Store>();
        store.set_todo_items(ModelRc::from(models.todo_items.clone()));
        store.set_base_expressions(ModelRc::from(models.base.clone()));
        store.set_accessory_expressions(ModelRc::from(models.accessory.clone()));
        store.set_editor_rows(ModelRc::from(models.editor_rows.clone()));
        store.set_devices(ModelRc::from(models.devices.clone()));
        store.set_characters(ModelRc::from(models.characters.clone()));
        store.set_accent_presets(ModelRc::from(models.accent_presets.clone()));
        store.set_shortcuts(ModelRc::from(models.shortcuts.clone()));
        store.set_zh_fonts(ModelRc::from(models.zh_fonts.clone()));
        store.set_en_fonts(ModelRc::from(models.en_fonts.clone()));

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

    /// 设置或清除控制台窗口背景图片。
    ///
    /// # Errors
    /// 图片文件不存在或无法解码时返回中文错误。
    pub fn set_background_image(&self, path: Option<&str>) -> Result<(), String> {
        let theme = self.window.global::<Theme>();
        match path {
            None => {
                theme.set_has_bg_image(false);
                theme.set_bg_image(slint::Image::default());
                Ok(())
            }
            Some(p) => {
                let image = slint::Image::load_from_path(Path::new(p))
                    .map_err(|e| format!("无法加载背景图片：{e}"))?;
                theme.set_bg_image(image);
                theme.set_has_bg_image(true);
                Ok(())
            }
        }
    }

    /// 切换到第几个标签页（0 专注，1 表情，2 音乐，3 设置）。
    pub fn set_tab(&self, tab: i32) {
        self.window.set_tab(tab);
    }

    /// 按当前状态刷新界面。
    pub fn render(&self, state: &AppState, now: WallMs) {
        let next = view::project(state, now);
        let first_render = self.shown.borrow().is_none();
        let prev = self.shown.borrow_mut().take().unwrap_or_default();
        let store = self.window.global::<Store>();
        let theme = self.window.global::<Theme>();
        let m = &self.models;

        if prev.palette != next.palette
            && let Some(palette) = next.palette
        {
            push::apply_palette(&theme, palette);
        }
        if prev.radii != next.radii
            && let Some(radii) = next.radii
        {
            push::apply_radii(&theme, radii);
        }
        if (first_render || prev.font_size != next.font_size) && next.font_size > 0 {
            push::apply_font_size(&theme, next.font_size);
        }
        if first_render || prev.font_pair != next.font_pair {
            let stack_id =
                apply_font_stack(next.font_pair.0.as_deref(), next.font_pair.1.as_deref());
            theme.set_font_family(stack_id.into());
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
        if prev.editor != next.editor {
            store.set_editor(editor::push_editor(&next.editor));
        }
        if prev.editor_rows != next.editor_rows {
            sync(
                &m.editor_rows,
                next.editor_rows.iter().map(editor::push_row),
            );
        }
        if prev.editor_draft != next.editor_draft {
            store.set_editor_draft(editor::push_draft(&next.editor_draft));
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
        if prev.accent_presets != next.accent_presets {
            sync(
                &m.accent_presets,
                next.accent_presets.iter().map(push::accent_preset),
            );
        }
        if prev.shortcuts != next.shortcuts {
            sync(&m.shortcuts, next.shortcuts.iter().map(push::shortcut_item));
        }
        if prev.zh_fonts != next.zh_fonts {
            sync(&m.zh_fonts, next.zh_fonts.iter().map(SharedString::from));
        }
        if prev.en_fonts != next.en_fonts {
            sync(&m.en_fonts, next.en_fonts.iter().map(SharedString::from));
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
