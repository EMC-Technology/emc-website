use error_core::prelude::*;
use error_core::logging::LoggingUtils;
use error_core::logging::LogFileManager;
use error_core::error_capture::BusinessErrorCapture;

#[test]
fn test_logging_integration() {
    // 测试日志系统集成
    #[derive(Debug, thiserror::Error)]
    #[error("User not found")]
    struct TestError;

    let capture = BusinessErrorCapture::new("user_service", "login");
    let error = capture.capture_error(&TestError);

    // 测试日志条目创建
    let _log_entry = LogEntry::from_error_object(&error);

    // 测试日志文件管理器
    let log_manager = LogFileManager::new("./logs");
    log_manager.rotate_logs();

    // 测试不同级别的日志
    LoggingUtils::log_error(&error);

    // 测试配置日志系统
    LoggingUtils::configure_logging("./logs");
}
