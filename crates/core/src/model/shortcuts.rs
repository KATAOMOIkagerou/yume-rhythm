//! 自定义快捷键模型与冲突校验（功能清单第 3 节）。
//!
//! 规范格式：`[Ctrl+][Alt+][Shift+]<Key>`，例如 `Ctrl+Alt+C`、`Space`、`Q`、`F6`；
//! 空字符串表示未设置（仅全局快捷键允许留空禁用，控制台内快捷键也可留空禁用）。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    GlobalToggleConsole,
    GlobalToggleLocked,
    GlobalToggleAudio,
    LocalAudioToggle,
    LocalAudioStop,
    LocalHideConsole,
}

impl ShortcutAction {
    pub const ALL: [Self; 6] = [
        Self::GlobalToggleConsole,
        Self::GlobalToggleLocked,
        Self::GlobalToggleAudio,
        Self::LocalAudioToggle,
        Self::LocalAudioStop,
        Self::LocalHideConsole,
    ];

    pub const GLOBAL: [Self; 3] = [
        Self::GlobalToggleConsole,
        Self::GlobalToggleLocked,
        Self::GlobalToggleAudio,
    ];

    pub const LOCAL: [Self; 3] = [
        Self::LocalAudioToggle,
        Self::LocalAudioStop,
        Self::LocalHideConsole,
    ];

    #[must_use]
    pub const fn is_global(self) -> bool {
        matches!(
            self,
            Self::GlobalToggleConsole | Self::GlobalToggleLocked | Self::GlobalToggleAudio
        )
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::GlobalToggleConsole => "global_toggle_console",
            Self::GlobalToggleLocked => "global_toggle_locked",
            Self::GlobalToggleAudio => "global_toggle_audio",
            Self::LocalAudioToggle => "local_audio_toggle",
            Self::LocalAudioStop => "local_audio_stop",
            Self::LocalHideConsole => "local_hide_console",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutError {
    Invalid,
    NeedModifier,
    Duplicate(ShortcutAction),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcuts {
    pub global_toggle_console: String,
    pub global_toggle_locked: String,
    pub global_toggle_audio: String,
    pub local_audio_toggle: String,
    pub local_audio_stop: String,
    pub local_hide_console: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            global_toggle_console: "Ctrl+Alt+C".into(),
            global_toggle_locked: "Ctrl+Alt+L".into(),
            global_toggle_audio: String::new(),
            local_audio_toggle: "Space".into(),
            local_audio_stop: "Q".into(),
            local_hide_console: "H".into(),
        }
    }
}

impl Shortcuts {
    #[must_use]
    pub fn get(&self, action: ShortcutAction) -> &str {
        match action {
            ShortcutAction::GlobalToggleConsole => &self.global_toggle_console,
            ShortcutAction::GlobalToggleLocked => &self.global_toggle_locked,
            ShortcutAction::GlobalToggleAudio => &self.global_toggle_audio,
            ShortcutAction::LocalAudioToggle => &self.local_audio_toggle,
            ShortcutAction::LocalAudioStop => &self.local_audio_stop,
            ShortcutAction::LocalHideConsole => &self.local_hide_console,
        }
    }

    fn slot_mut(&mut self, action: ShortcutAction) -> &mut String {
        match action {
            ShortcutAction::GlobalToggleConsole => &mut self.global_toggle_console,
            ShortcutAction::GlobalToggleLocked => &mut self.global_toggle_locked,
            ShortcutAction::GlobalToggleAudio => &mut self.global_toggle_audio,
            ShortcutAction::LocalAudioToggle => &mut self.local_audio_toggle,
            ShortcutAction::LocalAudioStop => &mut self.local_audio_stop,
            ShortcutAction::LocalHideConsole => &mut self.local_hide_console,
        }
    }

    /// 设置快捷键，校验语法、全局修饰键要求及同作用域冲突。
    ///
    /// # Errors
    /// 语法非法、全局热键缺少修饰键或与同作用域其他动作重复。
    pub fn set(&mut self, action: ShortcutAction, raw: &str) -> Result<(), ShortcutError> {
        let normalized = if raw.trim().is_empty() {
            String::new()
        } else {
            normalize_shortcut(raw, action.is_global())?
        };
        if !normalized.is_empty() {
            let peers = if action.is_global() {
                ShortcutAction::GLOBAL
            } else {
                ShortcutAction::LOCAL
            };
            for peer in peers {
                if peer != action && self.get(peer) == normalized {
                    return Err(ShortcutError::Duplicate(peer));
                }
            }
        }
        *self.slot_mut(action) = normalized;
        Ok(())
    }

    pub fn clear(&mut self, action: ShortcutAction) {
        self.slot_mut(action).clear();
    }

    /// 宽松清理持久化读取后的全部字段：非法值回退默认，重复项清空后者。
    pub fn sanitize(&mut self) {
        let defaults = Self::default();
        for action in ShortcutAction::ALL {
            let cur = self.get(action).to_owned();
            if cur.trim().is_empty() {
                self.slot_mut(action).clear();
                continue;
            }
            match normalize_shortcut(&cur, action.is_global()) {
                Ok(norm) => *self.slot_mut(action) = norm,
                Err(_) => *self.slot_mut(action) = defaults.get(action).to_owned(),
            }
        }
        dedup_scope(self, ShortcutAction::GLOBAL);
        dedup_scope(self, ShortcutAction::LOCAL);
    }

    /// 匹配控制台内按键事件。
    #[must_use]
    pub fn match_local(
        &self,
        ctrl: bool,
        alt: bool,
        shift: bool,
        raw_key: &str,
    ) -> Option<ShortcutAction> {
        let binding = format_recorded(ctrl, alt, shift, raw_key, false).ok()?;
        ShortcutAction::LOCAL
            .into_iter()
            .find(|&action| !self.get(action).is_empty() && self.get(action) == binding)
    }

    /// 生成供 `yume-win::hotkeys` 使用的 `(id, accelerator)` 列表（跳过未设置的项）。
    #[must_use]
    pub fn global_accelerators(&self) -> Vec<(&'static str, String)> {
        ShortcutAction::GLOBAL
            .into_iter()
            .filter_map(|action| {
                let acc = to_accelerator(self.get(action))?;
                Some((action.id(), acc))
            })
            .collect()
    }
}

fn dedup_scope(shortcuts: &mut Shortcuts, scope: [ShortcutAction; 3]) {
    for i in 0..scope.len() {
        let cur = shortcuts.get(scope[i]).to_owned();
        if cur.is_empty() {
            continue;
        }
        for &later in &scope[i + 1..] {
            if shortcuts.get(later) == cur {
                shortcuts.clear(later);
            }
        }
    }
}

/// 将按键事件（如 Slint `KeyEvent`）格式化为规范快捷键字符串。
///
/// # Errors
/// 非支持按键，或全局热键缺少修饰键。
pub fn format_recorded(
    ctrl: bool,
    alt: bool,
    shift: bool,
    raw_key: &str,
    require_modifier: bool,
) -> Result<String, ShortcutError> {
    let key = normalize_key_token(raw_key).ok_or(ShortcutError::Invalid)?;
    if require_modifier && !(ctrl || alt || shift) {
        return Err(ShortcutError::NeedModifier);
    }
    let mut parts = Vec::with_capacity(4);
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    parts.push(key);
    Ok(parts.join("+"))
}

/// 解析并规范化快捷键文本。
///
/// # Errors
/// 语法无效或全局热键缺少修饰键。
pub fn normalize_shortcut(raw: &str, require_modifier: bool) -> Result<String, ShortcutError> {
    if raw == " " {
        return format_recorded(false, false, false, " ", require_modifier);
    }
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut key: Option<&'static str> = None;

    for token in raw.split('+').map(str::trim).filter(|t| !t.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            _ => {
                if key.is_some() {
                    return Err(ShortcutError::Invalid);
                }
                key = Some(normalize_key_token(token).ok_or(ShortcutError::Invalid)?);
            }
        }
    }
    let key = key.ok_or(ShortcutError::Invalid)?;
    format_recorded(ctrl, alt, shift, key, require_modifier)
}

/// 转为 `global-hotkey` 可解析的加速器字符串（如 `"ctrl+alt+KeyC"`）。
#[must_use]
pub fn to_accelerator(binding: &str) -> Option<String> {
    let norm = normalize_shortcut(binding, true).ok()?;
    let mut out = Vec::new();
    for part in norm.split('+') {
        let mapped = match part {
            "Ctrl" => "ctrl".to_owned(),
            "Alt" => "alt".to_owned(),
            "Shift" => "shift".to_owned(),
            "Space" => "Space".to_owned(),
            k if k.len() == 1 && k.bytes().all(|b| b.is_ascii_uppercase()) => {
                format!("Key{k}")
            }
            k if k.len() == 1 && k.bytes().all(|b| b.is_ascii_digit()) => {
                format!("Digit{k}")
            }
            k => k.to_owned(),
        };
        out.push(mapped);
    }
    Some(out.join("+"))
}

/// 供界面展示的中文友好文案。
#[must_use]
pub fn display_label(binding: &str) -> String {
    if binding.is_empty() {
        return "未设置".into();
    }
    if binding == "Space" {
        return "空格".into();
    }
    binding.replace("+Space", "+空格")
}

fn normalize_key_token(raw: &str) -> Option<&'static str> {
    if raw == " " {
        return Some("Space");
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "space" | "空格" => return Some("Space"),
        "f1" => return Some("F1"),
        "f2" => return Some("F2"),
        "f3" => return Some("F3"),
        "f4" => return Some("F4"),
        "f5" => return Some("F5"),
        "f6" => return Some("F6"),
        "f7" => return Some("F7"),
        "f8" => return Some("F8"),
        "f9" => return Some("F9"),
        "f10" => return Some("F10"),
        "f11" => return Some("F11"),
        "f12" => return Some("F12"),
        _ => {}
    }
    // 处理带 `KeyX` / `DigitX` 前缀或单字符按键
    let single = lower
        .strip_prefix("key")
        .or_else(|| lower.strip_prefix("digit"))
        .unwrap_or(&lower);
    let bytes = single.as_bytes();
    if bytes.len() == 1 {
        return match bytes[0] {
            b'a' => Some("A"),
            b'b' => Some("B"),
            b'c' => Some("C"),
            b'd' => Some("D"),
            b'e' => Some("E"),
            b'f' => Some("F"),
            b'g' => Some("G"),
            b'h' => Some("H"),
            b'i' => Some("I"),
            b'j' => Some("J"),
            b'k' => Some("K"),
            b'l' => Some("L"),
            b'm' => Some("M"),
            b'n' => Some("N"),
            b'o' => Some("O"),
            b'p' => Some("P"),
            b'q' => Some("Q"),
            b'r' => Some("R"),
            b's' => Some("S"),
            b't' => Some("T"),
            b'u' => Some("U"),
            b'v' => Some("V"),
            b'w' => Some("W"),
            b'x' => Some("X"),
            b'y' => Some("Y"),
            b'z' => Some("Z"),
            b'0' => Some("0"),
            b'1' => Some("1"),
            b'2' => Some("2"),
            b'3' => Some("3"),
            b'4' => Some("4"),
            b'5' => Some("5"),
            b'6' => Some("6"),
            b'7' => Some("7"),
            b'8' => Some("8"),
            b'9' => Some("9"),
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_formats_accelerators() {
        assert_eq!(
            normalize_shortcut("alt + ctrl + c", true),
            Ok("Ctrl+Alt+C".into())
        );
        assert_eq!(
            to_accelerator("Ctrl+Alt+C").as_deref(),
            Some("ctrl+alt+KeyC")
        );
        assert_eq!(
            to_accelerator("Ctrl+Shift+Space").as_deref(),
            Some("ctrl+shift+Space")
        );
        assert_eq!(
            normalize_shortcut("c", true),
            Err(ShortcutError::NeedModifier)
        );
        assert_eq!(normalize_shortcut(" ", false), Ok("Space".into()));
    }

    #[test]
    fn rejects_duplicate_within_same_scope() {
        let mut s = Shortcuts::default();
        assert_eq!(
            s.set(ShortcutAction::GlobalToggleAudio, "Ctrl+Alt+C"),
            Err(ShortcutError::Duplicate(
                ShortcutAction::GlobalToggleConsole
            ))
        );
        assert!(
            s.set(ShortcutAction::GlobalToggleAudio, "Ctrl+Alt+M")
                .is_ok()
        );
        assert_eq!(s.get(ShortcutAction::GlobalToggleAudio), "Ctrl+Alt+M");
    }

    #[test]
    fn matches_local_shortcuts() {
        let s = Shortcuts::default();
        assert_eq!(
            s.match_local(false, false, false, " "),
            Some(ShortcutAction::LocalAudioToggle)
        );
        assert_eq!(
            s.match_local(false, false, false, "q"),
            Some(ShortcutAction::LocalAudioStop)
        );
        assert_eq!(s.match_local(true, false, false, "q"), None);
    }
}
