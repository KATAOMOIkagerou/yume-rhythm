//! 角色包文件来源。L0 crate 不直接访问文件系统，目录实现在 yume-app。

use std::collections::HashMap;

/// 按包内相对路径读取文件。路径已经过校验（见 `validate::check_path`）。
pub trait PackSource {
    /// 读取文件。文件不存在时返回 `Ok(None)`。
    ///
    /// # Errors
    /// 读取失败（权限、磁盘错误等）时返回错误描述。
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, String>;
}

/// 内存中的角色包，测试用。
#[derive(Clone, Debug, Default)]
pub struct MemorySource {
    files: HashMap<String, Vec<u8>>,
}

impl MemorySource {
    pub fn insert(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(path.to_string(), bytes);
    }
}

impl PackSource for MemorySource {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.files.get(path).cloned())
    }
}
