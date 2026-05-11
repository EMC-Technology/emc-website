use crate::provider::types::TokenUsage;

/// 指标采集器 trait，用于记录 LLM 请求的运行时指标
pub trait MetricsCollector: Send + Sync {
    /// 记录一次请求发起
    fn record_request(&self, provider: &str, model: &str);
    /// 记录一次响应返回（含延迟和 Token 用量）
    fn record_response(&self, provider: &str, model: &str, latency_ms: u64, usage: &TokenUsage);
    /// 记录一次错误
    fn record_error(&self, provider: &str, model: &str, error_class: &str);
    /// 记录一次流式事件
    fn record_stream_event(&self, provider: &str, model: &str, event_type: &str);
}

/// 空操作指标采集器，所有方法均为 no-op
pub struct NoopMetricsCollector;

impl MetricsCollector for NoopMetricsCollector {
    fn record_request(&self, _provider: &str, _model: &str) {}
    fn record_response(
        &self,
        _provider: &str,
        _model: &str,
        _latency_ms: u64,
        _usage: &TokenUsage,
    ) {
    }
    fn record_error(&self, _provider: &str, _model: &str, _error_class: &str) {}
    fn record_stream_event(&self, _provider: &str, _model: &str, _event_type: &str) {}
}

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// 跟踪级别
    Trace,
    /// 调试级别
    Debug,
    /// 信息级别（默认）
    #[default]
    Info,
    /// 警告级别
    Warn,
    /// 错误级别
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_metrics_collector() {
        let collector = NoopMetricsCollector;
        collector.record_request("openai", "gpt-4o");
        collector.record_response("openai", "gpt-4o", 500, &TokenUsage::default());
        collector.record_error("openai", "gpt-4o", "timeout");
        collector.record_stream_event("openai", "gpt-4o", "text");
    }

    #[test]
    fn test_log_level_default() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn test_log_level_ordering() {
        let levels = [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];
        for i in 0..levels.len() - 1 {
            assert!(
                (levels[i] as u8) < (levels[i + 1] as u8),
                "{:?} should be < {:?}",
                levels[i],
                levels[i + 1]
            );
        }
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::default());
        assert_ne!(LogLevel::Trace, LogLevel::Error);
    }

    #[test]
    fn test_noop_metrics_all_methods() {
        let collector = NoopMetricsCollector;
        collector.record_request("provider", "model");
        collector.record_response("provider", "model", 100, &TokenUsage::default());
        collector.record_error("provider", "model", "network");
        collector.record_stream_event("provider", "model", "text");
        collector.record_stream_event("provider", "model", "thinking");
        collector.record_stream_event("provider", "model", "tool_use");
    }
}
