use crate::settings::PersistedState;
use std::fs;
use std::path::{Path, PathBuf};

const STATE_FILE_NAME: &str = "state.json";

/// Resolves the persistent data directory (`%LOCALAPPDATA%/YumeRhythm/` by default,
/// overridable via `YUME_DATA_DIR` or `PET_DATA_DIR` for isolated testing).
pub fn resolve_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("YUME_DATA_DIR").or_else(|| std::env::var_os("PET_DATA_DIR"))
    {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("YumeRhythm");
    }
    PathBuf::from(".").join("YumeRhythm")
}

pub fn load_persisted_state(data_dir: &Path) -> PersistedState {
    let path = data_dir.join(STATE_FILE_NAME);
    let Ok(raw) = fs::read_to_string(&path) else {
        return PersistedState::default();
    };
    let mut state: PersistedState = serde_json::from_str(&raw).unwrap_or_default();
    state.sanitize();
    state
}

pub fn save_persisted_state(data_dir: &Path, state: &PersistedState) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|e| format!("无法创建配置目录：{e}"))?;
    let mut sanitized = state.clone();
    sanitized.sanitize();
    let json =
        serde_json::to_string_pretty(&sanitized).map_err(|e| format!("序列化配置失败：{e}"))?;

    let target = data_dir.join(STATE_FILE_NAME);
    let tmp = data_dir.join(format!("{STATE_FILE_NAME}.tmp"));
    fs::write(&tmp, json).map_err(|e| format!("写入临时配置失败：{e}"))?;
    if target.exists() {
        let _ = fs::remove_file(&target);
    }
    fs::rename(&tmp, &target).map_err(|e| format!("保存配置文件失败：{e}"))?;
    Ok(())
}
