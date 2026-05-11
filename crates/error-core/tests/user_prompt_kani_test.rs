//! User prompt module formal verification tests using Kani
#![cfg(kani)]

use error_core::classification::Severity;
use error_core::user_prompt::*;

// Test UserPromptConfig
#[kani::proof]
fn test_user_prompt_config() {
    let mut config = UserPromptConfig::new();
    config.set_template(Severity::ERROR, "zh-CN", "发生错误: {message}");

    let prompt = config.get_prompt(Severity::ERROR, "Network timeout", Some("zh-CN"));
    assert_eq!(prompt, "发生错误: Network timeout");

    let prompt_en = config.get_prompt(Severity::ERROR, "Network timeout", Some("en-US"));
    assert_eq!(prompt_en, "An error occurred. Please try again later.");
}

// Test UserPromptConfig default branch
#[kani::proof]
fn test_user_prompt_config_default_branch() {
    let mut config = UserPromptConfig::new();
    // 清空默认提示，触发默认分支
    config.default_prompts.clear();

    let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
    assert_eq!(prompt, "Test error");
}

// Test UserPromptConfig set_default_locale
#[kani::proof]
fn test_user_prompt_config_set_default_locale() {
    let mut config = UserPromptConfig::new();
    config.set_default_locale("fr-FR");

    // 设置法语模板
    config.set_template(Severity::ERROR, "fr-FR", "Erreur: {message}");

    // 测试默认语言是否生效
    let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
    assert_eq!(prompt, "Erreur: Test error");
}

// Test UserPromptConfig set_default_prompt
#[kani::proof]
fn test_user_prompt_config_set_default_prompt() {
    let mut config = UserPromptConfig::new();
    config.set_default_prompt(Severity::ERROR, "Custom error: {message}");

    let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
    assert_eq!(prompt, "Custom error: Test error");
}

// Test UserPromptConfig default implementation
#[kani::proof]
fn test_user_prompt_config_default_impl() {
    // 测试 UserPromptConfig 的 Default 实现
    let config = UserPromptConfig::default();
    let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
    assert!(prompt.contains("An error occurred. Please try again later."));
}

// Test UserPromptManager
#[kani::proof]
fn test_user_prompt_manager() {
    let manager = UserPromptManager::new();
    let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", None);
    assert!(prompt.contains("An error occurred. Please try again later."));
}

// Test UserPromptManager default implementation
#[kani::proof]
fn test_user_prompt_manager_default() {
    let manager = UserPromptManager::default();
    let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", None);
    assert!(prompt.contains("An error occurred. Please try again later."));
}

// Test UserPromptManager warning prompt
#[kani::proof]
fn test_user_prompt_manager_warning_prompt() {
    let manager = UserPromptManager::new();
    let prompt = manager.get_warning_prompt("Test warning", None);
    assert!(prompt.contains("A warning occurred. Please review your actions."));
}

// Test UserPromptManager info prompt
#[kani::proof]
fn test_user_prompt_manager_info_prompt() {
    let manager = UserPromptManager::new();
    let prompt = manager.get_info_prompt("Test info", None);
    assert!(prompt.contains("Information: Test info"));
}

// Test UserPromptManager configure
#[kani::proof]
fn test_user_prompt_manager_configure() {
    let mut manager = UserPromptManager::new();
    let mut config = UserPromptConfig::new();
    config.set_template(Severity::ERROR, "zh-CN", "自定义错误: {message}");
    manager.configure(config);

    let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", Some("zh-CN"));
    assert_eq!(prompt, "自定义错误: Test error");
}

// Test TerminalUIPromptAdapter
#[kani::proof]
fn test_terminal_ui_prompt() {
    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    let prompt = adapter.get_terminal_prompt(Severity::ERROR, "Test error", None);
    assert!(prompt.contains("[ERROR]"));
    assert!(prompt.contains("An error occurred. Please try again later."));
}

// Test TerminalUIPromptAdapter with all severities
#[kani::proof]
fn test_terminal_ui_prompt_all_severities() {
    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    // 测试所有严重程度的提示
    let critical_prompt = adapter.get_terminal_prompt(Severity::CRITICAL, "Critical error", None);
    assert!(critical_prompt.contains("[CRITICAL]"));

    let error_prompt = adapter.get_terminal_prompt(Severity::ERROR, "Error", None);
    assert!(error_prompt.contains("[ERROR]"));

    let warning_prompt = adapter.get_terminal_prompt(Severity::WARNING, "Warning", None);
    assert!(warning_prompt.contains("[WARNING]"));

    let info_prompt = adapter.get_terminal_prompt(Severity::INFO, "Info", None);
    assert!(info_prompt.contains("[INFO]"));
}
