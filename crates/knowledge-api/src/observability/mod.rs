pub mod metrics;

use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    Resource,
    trace::{Config, Sampler, TracerProvider},
};
use std::env;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(test)]
const DEFAULT_SERVICE_NAME: &str = "knowledge-api";

mod resource_keys {
    pub const SERVICE_NAME: &str = "service.name";
    pub const SERVICE_VERSION: &str = "service.version";
}

/// OpenTelemetry 遥测句柄
///
/// 持有 [`TracerProvider`] 的所有权，用于在应用生命周期结束时优雅关闭遥测系统。
/// 通过调用 [`shutdown`] 方法可以显式释放资源。
#[derive(Clone)]
pub struct TelemetryHandle {
    tracer_provider: Option<TracerProvider>,
}

impl TelemetryHandle {
    /// 关闭遥测系统并释放资源
    ///
    /// 将内部 `TracerProvider` 设为 `None`，触发所有缓冲的 trace 数据刷新并关闭导出器。
    /// 应在应用退出时调用以确保数据不丢失。
    pub fn shutdown(mut self) {
        self.tracer_provider = None;
    }
}

/// # Errors
///
/// 当 OpenTelemetry 初始化失败时返回错误。
pub fn init_telemetry(service_name: &str) -> crate::Result<TelemetryHandle> {
    let resolved_service_name =
        env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| service_name.to_string());
    let resource = build_resource(&resolved_service_name);
    let tracer_provider = build_tracer_provider(&resource);

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tokio=warn"));
    let log_layer = fmt::layer();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(log_layer)
        .init();

    metrics::init_business_metrics();

    tracing::info!(service.name = %resolved_service_name, "Telemetry 初始化完成");

    Ok(TelemetryHandle {
        tracer_provider: Some(tracer_provider),
    })
}

fn build_resource(service_name: &str) -> Resource {
    Resource::new(vec![
        KeyValue::new(resource_keys::SERVICE_NAME, service_name.to_string()),
        KeyValue::new(
            resource_keys::SERVICE_VERSION,
            crate::API_VERSION.to_string(),
        ),
    ])
}

fn build_tracer_provider(resource: &Resource) -> TracerProvider {
    use opentelemetry::global;
    let sampling_ratio: f64 = env::var("OTEL_TRACES_SAMPLING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let sampler = if sampling_ratio >= 1.0 {
        Sampler::AlwaysOn
    } else if sampling_ratio <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(sampling_ratio)
    };

    let config = Config::default()
        .with_resource(resource.clone())
        .with_sampler(sampler);

    let provider = TracerProvider::builder().with_config(config).build();
    global::set_tracer_provider(provider.clone());
    provider
}

#[must_use]
/// 判断当前是否为生产环境
///
/// 检查 `ENVIRONMENT` 环境变量是否为 `"production"` 或 `"staging"`。
/// 用于控制日志级别、指标采集粒度等运行时行为。
pub fn is_production() -> bool {
    matches!(
        env::var("ENVIRONMENT").as_deref(),
        Ok("production" | "staging")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_default_service_name() {
        assert_eq!(DEFAULT_SERVICE_NAME, "knowledge-api");
    }
    #[test]
    fn test_is_production_detection() {
        let _ = is_production();
    }
}
