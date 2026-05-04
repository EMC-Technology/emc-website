//! 多仓库注册表模块
//!
//! 管理多个知识库仓库的注册信息，持久化存储于 `~/.knowledge-system/registry.json`。
//! 支持仓库的注册、注销、查询，以及按路径或当前工作目录查找仓库。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::helpers;
use crate::Result;

/// 注册表条目
///
/// 描述一个已注册知识库仓库的元信息。
///
/// # 字段
///
/// - `path`：仓库的绝对路径
/// - `indexed_at`：最近一次完整索引的 ISO 8601 时间戳
/// - `doc_count`：仓库中的文档数量
/// - `last_hash`：最近一次完整索引的 blake3 哈希值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// 仓库的绝对路径
    pub path: String,
    /// 最近一次完整索引的 ISO 8601 时间戳
    pub indexed_at: String,
    /// 仓库中的文档数量
    pub doc_count: usize,
    /// 最近一次完整索引的 blake3 哈希值
    pub last_hash: String,
}

/// 多仓库注册表
///
/// 管理多个知识库仓库的注册信息，数据持久化到 `~/.knowledge-system/registry.json`。
///
/// # 示例
///
/// ```ignore
/// use knowledge_core::registry::{Registry, RegistryEntry};
///
/// let mut reg = Registry::load()?;
/// reg.register("my-repo".to_string(), RegistryEntry {
///     path: "/path/to/repo".to_string(),
///     indexed_at: "2025-01-01T00:00:00Z".to_string(),
///     doc_count: 42,
///     last_hash: "abc123".to_string(),
/// })?;
/// reg.save()?;
/// ```
pub struct Registry {
    entries: HashMap<String, RegistryEntry>,
    path: PathBuf,
}

impl Registry {
    /// 注册表文件名
    const REGISTRY_FILENAME: &'static str = "registry.json";

    /// 知识系统配置目录名
    const CONFIG_DIR_NAME: &'static str = ".knowledge-system";

    /// 从默认路径加载注册表
    ///
    /// 从 `~/.knowledge-system/registry.json` 加载注册表数据。
    /// 若文件不存在，返回空注册表实例。
    ///
    /// # Errors
    ///
    /// - 文件存在但无法读取时返回 I/O 错误
    /// - 文件内容不是合法 JSON 时返回解析错误
    pub fn load() -> Result<Self> {
        let path = Self::default_registry_path()?;
        Self::load_from(&path)
    }

    /// 从指定路径加载注册表
    ///
    /// # 参数
    ///
    /// - `path`：注册表 JSON 文件的路径
    ///
    /// # Errors
    ///
    /// - 文件存在但无法读取时返回 I/O 错误
    /// - 文件内容不是合法 JSON 时返回解析错误
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                entries: HashMap::new(),
                path: path.to_path_buf(),
            });
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            helpers::config_error(&format!("无法读取注册表文件: {e}"))
        })?;

        let entries: HashMap<String, RegistryEntry> =
            serde_json::from_str(&content).map_err(|e| {
                helpers::parse_error(&format!("注册表 JSON 解析失败: {e}"))
            })?;

        Ok(Self {
            entries,
            path: path.to_path_buf(),
        })
    }

    /// 将注册表保存到磁盘
    ///
    /// 将当前注册表数据序列化为 JSON 并写入 `path` 指定的文件。
    /// 若父目录不存在，会自动创建。
    ///
    /// # Errors
    ///
    /// - 无法创建父目录时返回配置错误
    /// - 无法写入文件时返回 I/O 错误
    /// - 序列化失败时返回序列化错误
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                helpers::config_error(&format!("无法创建注册表目录: {e}"))
            })?;
        }

        let content = serde_json::to_vec_pretty(&self.entries).map_err(|e| {
            helpers::serde_error(&format!("注册表序列化失败: {e}"))
        })?;

        std::fs::write(&self.path, content).map_err(|e| {
            helpers::config_error(&format!("无法写入注册表文件: {e}"))
        })?;

        Ok(())
    }

    /// 注册一个仓库
    ///
    /// 将指定名称的仓库条目添加到注册表。若名称已存在，则覆盖原有条目。
    /// 注册后自动持久化到磁盘。
    ///
    /// # 参数
    ///
    /// - `name`：仓库名称（唯一标识）
    /// - `entry`：仓库元信息
    ///
    /// # Errors
    ///
    /// 保存到磁盘失败时返回错误
    pub fn register(&mut self, name: String, entry: RegistryEntry) -> Result<()> {
        self.entries.insert(name, entry);
        self.save()
    }

    /// 注销一个仓库
    ///
    /// 从注册表中移除指定名称的仓库条目，并持久化到磁盘。
    /// 若名称不存在，不做任何操作。
    ///
    /// # 参数
    ///
    /// - `name`：要注销的仓库名称
    ///
    /// # Errors
    ///
    /// 保存到磁盘失败时返回错误
    pub fn unregister(&mut self, name: &str) -> Result<()> {
        self.entries.remove(name);
        self.save()
    }

    /// 按名称查询仓库条目
    ///
    /// # 参数
    ///
    /// - `name`：仓库名称
    ///
    /// # 返回值
    ///
    /// 若找到则返回条目引用，否则返回 `None`
    pub fn get(&self, name: &str) -> Option<&RegistryEntry> {
        self.entries.get(name)
    }

    /// 列出所有已注册的仓库名称
    ///
    /// # 返回值
    ///
    /// 仓库名称的引用列表（无特定顺序）
    pub fn list(&self) -> Vec<&String> {
        self.entries.keys().collect()
    }

    /// 按路径查找仓库名称
    ///
    /// 在所有已注册条目中查找 `path` 字段与给定路径匹配的条目，
    /// 返回其仓库名称。
    ///
    /// # 参数
    ///
    /// - `path`：要查找的仓库绝对路径
    ///
    /// # 返回值
    ///
    /// 若找到则返回仓库名称引用，否则返回 `None`
    pub fn find_by_path(&self, path: &str) -> Option<&String> {
        self.entries
            .iter()
            .find(|(_, entry)| entry.path == path)
            .map(|(name, _)| name)
    }

    /// 按当前工作目录查找仓库
    ///
    /// 获取当前工作目录，并在注册表中查找路径匹配的仓库。
    ///
    /// # 返回值
    ///
    /// 若找到则返回仓库名称引用，否则返回 `None`
    pub fn find_by_cwd(&self) -> Option<&String> {
        let cwd = std::env::current_dir().ok()?;
        let cwd_str = cwd.to_str()?;
        self.find_by_path(cwd_str)
    }

    /// 获取默认注册表文件路径
    ///
    /// 返回 `~/.knowledge-system/registry.json` 的完整路径。
    ///
    /// # Errors
    ///
    /// 无法获取用户主目录时返回配置错误
    fn default_registry_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| {
            helpers::config_error("无法获取用户主目录")
        })?;
        Ok(home.join(Self::CONFIG_DIR_NAME).join(Self::REGISTRY_FILENAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str) -> RegistryEntry {
        RegistryEntry {
            path: path.to_string(),
            indexed_at: "2025-01-01T00:00:00Z".to_string(),
            doc_count: 10,
            last_hash: "a".repeat(64),
        }
    }

    #[test]
    fn test_registry_new_is_empty() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("registry.json");
        let reg = Registry::load_from(&path).expect("加载注册表失败");
        assert!(reg.list().is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("registry.json");
        let mut reg = Registry::load_from(&path).expect("加载注册表失败");

        let entry = make_entry("/path/to/repo");
        reg.register("my-repo".to_string(), entry.clone())
            .expect("注册失败");

        let got = reg.get("my-repo").expect("应找到 my-repo");
        assert_eq!(got.path, entry.path);
        assert_eq!(got.doc_count, entry.doc_count);
        assert_eq!(got.last_hash, entry.last_hash);

        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_unregister() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("registry.json");
        let mut reg = Registry::load_from(&path).expect("加载注册表失败");

        reg.register("repo-a".to_string(), make_entry("/a"))
            .expect("注册失败");
        reg.register("repo-b".to_string(), make_entry("/b"))
            .expect("注册失败");

        assert_eq!(reg.list().len(), 2);

        reg.unregister("repo-a").expect("注销失败");
        assert!(reg.get("repo-a").is_none());
        assert!(reg.get("repo-b").is_some());
        assert_eq!(reg.list().len(), 1);

        reg.unregister("nonexistent").expect("注销不存在的条目不应报错");
        assert_eq!(reg.list().len(), 1);
    }

    #[test]
    fn test_registry_find_by_path() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("registry.json");
        let mut reg = Registry::load_from(&path).expect("加载注册表失败");

        reg.register("repo-x".to_string(), make_entry("/data/repos/x"))
            .expect("注册失败");
        reg.register("repo-y".to_string(), make_entry("/data/repos/y"))
            .expect("注册失败");

        let name = reg.find_by_path("/data/repos/x").expect("应找到路径");
        assert_eq!(name, "repo-x");

        assert!(reg.find_by_path("/nonexistent").is_none());
    }

    #[test]
    fn test_registry_save_and_load() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("registry.json");

        let mut reg = Registry::load_from(&path).expect("加载注册表失败");
        reg.register("repo-1".to_string(), make_entry("/path/1"))
            .expect("注册失败");
        reg.register("repo-2".to_string(), make_entry("/path/2"))
            .expect("注册失败");
        reg.save().expect("保存失败");

        let loaded = Registry::load_from(&path).expect("重新加载失败");
        assert_eq!(loaded.list().len(), 2);
        assert_eq!(loaded.get("repo-1").expect("应找到 repo-1").path, "/path/1");
        assert_eq!(loaded.get("repo-2").expect("应找到 repo-2").path, "/path/2");
    }
}
