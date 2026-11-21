//! 配置文件与目录。只做文件读写，不关心内容格式（格式在 yume-core::persist）。

use std::io;
use std::path::{Path, PathBuf};

pub const APP_DIR: &str = "韵梦桌伴";

/// `%APPDATA%\韵梦桌伴`（漫游配置：设置、用户角色包）。
#[must_use]
pub fn roaming_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join(APP_DIR))
}

/// `%LOCALAPPDATA%\韵梦桌伴`（本机数据：日志）。
#[must_use]
pub fn local_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join(APP_DIR))
}

/// 读取文本文件。不存在时返回 `Ok(None)`。
///
/// # Errors
/// 文件存在但读取失败。
pub fn read_text(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// 原子写入：先写临时文件再改名，断电或崩溃不会留下半个文件。
///
/// # Errors
/// 创建目录、写入或改名失败。
pub fn write_text_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_round_trips() {
        let dir = std::env::temp_dir().join(format!("yume-storage-{}", std::process::id()));
        let file = dir.join("state.json");
        assert_eq!(read_text(&file).unwrap(), None);
        write_text_atomic(&file, "a").unwrap();
        write_text_atomic(&file, "b").unwrap();
        assert_eq!(read_text(&file).unwrap().as_deref(), Some("b"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
