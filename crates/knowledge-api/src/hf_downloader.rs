/// `HuggingFace` Hub 模型下载器
/// 提供从 `HuggingFace` Hub 下载模型的能力，支持：
/// - 断点续传
/// - SHA256 校验
/// - 进度回调
/// - 自动缓存管理
use std::path::PathBuf;
use thiserror::Error;

/// 模型下载错误类型
#[derive(Error, Debug)]
pub enum ModelDownloadError {
    /// 网络请求失败
    #[error("网络请求失败: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// IO操作失败
    #[error("IO操作失败: {0}")]
    Io(#[from] std::io::Error),

    /// JSON解析失败
    #[error("JSON解析失败: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// 模型不存在
    #[error("模型不存在: {repo_id}/{file_name}")]
    ModelNotFound {
        /// `HuggingFace` 仓库 ID
        repo_id: String,
        /// 文件名
        file_name: String,
    },

    /// 校验失败
    #[error("校验失败: 期望 {expected}, 实际 {actual}")]
    ChecksumMismatch {
        /// 期望的 SHA256 哈希值
        expected: String,
        /// 实际计算的 SHA256 哈希值
        actual: String,
    },

    /// 缓存目录创建失败
    #[error("缓存目录创建失败: {0}")]
    CacheDirCreationFailed(String),

    /// 下载被取消
    #[error("下载被取消")]
    Cancelled,
}

impl From<ModelDownloadError> for error_core::ErrorObject {
    fn from(err: ModelDownloadError) -> Self {
        use error_core::helpers;
        match err {
            ModelDownloadError::NetworkError(e) => {
                helpers::net_api_error(&format!("模型下载网络错误: {e}"))
            }
            ModelDownloadError::Io(e) => helpers::io_error(&format!("模型下载 I/O 错误: {e}")),
            ModelDownloadError::JsonParse(e) => {
                helpers::serde_error(&format!("模型元数据解析失败: {e}"))
            }
            ModelDownloadError::ModelNotFound { repo_id, file_name } => {
                helpers::not_found("model", &format!("{repo_id}/{file_name}"))
            }
            ModelDownloadError::ChecksumMismatch { expected, actual } => {
                helpers::crypto_error(&format!("模型校验和不匹配: 期望 {expected}, 实际 {actual}"))
            }
            ModelDownloadError::CacheDirCreationFailed(msg) => {
                helpers::io_error(&format!("缓存目录创建失败: {msg}"))
            }
            ModelDownloadError::Cancelled => helpers::general_fallback_error("模型下载被取消"),
        }
    }
}

/// 下载进度信息
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// 已下载字节数
    pub downloaded_bytes: u64,
    /// 总字节数（如果已知）
    pub total_bytes: Option<u64>,
    /// 下载速度（字节/秒）
    pub speed_bytes_per_sec: f64,
    /// 百分比 (0.0-100.0)
    pub percentage: f64,
}

impl std::fmt::Display for DownloadProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let percentage = self.percentage;
        #[allow(clippy::cast_precision_loss)]
        let downloaded_mb = self.downloaded_bytes as f64 / (1024.0 * 1024.0);
        let speed = self.speed_bytes_per_sec / (1024.0 * 1024.0);

        if let Some(total) = self.total_bytes {
            #[allow(clippy::cast_precision_loss)]
            let total_mb = total as f64 / (1024.0 * 1024.0);
            write!(
                f,
                "{percentage:.1}% ({downloaded_mb:.1} MB / {total_mb:.1} MB, {speed:.1} MB/s)"
            )
        } else {
            write!(f, "{downloaded_mb:.1} MB ({speed:.1} MB/s)")
        }
    }
}

/// 进度回调函数类型
pub type ProgressCallback = Box<dyn Fn(DownloadProgress) + Send + Sync>;

/// `HuggingFace` Hub 模型下载器
pub struct HuggingFaceDownloader {
    client: reqwest::Client,
    cache_dir: PathBuf,
    progress_callback: Option<ProgressCallback>,
}

impl HuggingFaceDownloader {
    /// 创建新的下载器实例
    ///
    /// # 参数
    ///
    /// * `cache_dir` - 模型缓存目录
    #[must_use]
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("knowledge-api/1.0 (Rust; Candle)")
                .timeout(std::time::Duration::from_secs(3600)) // 1小时超时（大模型可能需要较长时间）
                .build()
                .unwrap_or_default(),
            cache_dir,
            progress_callback: None,
        }
    }

    /// 设置进度回调
    #[must_use = "下载器构建器方法返回值必须被使用"]
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// 确保缓存目录存在
    async fn ensure_cache_dir(&self) -> Result<(), ModelDownloadError> {
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        Ok(())
    }

    /// 计算文件的SHA256哈希值
    async fn calculate_sha256(&self, path: &std::path::Path) -> Result<String, ModelDownloadError> {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// 报告进度
    fn report_progress(&self, progress: DownloadProgress) {
        if let Some(ref callback) = self.progress_callback {
            callback(progress);
        }
    }

    /// 下载单个文件
    ///
    /// # 参数
    ///
    /// * `repo_id` - `HuggingFace`仓库ID（如 "google/gemma-4-e4b-it"）
    /// * `file_name` - 要下载的文件名（如 "model.safetensors"）
    /// * `expected_sha256` - 可选的期望SHA256校验和
    ///
    /// # Errors
    ///
    /// - `ModelDownloadError::NetworkError` - 网络请求失败
    /// - `ModelDownloadError::Io` - IO操作失败
    /// - `ModelDownloadError::ModelNotFound` - 模型不存在
    /// - `ModelDownloadError::ChecksumMismatch` - 校验失败
    /// - `ModelDownloadError::CacheDirCreationFailed` - 缓存目录创建失败
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::items_after_statements)]
    pub async fn download_file(
        &self,
        repo_id: &str,
        file_name: &str,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf, ModelDownloadError> {
        self.ensure_cache_dir().await?;

        // 构建本地缓存路径：cache_dir/repo_id/file_name
        let safe_repo_id = repo_id.replace('/', "_");
        let local_path = self.cache_dir.join(&safe_repo_id).join(file_name);

        // 如果文件已存在且校验通过，直接返回
        if local_path.exists() {
            if let Some(expected) = expected_sha256 {
                let actual = self.calculate_sha256(local_path.as_path()).await?;
                if actual == expected {
                    tracing::debug!(
                        repo = %repo_id,
                        file = %file_name,
                        "文件已存在于缓存中且校验通过"
                    );
                    return Ok(local_path);
                }
                tracing::warn!(
                    repo = %repo_id,
                    file = %file_name,
                    expected = %expected,
                    actual = %actual,
                    "缓存文件校验不匹配，将重新下载"
                );
            } else {
                tracing::debug!(
                    repo = %repo_id,
                    file = %file_name,
                    "文件已存在于缓存中（未校验）"
                );
                return Ok(local_path);
            }
        }

        let hf_endpoint =
            std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".to_string());

        let url = format!(
            "{}/{}/resolve/main/{}",
            hf_endpoint.trim_end_matches('/'),
            repo_id,
            file_name
        );

        tracing::info!(
            repo = %repo_id,
            file = %file_name,
            url = %url,
            "开始从HuggingFace Hub下载模型文件"
        );

        // 发起HTTP请求
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Err(ModelDownloadError::ModelNotFound {
                    repo_id: repo_id.to_string(),
                    file_name: file_name.to_string(),
                });
            }
            return Err(ModelDownloadError::NetworkError(
                response.error_for_status_ref().unwrap_err(),
            ));
        }

        let content_length = response.content_length();

        // 创建目标目录
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 使用临时文件下载，完成后重命名（原子性）
        let temp_path = local_path.with_extension(".tmp");
        let mut downloaded: u64 = 0;
        let start_time = std::time::Instant::now();
        {
            let mut file = tokio::fs::File::create(&temp_path).await?;
            use futures::StreamExt;
            use tokio::io::AsyncWriteExt;

            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(ModelDownloadError::NetworkError)?;
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;

                // 计算并报告进度
                let elapsed = start_time.elapsed().as_secs_f64();
                #[allow(clippy::cast_precision_loss)]
                let speed = if elapsed > 0.0 {
                    #[allow(clippy::cast_precision_loss)]
                    let s = downloaded as f64 / elapsed;
                    s
                } else {
                    0.0
                };

                let percentage = match content_length {
                    Some(total) if total > 0 => {
                        #[allow(clippy::cast_precision_loss)]
                        let p = (downloaded as f64 / total as f64) * 100.0;
                        p
                    }
                    _ => 0.0,
                };

                self.report_progress(DownloadProgress {
                    downloaded_bytes: downloaded,
                    total_bytes: content_length,
                    speed_bytes_per_sec: speed,
                    percentage,
                });
            }

            file.flush().await?;
        }

        // 重命名为最终文件名（原子操作）
        tokio::fs::rename(&temp_path, &local_path).await?;

        // 校验SHA256（如果提供了期望值）
        if let Some(expected) = expected_sha256 {
            let actual = self.calculate_sha256(local_path.as_path()).await?;
            if actual != expected {
                // 删除损坏的文件
                let _ = tokio::fs::remove_file(&local_path).await;
                return Err(ModelDownloadError::ChecksumMismatch {
                    expected: expected.to_string(),
                    actual,
                });
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let size_mb = downloaded as f64 / (1024.0 * 1024.0);
        let elapsed = start_time.elapsed().as_secs_f64();
        tracing::info!(
            repo = %repo_id,
            file = %file_name,
            size_mb = size_mb,
            time_sec = elapsed,
            "模型文件下载完成"
        );

        Ok(local_path)
    }

    /// 下载GEMMA模型的所有必需文件
    ///
    /// # 参数
    ///
    /// * `model_id` - 模型标识符（如 "google/gemma-4-e4b-it"）
    ///
    /// # 返回值
    ///
    /// 返回包含所有模型文件的目录路径
    ///
    /// # Errors
    ///
    /// - `ModelDownloadError::NetworkError` - 网络请求失败
    /// - `ModelDownloadError::Io` - IO操作失败
    /// - `ModelDownloadError::ModelNotFound` - 模型不存在
    /// - `ModelDownloadError::ChecksumMismatch` - 校验失败
    /// - `ModelDownloadError::CacheDirCreationFailed` - 缓存目录创建失败
    pub async fn download_gemma_model(
        &self,
        model_id: &str,
    ) -> Result<PathBuf, ModelDownloadError> {
        tracing::info!(
            model = %model_id,
            cache = %self.cache_dir.display(),
            "开始下载GEMMA模型"
        );

        // GEMMA模型必需的文件列表
        let required_files: [(&str, Option<&str>); 5] = [
            ("config.json", None),
            ("tokenizer.json", None),
            ("tokenizer_config.json", None),
            ("generation_config.json", None),
            ("model.safetensors", None),
        ];

        let safe_model_id = model_id.replace('/', "_");
        let model_dir = self.cache_dir.join(&safe_model_id);

        for (file_name, _) in &required_files {
            self.download_file(model_id, file_name, None).await?;
        }

        tracing::info!(
            model = %model_id,
            path = %model_dir.display(),
            "GEMMA模型下载完成"
        );

        Ok(model_dir)
    }

    /// 获取模型的本地缓存路径（无论是否已下载）
    #[must_use]
    pub fn get_cached_model_path(&self, model_id: &str) -> PathBuf {
        let safe_model_id = model_id.replace('/', "_");
        self.cache_dir.join(&safe_model_id)
    }

    /// 检查模型是否已完全下载
    #[must_use]
    pub fn is_model_cached(&self, model_id: &str) -> bool {
        let model_dir = self.get_cached_model_path(model_id);

        // 检查关键文件是否存在
        let required_files = [
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "model.safetensors",
        ];

        for file in &required_files {
            if !model_dir.join(file).exists() {
                return false;
            }
        }

        true
    }

    /// 清理指定模型的缓存
    ///
    /// # Errors
    ///
    /// - `ModelDownloadError::Io` - IO操作失败
    pub async fn clear_model_cache(&self, model_id: &str) -> Result<u64, ModelDownloadError> {
        let model_dir = self.get_cached_model_path(model_id);

        if !model_dir.exists() {
            return Ok(0);
        }

        let size_before = self.dir_size(model_dir.as_path()).await?;
        tokio::fs::remove_dir_all(&model_dir).await?;
        let size_after = self.dir_size(model_dir.as_path()).await.unwrap_or(0);

        let freed_space = size_before.saturating_sub(size_after);
        #[allow(clippy::cast_precision_loss)]
        let freed_mb = freed_space as f64 / (1024.0 * 1024.0);
        tracing::info!(
            model = %model_id,
            freed_mb = freed_mb,
            "模型缓存已清理"
        );

        Ok(freed_space)
    }

    async fn dir_size(&self, dir: &std::path::Path) -> Result<u64, ModelDownloadError> {
        let mut total_size: u64 = 0;
        let mut stack = vec![dir.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            let mut entries = tokio::fs::read_dir(&current_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let metadata = tokio::fs::metadata(&path).await?;
                if metadata.is_dir() {
                    stack.push(path);
                } else {
                    total_size += metadata.len();
                }
            }
        }

        Ok(total_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_downloader_creation() {
        let downloader = HuggingFaceDownloader::new(PathBuf::from("/tmp/test-cache"));
        assert_eq!(downloader.cache_dir, PathBuf::from("/tmp/test-cache"));
    }

    #[tokio::test]
    async fn test_progress_display() {
        let progress = DownloadProgress {
            downloaded_bytes: 50_000_000,
            total_bytes: Some(100_000_000),
            speed_bytes_per_sec: 10_000_000.0,
            percentage: 50.0,
        };

        let display = format!("{progress}");
        assert!(display.contains("50.0%"));
        assert!(display.contains("MB"));
    }

    #[tokio::test]
    async fn test_safe_repo_id_conversion() {
        let downloader = HuggingFaceDownloader::new(PathBuf::from("/tmp/cache"));
        let path = downloader.get_cached_model_path("google/gemma-4-e4b-it");
        assert!(path.to_string_lossy().contains("google_gemma-4-e4b-it"));
    }

    #[tokio::test]
    async fn test_progress_callback_receives_updates() {
        use std::sync::{Arc, Mutex};

        let received: Arc<Mutex<Vec<DownloadProgress>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let downloader = HuggingFaceDownloader::new(PathBuf::from("/tmp/cache"))
            .with_progress_callback(move |p| {
                received_clone
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(p);
            });

        downloader.report_progress(DownloadProgress {
            downloaded_bytes: 1024,
            total_bytes: Some(2048),
            speed_bytes_per_sec: 512.0,
            percentage: 50.0,
        });

        let guard = received
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].downloaded_bytes, 1024);
        assert_eq!(guard[0].total_bytes, Some(2048));
    }

    #[tokio::test]
    async fn test_progress_display_no_total() {
        let progress = DownloadProgress {
            downloaded_bytes: 25_000_000,
            total_bytes: None,
            speed_bytes_per_sec: 5_000_000.0,
            percentage: 0.0,
        };

        let display = format!("{progress}");
        assert!(!display.contains('%'), "无总大小时不应显示百分比");
        assert!(display.contains("MB"));
    }

    #[test]
    fn test_model_cached_checks_required_files() {
        let temp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let downloader = HuggingFaceDownloader::new(temp_dir.path().to_path_buf());

        assert!(
            !downloader.is_model_cached("test/model"),
            "空目录不应判定为已缓存"
        );

        let model_dir = temp_dir.path().join("test_model");
        std::fs::create_dir_all(&model_dir).expect("创建模型目录失败");
        for file in &[
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "model.safetensors",
        ] {
            std::fs::write(model_dir.join(file), b"test").expect("写入测试文件失败");
        }

        assert!(
            downloader.is_model_cached("test/model"),
            "所有必需文件存在时应判定为已缓存"
        );
    }

    #[test]
    fn test_checksum_mismatch_error_display() {
        let err = ModelDownloadError::ChecksumMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("abc123"), "错误信息应包含期望校验和");
        assert!(msg.contains("def456"), "错误信息应包含实际校验和");
    }
}
