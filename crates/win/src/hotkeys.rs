//! 全局热键。必须在有消息循环的线程（主线程）上注册与重新绑定。

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    ids: Arc<Mutex<HashMap<u32, &'static str>>>,
}

impl Hotkeys {
    /// 卸载旧热键并注册新的一组热键。返回注册失败（多半是被其他程序占用）的 id 列表。
    pub fn rebind(&mut self, bindings: &[(&'static str, &str)]) -> Vec<&'static str> {
        for old in self.registered.drain(..) {
            let _ = self.manager.unregister(old);
        }
        let mut next_ids = HashMap::new();
        let mut failed = Vec::new();
        for (id, accelerator) in bindings {
            if accelerator.is_empty() {
                continue;
            }
            match accelerator.parse::<HotKey>() {
                Ok(hotkey) if self.manager.register(hotkey).is_ok() => {
                    next_ids.insert(hotkey.id(), *id);
                    self.registered.push(hotkey);
                }
                _ => failed.push(*id),
            }
        }
        if let Ok(mut guard) = self.ids.lock() {
            *guard = next_ids;
        }
        failed
    }
}

/// 注册热键。`bindings` 为（id，快捷键字符串，如 `"ctrl+alt+KeyC"`）。
/// 返回热键句柄与注册失败（多半是被其他程序占用）的 id 列表。
///
/// # Errors
/// 热键管理器创建失败。
pub fn register(
    bindings: &[(&'static str, &str)],
    on_press: impl Fn(&'static str) + Send + Sync + 'static,
) -> Result<(Hotkeys, Vec<&'static str>), String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
    let ids = Arc::new(Mutex::new(HashMap::<u32, &'static str>::new()));
    let handler_ids = ids.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state == HotKeyState::Pressed
            && let Ok(guard) = handler_ids.lock()
            && let Some(id) = guard.get(&e.id)
        {
            on_press(id);
        }
    }));
    let mut hotkeys = Hotkeys {
        manager,
        registered: Vec::new(),
        ids,
    };
    let failed = hotkeys.rebind(bindings);
    Ok((hotkeys, failed))
}
