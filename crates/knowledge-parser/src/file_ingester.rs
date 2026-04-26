//! 文件输入器（流式读取 + BLAKE3 增量哈希）
//!
//! # 原子寻址公理应用
//!
//! 每个 IngestedFile 通过 BLAKE3 哈希值实现**内容寻址**（Content-Addressable），
//! 保证相同文件内容的幂等性——无论何时何地输入，哈希值唯一确定。
//! 这是知识图谱系统中 Document 节点去重和变更检测的基础原语。
//!
//! # 设计要点
//!
//! - **流式读取**：使用 BufReader 分块（64KB 缓冲区）避免大文件 OOM
//! - **增量哈希**：BLAKE3 的 `Hasher::update()` 支持流式数据逐步喂入，
//!   无需将整个文件加载到内存后再计算哈希
//! - **异步 I/O**：基于 tokio::fs，不阻塞异步运行时

use blake3::Hasher;
use knowledge_core::model::SourceType;
use crate::Result;
use error_core::helpers;
use error_core::prelude::ErrorSource;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// 默认最大文件大小限制（1 GB）
const DEFAULT_MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024;

/// 流式读取缓冲区大小（64 KB）
const READ_BUFFER_SIZE: usize = 64 * 1024;

/// 文件输入器（读取文件流 → blake3 增量哈希 → 判定新文件/变更/跳过）
///
/// # 核心职责
///
/// - 流式读取大文件（避免一次性加载到内存）
/// - 计算 BLAKE3 哈希值（用于增量变更检测和幂等性判断）
/// - 文件大小限制检查（防止 OOM）
/// - 返回文件内容和元数据给下游解析器
///
/// # Example
///
/// ```ignore
/// use knowledge_parser::FileIngester;
///
/// #[tokio::main]
/// async fn main() {
///     let ingester = FileIngester::new();
///     let file = ingester.ingest(Path::new("README.md")).await?;
///     println!("hash: {}, size: {}", file.hash, file.file_size);
/// }
/// ```
pub struct FileIngester {
    max_file_size: u64,
}

/// 文件处理结果（传递给下游解析流水线）
#[derive(Debug)]
pub struct IngestedFile {
    /// 文件系统路径
    pub path: PathBuf,
    /// UTF-8 文本内容（二进制文件返回错误）
    pub content: String,
    /// BLAKE3 hex 编码（64 字符）
    pub hash: String,
    /// 文件字节数
    pub file_size: u64,
    /// 自动检测的源类型
    pub source_type: SourceType,
}

impl Default for FileIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl FileIngester {
    /// 创建新的文件输入器实例（默认最大文件大小 1GB）
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        }
    }

    /// 自定义最大文件大小限制
    ///
    /// # 参数
    ///
    /// * `max_size` - 允许的最大文件字节数
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ingester = FileIngester::new().with_max_size(10 * 1024 * 1024); // 10MB
    /// ```
    #[must_use]
    pub const fn with_max_size(mut self, max_size: u64) -> Self {
        self.max_file_size = max_size;
        self
    }

    /// 处理单个文件（流式读取 + 哈希计算）
    ///
    /// 执行流程：
    /// 1. 检查文件是否存在及元信息（大小、是否为目录）
    /// 2. 检测源类型（通过扩展名）
    /// 3. 流式读取 + 同步计算 BLAKE3 增量哈希
    /// 4. 验证 UTF-8 编码
    /// 5. 构造 `IngestedFile` 返回
    ///
    /// # 错误场景
    ///
    /// | 场景 | 错误变体 | 错误码 |
    /// |------|----------|--------|
    /// | 文件不存在 | `NotFound` | E1002 |
    /// | 文件超限 (>`max_file_size`) | `ParseError` | E3001 |
    /// | 非 UTF-8 编码 | `ParseError` | E3001 |
    /// | IO 错误 | `Internal` | E9999 |
    /// | 不支持的格式 | `UnsupportedFormat` | E3002 |
    ///
    /// # Panics
    ///
    /// 此函数不会 panic。
    ///
    /// # Errors
    ///
    /// 文件不存在、路径非文件、文件超限、非 UTF-8 编码、IO 错误或不支持的格式时返回错误。
    pub async fn ingest(&self, path: &Path) -> Result<IngestedFile> {
        // 1. 检查文件是否存在并获取元信息
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|e| helpers::not_found("file", &format!("文件不存在或无法访问 {}: {e}", path.display())))?;

        if !metadata.is_file() {
            return Err(helpers::not_found("file", &format!("路径不是常规文件: {}", path.display())));
        }

        let file_size = metadata.len();

        if file_size > self.max_file_size {
            return Err(helpers::parse_error(&format!("文件大小超限: {} 字节 (上限: {} 字节)",
                file_size, self.max_file_size
            )));
        }

        // 2. 检测源类型
        let source_type =
            crate::source_type_detector::SourceTypeDetector::detect(path)?;

        // 3. 流式读取 + 增量哈希
        // 使用 spawn_blocking 将同步的 BufReader 读取移至线程池，
        // 避免阻塞 tokio 异步运行时的工作线程。
        let path_owned = path.to_path_buf();
        let (content, hash) = tokio::task::spawn_blocking(move || {
            read_and_hash_sync(&path_owned)
        })
        .await
        .map_err(|e| helpers::internal_error(&format!("任务执行被取消: {e}")))??;

        Ok(IngestedFile {
            path: path.to_path_buf(),
            content,
            hash,
            file_size,
            source_type,
        })
    }

    /// 批量处理目录下所有支持的文件
    ///
    /// 递归遍历目录，对每个支持的文件调用 [`Self::ingest`]。
    /// 遇到不支持格式的文件会跳过而非中断整个批处理。
    ///
    /// # 参数
    ///
    /// * `dir` - 要扫描的目录路径
    ///
    /// # Returns
    ///
    /// 成功处理的文件列表。空目录返回空 Vec；IO 错误直接传播。
    ///
    /// # Errors
    ///
    /// 目录读取失败或文件处理失败时返回错误。
    pub async fn ingest_directory(&self, dir: &Path) -> Result<Vec<IngestedFile>> {
        self.ingest_directory_inner(dir).await
    }

    async fn ingest_directory_inner(&self, dir: &Path) -> Result<Vec<IngestedFile>> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| helpers::internal_error(&format!("无法读取目录 {}: {e}", dir.display())))?;

        let mut results = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| helpers::internal_error(&format!("读取目录条目失败: {e}")))?
        {
            let path = entry.path();

            if path.is_dir() {
                match Box::pin(self.ingest_directory_inner(&path)).await {
                    Ok(mut sub_results) => results.append(&mut sub_results),
                    Err(e) => {
                        tracing::warn!("跳过目录 {}: {e}", path.display());
                    }
                }
            } else if path.is_file() {
                match self.ingest(&path).await {
                    Ok(file) => results.push(file),
                    Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                        tracing::debug!("跳过不支持的格式: {}", path.display());
                    }
                    Err(e) => {
                        tracing::warn!("处理文件 {} 失败: {e}", path.display());
                    }
                }
            }
        }

        Ok(results)
    }
}

/// 同步读取文件并计算 `BLAKE3` 哈希（在 `spawn_blocking` 中运行）
fn read_and_hash_sync(path: &Path) -> Result<(String, String)> {
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).map_err(|e| {
        helpers::internal_error(&format!("无法打开文件 {}: {e}", path.display()))
    })?;

    let mut reader = BufReader::with_capacity(READ_BUFFER_SIZE, file);
    let mut hasher = Hasher::new();
    let mut content_bytes = Vec::new();

    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|e| {
            helpers::internal_error(&format!("读取文件 {} 失败: {e}", path.display()))
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        content_bytes.extend_from_slice(&buffer[..bytes_read]);
    }

    let hash_hex = hasher.finalize().to_hex().to_string();

    let content = String::from_utf8(content_bytes).map_err(|e| {
        helpers::parse_error(&format!(
            "文件编码错误（非 UTF-8）: {}: 无效 UTF-8 序列位于字节偏移 {}",
            path.display(),
            e.utf8_error().valid_up_to()
        ))
    })?;

    Ok((content, hash_hex))
}

#[cfg(test)]
mod tests {
    use error_core::prelude::ErrorSource;
    use super::*;
    use std::io::Write as IoWrite;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("knowledge_parser_test_{name}_{id}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn create_temp_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("创建临时文件失败");
        f.write_all(content).expect("写入临时文件失败");
        path
    }

    fn cleanup_dir(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_ingest_small_file_success() {
        let dir = unique_test_dir("small");
        let path = create_temp_file(&dir, "test_small.md", b"hello world");

        let ingester = FileIngester::new();
        let result = ingester.ingest(&path).await;

        assert!(result.is_ok(), "小文件应成功读取: {:?}", result.err());
        let file = result.unwrap();
        assert_eq!(file.content, "hello world");
        assert_eq!(file.file_size, 11);
        assert_eq!(file.source_type, SourceType::Markdown);
        assert_eq!(file.hash.len(), 64);
        assert!(file.hash.chars().all(|c| c.is_ascii_hexdigit()));

        cleanup_dir(&dir);
    }

    #[tokio::test]
    async fn test_hash_determinism() {
        let dir = unique_test_dir("hash");
        let content = b"deterministic content for hashing test";
        let path_a = create_temp_file(&dir, "test_hash_a.txt", content);
        let path_b = create_temp_file(&dir, "test_hash_b.txt", content);

        let ingester = FileIngester::new();
        let result_a = ingester.ingest(&path_a).await.expect("文件A应成功");
        let result_b = ingester.ingest(&path_b).await.expect("文件B应成功");

        assert_eq!(
            result_a.hash, result_b.hash,
            "相同内容应产生相同的 BLAKE3 哈希值"
        );

        cleanup_dir(&dir);
    }

    #[tokio::test]
    async fn test_file_size_limit_exceeded() {
        let dir = unique_test_dir("oversized");
        let large_content = vec![0x41u8; 100];
        let path = create_temp_file(&dir, "test_oversized.md", &large_content);

        let ingester = FileIngester::new().with_max_size(50);
        let result = ingester.ingest(&path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::FS => {
                assert!(
                    err.message().contains("文件大小超限"),
                    "错误消息应包含'文件大小超限': {}",
                    err.message()
                );
            }
            other => panic!("期望 ParseError, 实际: {other}"),
        }

        cleanup_dir(&dir);
    }

    #[tokio::test]
    async fn test_non_utf8_file_error() {
        let dir = unique_test_dir("nonutf8");
        let invalid_utf8: Vec<u8> = vec![0x80, 0xFF, 0xFE, 0x01];
        let path = create_temp_file(&dir, "test_binary.txt", &invalid_utf8);

        let ingester = FileIngester::new();
        let result = ingester.ingest(&path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::FS => {
                assert!(
                    err.message().contains("编码错误"),
                    "错误消息应包含'编码错误': {}",
                    err.message()
                );
            }
            other => panic!("期望 ParseError, 实际: {other}"),
        }

        cleanup_dir(&dir);
    }

    #[tokio::test]
    async fn test_not_found_error() {
        let nonexistent = PathBuf::from("/nonexistent/path/file_that_does_not_exist.xyz");

        let ingester = FileIngester::new();
        let result = ingester.ingest(&nonexistent).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            err if err.code().contains("VAL") => {
                assert!(
                    err.message().contains("文件不存在"),
                    "错误消息应包含'文件不存在': {}",
                    err.message()
                );
            }
            other => panic!("期望 NotFound, 实际: {other}"),
        }
    }

    #[tokio::test]
    async fn test_with_max_size_builder_pattern() {
        let ingester = FileIngester::new().with_max_size(1024);
        assert_eq!(ingester.max_file_size, 1024);
    }

    #[tokio::test]
    async fn test_default_max_file_size_is_1gb() {
        let ingester = FileIngester::new();
        assert_eq!(ingester.max_file_size, DEFAULT_MAX_FILE_SIZE);
    }
}
