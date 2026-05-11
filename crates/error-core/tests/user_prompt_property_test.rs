//! User prompt property tests
//!
//! This module contains property tests for the user prompt module to ensure coverage of all possible cases.

use error_core::classification::Severity;
use error_core::user_prompt::{TerminalUIPromptAdapter, UserPromptConfig, UserPromptManager};

#[test]
fn test_user_prompt_config_all_severities() {
    // Test UserPromptConfig with all severity levels
    let config = UserPromptConfig::new();

    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    let test_message = "Test message";

    for severity in &severities {
        let prompt = config.get_prompt(*severity, test_message, None);
        assert!(!prompt.is_empty());

        // Test with a different locale
        let prompt_fr = config.get_prompt(*severity, test_message, Some("fr-FR"));
        assert!(!prompt_fr.is_empty());
    }
}

#[test]
fn test_user_prompt_config_templates() {
    // Test UserPromptConfig with templates for different locales
    let mut config = UserPromptConfig::new();

    // Set templates for different severities and locales
    config.set_template(Severity::CRITICAL, "zh-CN", "严重错误: {message}");
    config.set_template(Severity::ERROR, "zh-CN", "错误: {message}");
    config.set_template(Severity::WARNING, "zh-CN", "警告: {message}");
    config.set_template(Severity::INFO, "zh-CN", "信息: {message}");

    config.set_template(Severity::CRITICAL, "fr-FR", "Erreur critique: {message}");
    config.set_template(Severity::ERROR, "fr-FR", "Erreur: {message}");
    config.set_template(Severity::WARNING, "fr-FR", "Avertissement: {message}");
    config.set_template(Severity::INFO, "fr-FR", "Information: {message}");

    let test_message = "Test message";

    // Test Chinese locale
    assert_eq!(
        config.get_prompt(Severity::CRITICAL, test_message, Some("zh-CN")),
        "严重错误: Test message"
    );
    assert_eq!(
        config.get_prompt(Severity::ERROR, test_message, Some("zh-CN")),
        "错误: Test message"
    );
    assert_eq!(
        config.get_prompt(Severity::WARNING, test_message, Some("zh-CN")),
        "警告: Test message"
    );
    assert_eq!(
        config.get_prompt(Severity::INFO, test_message, Some("zh-CN")),
        "信息: Test message"
    );

    // Test French locale
    assert_eq!(
        config.get_prompt(Severity::CRITICAL, test_message, Some("fr-FR")),
        "Erreur critique: Test message"
    );
    assert_eq!(
        config.get_prompt(Severity::ERROR, test_message, Some("fr-FR")),
        "Erreur: Test message"
    );
    assert_eq!(
        config.get_prompt(Severity::WARNING, test_message, Some("fr-FR")),
        "Avertissement: Test message"
    );
    assert_eq!(
        config.get_prompt(Severity::INFO, test_message, Some("fr-FR")),
        "Information: Test message"
    );

    // Test default locale (en-US)
    assert!(
        config
            .get_prompt(Severity::CRITICAL, test_message, Some("en-US"))
            .contains("critical error")
    );
    assert!(
        config
            .get_prompt(Severity::ERROR, test_message, Some("en-US"))
            .contains("error occurred")
    );
    assert!(
        config
            .get_prompt(Severity::WARNING, test_message, Some("en-US"))
            .contains("warning occurred")
    );
    assert!(
        config
            .get_prompt(Severity::INFO, test_message, Some("en-US"))
            .contains("Information: Test message")
    );
}

#[test]
fn test_user_prompt_config_default_locale() {
    // Test UserPromptConfig with different default locales
    let mut config = UserPromptConfig::new();

    // Set default locale to Chinese
    config.set_default_locale("zh-CN");
    config.set_template(Severity::ERROR, "zh-CN", "错误: {message}");

    let test_message = "Test message";

    // Test that default locale is used when no locale is specified
    let prompt = config.get_prompt(Severity::ERROR, test_message, None);
    assert_eq!(prompt, "错误: Test message");

    // Set default locale to French
    config.set_default_locale("fr-FR");
    config.set_template(Severity::ERROR, "fr-FR", "Erreur: {message}");

    // Test that new default locale is used
    let prompt = config.get_prompt(Severity::ERROR, test_message, None);
    assert_eq!(prompt, "Erreur: Test message");
}

#[test]
fn test_user_prompt_config_edge_cases() {
    // Test UserPromptConfig with edge cases
    let mut config = UserPromptConfig::new();

    // Test with empty message
    let prompt = config.get_prompt(Severity::ERROR, "", None);
    assert!(!prompt.is_empty());

    // Test with empty locale
    let prompt = config.get_prompt(Severity::ERROR, "Test message", Some(""));
    assert!(!prompt.is_empty());

    // Test with non-existent locale
    let prompt = config.get_prompt(Severity::ERROR, "Test message", Some("xx-XX"));
    assert!(!prompt.is_empty());

    // Test with a custom template
    config.set_template(Severity::ERROR, "en-US", "Custom error: {message}");
    let prompt = config.get_prompt(Severity::ERROR, "Test message", Some("en-US"));
    assert_eq!(prompt, "Custom error: Test message");
}

#[test]
fn test_user_prompt_manager_all_methods() {
    // Test all methods in UserPromptManager
    let mut manager = UserPromptManager::new();

    let test_message = "Test message";

    // Test get_error_prompt with all severities
    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    for severity in &severities {
        let prompt = manager.get_error_prompt(*severity, test_message, None);
        assert!(!prompt.is_empty());
    }

    // Test get_warning_prompt
    let warning_prompt = manager.get_warning_prompt(test_message, None);
    assert!(!warning_prompt.is_empty());

    // Test get_info_prompt
    let info_prompt = manager.get_info_prompt(test_message, None);
    assert!(!info_prompt.is_empty());

    // Test configure
    let mut custom_config = UserPromptConfig::new();
    custom_config.set_template(Severity::ERROR, "zh-CN", "自定义错误: {message}");
    manager.configure(custom_config);

    let prompt = manager.get_error_prompt(Severity::ERROR, test_message, Some("zh-CN"));
    assert_eq!(prompt, "自定义错误: Test message");
}

#[test]
fn test_terminal_ui_prompt_all_severities() {
    // Test TerminalUIPromptAdapter with all severity levels
    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    let test_message = "Test message";
    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    for severity in &severities {
        let prompt = adapter.get_terminal_prompt(*severity, test_message, None);
        assert!(!prompt.is_empty());

        // Test with a different locale
        let prompt_fr = adapter.get_terminal_prompt(*severity, test_message, Some("fr-FR"));
        assert!(!prompt_fr.is_empty());
    }
}

#[test]
fn test_terminal_ui_prompt_edge_cases() {
    // Test TerminalUIPromptAdapter with edge cases
    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    // Test with empty message
    let prompt = adapter.get_terminal_prompt(Severity::ERROR, "", None);
    assert!(!prompt.is_empty());

    // Test with empty locale
    let prompt = adapter.get_terminal_prompt(Severity::ERROR, "Test message", Some(""));
    assert!(!prompt.is_empty());

    // Test with non-existent locale
    let prompt = adapter.get_terminal_prompt(Severity::ERROR, "Test message", Some("xx-XX"));
    assert!(!prompt.is_empty());
}

#[test]
fn test_user_prompt_config_render_template() {
    // Test the render_template method indirectly through get_prompt
    let mut config = UserPromptConfig::new();

    // Set a template with multiple {message} placeholders
    config.set_template(
        Severity::ERROR,
        "en-US",
        "Error: {message}. Please see {message} for details.",
    );

    let test_message = "Network timeout";
    let prompt = config.get_prompt(Severity::ERROR, test_message, Some("en-US"));
    assert_eq!(
        prompt,
        "Error: Network timeout. Please see Network timeout for details."
    );
}

#[test]
fn test_user_prompt_manager_default_impl() {
    // Test UserPromptManager default implementation
    let manager = UserPromptManager::default();

    let test_message = "Test message";
    let prompt = manager.get_error_prompt(Severity::ERROR, test_message, None);
    assert!(!prompt.is_empty());
    assert!(prompt.contains("error occurred"));
}

#[test]
fn test_user_prompt_config_set_default_prompt() {
    // Test set_default_prompt method
    let mut config = UserPromptConfig::new();

    // Override default prompts
    config.set_default_prompt(Severity::CRITICAL, "Custom critical error message");
    config.set_default_prompt(Severity::ERROR, "Custom error message");
    config.set_default_prompt(Severity::WARNING, "Custom warning message");
    config.set_default_prompt(Severity::INFO, "Custom info message: {message}");

    let test_message = "Test message";

    assert_eq!(
        config.get_prompt(Severity::CRITICAL, test_message, None),
        "Custom critical error message"
    );
    assert_eq!(
        config.get_prompt(Severity::ERROR, test_message, None),
        "Custom error message"
    );
    assert_eq!(
        config.get_prompt(Severity::WARNING, test_message, None),
        "Custom warning message"
    );
    assert_eq!(
        config.get_prompt(Severity::INFO, test_message, None),
        "Custom info message: Test message"
    );
}

#[test]
fn test_terminal_ui_prompt_formatting() {
    // Test that terminal prompts are properly formatted with severity prefixes
    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    let test_message = "Test message";

    let critical_prompt = adapter.get_terminal_prompt(Severity::CRITICAL, test_message, None);
    assert!(critical_prompt.contains("[CRITICAL]"));

    let error_prompt = adapter.get_terminal_prompt(Severity::ERROR, test_message, None);
    assert!(error_prompt.contains("[ERROR]"));

    let warning_prompt = adapter.get_terminal_prompt(Severity::WARNING, test_message, None);
    assert!(warning_prompt.contains("[WARNING]"));

    let info_prompt = adapter.get_terminal_prompt(Severity::INFO, test_message, None);
    assert!(info_prompt.contains("[INFO]"));
}
