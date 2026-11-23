//! 角色包的查找、扫描与后台加载。文件读取在这里，格式解析与校验在 yume-character。

use std::path::{Component, Path, PathBuf};
use yume_character::{Pack, PackInfo, PackSource};
use yume_core::model::pet::CharacterInfo;
use yume_core::persist::is_valid_character_id;

const MANIFEST: &str = "character.json";

/// 从目录读取角色包文件。拒绝绝对路径和 `..`，包里的文件只能在包目录内。
pub struct DirSource(PathBuf);

impl PackSource for DirSource {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let relative = Path::new(path);
        let safe = relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if !safe {
            return Err(format!("非法路径：{path}"));
        }
        match std::fs::read(self.0.join(relative)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{path}：{e}")),
        }
    }
}

/// 角色包所在的目录，按优先级排列：用户目录，再是程序自带的。
#[must_use]
pub fn roots(user_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("YUME_CHARACTERS") {
        roots.push(PathBuf::from(dir));
    }
    if let Some(dir) = user_dir {
        roots.push(dir.join("characters"));
    }
    if let Some(dir) = builtin_dir() {
        roots.push(dir);
    }
    roots
}

/// 程序旁边的 `characters`。开发时可执行文件在 `target/<profile>/` 下，向上找仓库根目录。
fn builtin_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .skip(1)
        .take(4)
        .map(|dir| dir.join("characters"))
        .find(|dir| dir.is_dir())
}

/// 列出可用角色。同一 id 只取优先级最高的那个。
#[must_use]
pub fn scan(roots: &[PathBuf]) -> Vec<CharacterInfo> {
    let mut found: Vec<CharacterInfo> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            let Some(id) = dir.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !is_valid_character_id(id) || found.iter().any(|c| c.id == id) {
                continue;
            }
            let Ok(bytes) = std::fs::read(dir.join(MANIFEST)) else {
                continue;
            };
            match PackInfo::parse(&bytes) {
                Ok(info) if info.id == id => found.push(CharacterInfo {
                    id: info.id,
                    name: info.name,
                }),
                Ok(info) => log::warn!("角色目录 {id} 里的 id 是 {}，已跳过", info.id),
                Err(e) => log::warn!("角色 {id} 的清单有误：{e}"),
            }
        }
    }
    found
}

/// 自定义表情根目录：`%APPDATA%\韵梦桌伴\custom_expressions`。
#[must_use]
pub fn custom_root(user_dir: Option<&Path>) -> Option<PathBuf> {
    user_dir.map(|d| d.join("custom_expressions"))
}

/// 加载角色包，同时尝试合并对应角色的自定义表情。
///
/// # Errors
/// 找不到或主包格式有误时返回中文描述。
pub fn load(roots: &[PathBuf], custom_root: Option<&Path>, id: &str) -> Result<Pack, String> {
    if !is_valid_character_id(id) {
        return Err(format!("角色 id 不合法：{id}"));
    }
    let dir = roots
        .iter()
        .map(|root| root.join(id))
        .find(|dir| dir.join(MANIFEST).is_file())
        .ok_or_else(|| format!("找不到角色「{id}」"))?;
    let custom_dir = custom_root.map(|root| DirSource(root.join(id)));
    let custom_ref = custom_dir.as_ref().map(|d| d as &dyn PackSource);
    Pack::load(&DirSource(dir), id, custom_ref).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_characters() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../characters")
    }

    #[test]
    fn finds_and_loads_builtin() {
        let roots = vec![repo_characters()];
        let list = scan(&roots);
        assert!(list.iter().any(|c| c.id == "sakura"), "{list:?}");
        assert!(load(&roots, None, "sakura").is_ok());
        assert!(load(&roots, None, "../sakura").is_err());
        assert!(load(&roots, None, "nobody").is_err());
    }

    #[test]
    fn dir_source_rejects_escape() {
        let source = DirSource(repo_characters().join("sakura"));
        assert!(source.read("../sakura/character.json").is_err());
        assert!(source.read("/etc/passwd").is_err());
        assert!(matches!(source.read("character.json"), Ok(Some(_))));
        assert!(matches!(source.read("missing.png"), Ok(None)));
    }
}
