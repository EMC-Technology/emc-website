use error_core::prelude::*;
use error_core::user_prompt::UserPromptConfig;

#[test]
fn test_user_prompt_integration() {
    // 测试用户提示系统集成

    // 测试默认用户提示管理器
    let prompt_manager = UserPromptManager::default();
    let error_prompt =
        prompt_manager.get_error_prompt(Severity::ERROR, "User not found", Some("zh-CN"));
    assert!(!error_prompt.is_empty());

    // 测试不同级别的提示
    let warning_prompt =
        prompt_manager.get_warning_prompt("Password will expire soon", Some("zh-CN"));
    assert!(!warning_prompt.is_empty());

    let info_prompt = prompt_manager.get_info_prompt("Profile updated successfully", Some("zh-CN"));
    assert!(!info_prompt.is_empty());

    // 测试用户提示配置
    let mut config = UserPromptConfig::default();
    config.set_default_locale("zh-CN");
    let mut configured_manager = UserPromptManager::new();
    configured_manager.configure(config);
    let configured_prompt =
        configured_manager.get_error_prompt(Severity::ERROR, "User not found", Some("zh-CN"));
    assert!(!configured_prompt.is_empty());
}
