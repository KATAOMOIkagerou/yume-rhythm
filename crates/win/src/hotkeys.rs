//! 全局热键。必须在有消息循环的线程（主线程）上注册。

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;

pub struct Hotkeys {
    _manager: GlobalHotKeyManager,
}

/// 注册热键。`bindings` 为（id，快捷键字符串，如 `"ctrl+alt+KeyC"`）。
/// 返回注册结果与注册失败（多半是被其他程序占用）的 id 列表。
///
/// # Errors
/// 热键管理器创建失败。
pub fn register(
    bindings: &[(&'static str, &str)],
    on_press: impl Fn(&'static str) + Send + Sync + 'static,
) -> Result<(Hotkeys, Vec<&'static str>), String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
    let mut ids = HashMap::new();
    let mut failed = Vec::new();
    for (id, accelerator) in bindings {
        match accelerator.parse::<HotKey>() {
            Ok(hotkey) if manager.register(hotkey).is_ok() => {
                ids.insert(hotkey.id(), *id);
            }
            _ => failed.push(*id),
        }
    }
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state == HotKeyState::Pressed
            && let Some(id) = ids.get(&e.id)
        {
            on_press(id);
        }
    }));
    Ok((Hotkeys { _manager: manager }, failed))
}
