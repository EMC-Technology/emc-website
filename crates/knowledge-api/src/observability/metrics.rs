//! 核心业务指标定义（Business Metrics）
//!
//! 基于 [metrics](https://docs.rs/metrics) crate 实现的 Prometheus 兼容指标体系。
//!
//! # 指标分类
//!
//! | 类型 | 用途 | 聚合方式 |
//! |------|------|----------|
//! | Counter | 知识图谱操作计数、文档处理计数 | 累加求和 |
//! | Histogram | 查询延迟、推理延迟、DB 查询耗时 | 分位数分布 |
//! | Gauge | 缓存命中率、连接池活跃数、队列深度 | 实时快照 |
//!
//! # 使用示例
//!
//! ```ignore
//! use knowledge_api::observability::metrics::*;
//!
//! // 记录知识节点创建
//! metrics::counter!("knowledge_nodes_total", "operation" => "create").increment(1);
//!
//! // 记录查询延迟
//! let start = std::time::Instant::now();
//! // ... 执行查询 ...
//! metrics::histogram!("query_duration_seconds", "query_type" => "semantic_search")
//!     .record(start.elapsed().as_secs_f64());
//!
//! // 更新缓存命中率
//! metrics::gauge!("cache_hit_rate", "cache" => "embedding_cache").set(0.95);
//! ```

use metrics::{counter, histogram};

/// 初始化所有业务指标描述符
///
/// 在 Telemetry 初始化时调用，向 Prometheus 注册所有指标的元数据（名称、类型、帮助文本）。
/// 这确保即使指标尚未被使用，Prometheus 也能在 `/metrics` 端点展示其定义。
///
/// # Example
///
/// 此函数由 `observability::init_telemetry()` 自动调用，通常无需手动调用。
pub fn init_business_metrics() {
    describe_all_metrics();
}

fn describe_all_metrics() {
    // ========== Counter 指标 ==========
    metrics::describe_counter!(
        "knowledge_nodes_total",
        "Total number of knowledge graph node operations (create/read/update/delete)"
    );
    metrics::describe_counter!(
        "knowledge_edges_total",
        "Total number of knowledge graph edge operations (create/delete)"
    );
    metrics::describe_counter!(
        "documents_total",
        "Total number of document processing operations (ingest/parse/index)"
    );

    // ========== Histogram 指标 ==========
    metrics::describe_histogram!(
        "query_duration_seconds",
        "Query execution latency in seconds, categorized by query type"
    );
    metrics::describe_histogram!(
        "embedding_inference_duration_seconds",
        "Embedding model inference latency in seconds"
    );
    metrics::describe_histogram!(
        "db_query_duration_seconds",
        "Database query execution latency in seconds"
    );

    // ========== Gauge 指标 ==========
    metrics::describe_gauge!(
        "cache_hit_rate",
        "Cache hit rate as a ratio (0.0-1.0) for various caches"
    );
    metrics::describe_gauge!(
        "active_connections",
        "Number of active connections in connection pools"
    );
    metrics::describe_gauge!("queue_depth", "Current depth of async processing queues");
}

// ============================================================================
// 便捷宏：带标签约束的业务指标记录器
// ============================================================================

/// 记录知识图谱节点操作
///
/// # Labels
/// - `operation`: 操作类型 (`create` | `read` | `update` | `delete`)
#[macro_export]
macro_rules! knowledge_nodes_total {
    ($operation:expr) => {
        $crate::metrics::counter!("knowledge_nodes_total", "operation" => $operation)
    };
}

/// 记录知识图谱边操作
///
/// # Labels
/// - `operation`: 操作类型 (`create` | `delete`)
#[macro_export]
macro_rules! knowledge_edges_total {
    ($operation:expr) => {
        $crate::metrics::counter!("knowledge_edges_total", "operation" => $operation)
    };
}

/// 记录文档处理操作
///
/// # Labels
/// - `operation`: 操作类型 (`ingest` | `parse` | `index`)
#[macro_export]
macro_rules! documents_total {
    ($operation:expr) => {
        $crate::metrics::counter!("documents_total", "operation" => $operation)
    };
}

/// 记录查询延迟（秒）
///
/// # Labels
/// - `query_type`: 查询类型 (`semantic_search` | `keyword_search` | `hybrid`)
#[macro_export]
macro_rules! query_duration_seconds {
    ($query_type:expr) => {
        $crate::metrics::histogram!("query_duration_seconds", "query_type" => $query_type)
    };
}

/// 记录嵌入模型推理延迟（秒）
///
/// # Labels
/// - `model`: 模型标识（如 `all-MiniLM-L6-v2`）
#[macro_export]
macro_rules! embedding_inference_duration_seconds {
    ($model:expr $(,)?) => {
        $crate::metrics::histogram!("embedding_inference_duration_seconds", "model" => $model)
    };
}

/// 记录数据库查询延迟（秒）
///
/// # Labels
/// - `operation`: DB 操作类型 (`select` | `insert` | `update`)
#[macro_export]
macro_rules! db_query_duration_seconds {
    ($operation:expr) => {
        $crate::metrics::histogram!("db_query_duration_seconds", "operation" => $operation)
    };
}

/// 更新缓存命中率
///
/// # Labels
/// - `cache`: 缓存名称 (`embedding_cache` | `node_cache` | `permission_cache`)
#[macro_export]
macro_rules! cache_hit_rate {
    ($cache:expr) => {
        $crate::metrics::gauge!("cache_hit_rate", "cache" => $cache)
    };
}

/// 更新活跃连接数
///
/// # Labels
/// - `pool`: 连接池名称 (`db_pool` | `redis_pool`)
#[macro_export]
macro_rules! active_connections {
    ($pool:expr) => {
        $crate::metrics::gauge!("active_connections", "pool" => $pool)
    };
}

/// 更新异步队列深度
///
/// # Labels
/// - `queue`: 队列名称 (`embedding_queue` | `parse_queue`)
#[macro_export]
macro_rules! queue_depth {
    ($queue:expr) => {
        $crate::metrics::gauge!("queue_depth", "queue" => $queue)
    };
}

// ============================================================================
// 结构化指标记录辅助函数
// ============================================================================

/// HTTP 请求指标记录器
///
/// 封装了请求级别的标准指标采集模式：
/// - 请求计数（按 `method` + `status_code` 分桶）
/// - 请求延迟直方图
/// - 请求体大小
///
/// # Example
///
/// ```ignore
/// let recorder = HttpRequestMetricsRecorder::new("POST", "/api/v1/documents");
/// // ... 处理请求 ...
/// recorder.record(201, body_size, elapsed);
/// ```
pub struct HttpRequestMetricsRecorder {
    method: String,
    path: String,
    start: std::time::Instant,
}

impl HttpRequestMetricsRecorder {
    /// 创建新的请求指标记录器
    #[must_use]
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            start: std::time::Instant::now(),
        }
    }

    /// 记录请求完成指标
    ///
    /// # Parameters
    /// - `status_code`: HTTP 响应状态码
    /// - `body_size_bytes`: 响应体大小（字节）
    pub fn record(&self, status_code: u16, _body_size_bytes: u64) {
        let elapsed_secs = self.start.elapsed().as_secs_f64();

        let status_category = match status_code {
            200..=299 => "2xx",
            300..=399 => "3xx",
            400..=499 => "4xx",
            500..=599 => "5xx",
            other => return tracing::warn!(status = other, "未知的 HTTP 状态码分类"),
        };

        counter!("http_requests_total", "method" => self.method.clone(), "path" => self.path.clone(), "status" => status_category).increment(1);

        histogram!("http_request_duration_seconds", "method" => self.method.clone(), "path" => self.path.clone(), "status" => status_category).record(elapsed_secs);
    }
}

/// 数据库操作指标记录器
///
/// 用于包装 `SurrealDB` 操作，自动记录查询延迟和操作类型。
pub struct DbOperationMetricsRecorder {
    operation: &'static str,
    start: std::time::Instant,
}

impl DbOperationMetricsRecorder {
    /// 创建新的 DB 操作记录器
    ///
    /// # Parameters
    /// - `operation`: 操作类型标识 (`select` | `insert` | `update` | `delete`)
    #[must_use]
    pub fn new(operation: &'static str) -> Self {
        Self {
            operation,
            start: std::time::Instant::now(),
        }
    }

    /// 记录操作完成
    #[must_use]
    pub fn record(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        histogram!("db_query_duration_seconds", "operation" => self.operation).record(elapsed);
        counter!("db_queries_total", "operation" => self.operation).increment(1);
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::gauge;

    #[test]
    fn test_init_business_metrics_does_not_panic() {
        init_business_metrics();
    }

    #[test]
    fn test_http_request_recorder_creation() {
        let recorder = HttpRequestMetricsRecorder::new("POST", "/api/v1/documents");
        assert_eq!(recorder.method, "POST");
        assert_eq!(recorder.path, "/api/v1/documents");
    }

    #[test]
    fn test_http_request_recorder_record_success() {
        let recorder = HttpRequestMetricsRecorder::new("GET", "/healthz");
        recorder.record(200, 42);
    }

    #[test]
    fn test_http_request_recorder_record_client_error() {
        let recorder = HttpRequestMetricsRecorder::new("GET", "/api/v1/documents/invalid-id");
        recorder.record(404, 128);
    }

    #[test]
    fn test_http_request_recorder_record_server_error() {
        let recorder = HttpRequestMetricsRecorder::new("POST", "/api/v1/documents");
        recorder.record(500, 0);
    }

    #[test]
    fn test_db_operation_recorder() {
        let recorder = DbOperationMetricsRecorder::new("select");
        let elapsed = recorder.record();
        assert!(elapsed >= 0.0, "延迟必须为非负值");
    }

    #[test]
    fn test_db_operation_recounter_insert() {
        let recorder = DbOperationMetricsRecorder::new("insert");
        let _elapsed = recorder.record();
    }

    #[tokio::test]
    async fn test_concurrent_metric_recording() {
        use tokio::task::JoinSet;

        let mut set = JoinSet::new();

        for i in 0..100 {
            set.spawn(async move {
                let recorder = DbOperationMetricsRecorder::new("select");
                let _ = recorder.record();

                if i % 2 == 0 {
                    counter!("knowledge_nodes_total", "operation" => "read").increment(1);
                } else {
                    gauge!("cache_hit_rate", "cache" => "node_cache").set(0.9);
                }
            });
        }

        while let Some(result) = set.join_next().await {
            result.expect("任务不应 panic");
        }
    }
}
