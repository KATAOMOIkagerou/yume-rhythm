//! 托盘图标与菜单。必须在有消息循环的线程（主线程）上创建。

use std::collections::HashMap;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem as Item, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// 托盘事件。`Menu` 携带菜单项 id。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    Menu(String),
    DoubleClick,
}

/// 托盘菜单项。`checked` 为 `Some` 时显示为勾选项。
#[derive(Clone, Debug)]
pub struct TrayItem {
    pub id: &'static str,
    pub label: String,
    pub checked: Option<bool>,
}

enum Handle {
    Plain(Item),
    Check(CheckMenuItem),
}

pub struct Tray {
    _icon: TrayIcon,
    items: HashMap<&'static str, Handle>,
}

impl Tray {
    /// `icon_rgba` 为直通 RGBA。`None` 项表示分隔线。
    ///
    /// # Errors
    /// 图标数据非法或系统拒绝创建托盘图标。
    pub fn new(
        icon_rgba: Vec<u8>,
        size: u32,
        tooltip: &str,
        entries: &[Option<TrayItem>],
        on_event: impl Fn(TrayEvent) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let mut items = HashMap::new();
        for entry in entries {
            let result = match entry {
                None => menu.append(&PredefinedMenuItem::separator()),
                Some(TrayItem {
                    id,
                    label,
                    checked: Some(on),
                }) => {
                    let item = CheckMenuItem::with_id(*id, label, true, *on, None);
                    let r = menu.append(&item);
                    items.insert(*id, Handle::Check(item));
                    r
                }
                Some(TrayItem {
                    id,
                    label,
                    checked: None,
                }) => {
                    let item = Item::with_id(*id, label, true, None);
                    let r = menu.append(&item);
                    items.insert(*id, Handle::Plain(item));
                    r
                }
            };
            result.map_err(|e| e.to_string())?;
        }
        let icon = Icon::from_rgba(icon_rgba, size, size).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(tooltip)
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        let on_event = std::sync::Arc::new(on_event);
        let menu_handler = on_event.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            menu_handler(TrayEvent::Menu(e.id.0))
        }));
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = e
            {
                on_event(TrayEvent::DoubleClick);
            }
        }));
        Ok(Self { _icon: tray, items })
    }

    pub fn set_label(&self, id: &str, label: &str) {
        match self.items.get(id) {
            Some(Handle::Plain(item)) => item.set_text(label),
            Some(Handle::Check(item)) => item.set_text(label),
            None => {}
        }
    }

    pub fn set_checked(&self, id: &str, checked: bool) {
        if let Some(Handle::Check(item)) = self.items.get(id) {
            item.set_checked(checked);
        }
    }
}
