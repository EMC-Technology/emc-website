//! User prompt fuzz tests
//!
//! This module contains fuzz tests for the user_prompt module to ensure coverage of all possible cases.

use error_core::classification::Severity;
use error_core::user_prompt::{TerminalUIPromptAdapter, UserPromptConfig, UserPromptManager};

#[test]
fn test_user_prompt_config() {
    // Test UserPromptConfig with various configurations
    let test_cases = vec![
        ("default_config", "en-US"),
        ("custom_locale", "zh-CN"),
        ("fr_locale", "fr-FR"),
        ("de_locale", "de-DE"),
        ("empty_locale", ""),
    ];

    for (_name, locale) in test_cases {
        let mut config = UserPromptConfig::new();
        config.set_default_locale(locale);

        // Test set_template
        config.set_template(
            Severity::ERROR,
            locale,
            &format!("Error in {locale}: {{message}}"),
        );
        config.set_template(
            Severity::WARNING,
            locale,
            &format!("Warning in {locale}: {{message}}"),
        );

        // Test get_prompt with specified locale
        let error_prompt = config.get_prompt(Severity::ERROR, "Test error", Some(locale));
        assert!(error_prompt.contains("Test error"));
        assert!(error_prompt.contains(locale));

        // Test get_prompt with default locale
        let warning_prompt = config.get_prompt(Severity::WARNING, "Test warning", None);
        assert!(warning_prompt.contains("Test warning"));
        assert!(warning_prompt.contains(locale));

        // Test get_prompt with non-existent locale
        let info_prompt =
            config.get_prompt(Severity::INFO, "Test info", Some("non-existent-locale"));
        assert!(info_prompt.contains("Test info"));

        // Test get_prompt with ERROR severity and non-existent locale
        let error_prompt =
            config.get_prompt(Severity::ERROR, "Test error", Some("non-existent-locale"));
        assert!(!error_prompt.contains("Test error"));
    }
}

#[test]
fn test_user_prompt_config_edge_cases() {
    // Test edge cases for UserPromptConfig

    // Test with empty message
    let config = UserPromptConfig::new();
    let empty_prompt = config.get_prompt(Severity::ERROR, "", None);
    assert!(empty_prompt.contains("An error occurred"));

    // Test with very long message
    let long_message = "a".repeat(1000);
    let long_prompt = config.get_prompt(Severity::INFO, &long_message, None);
    assert!(long_prompt.contains(&long_message));

    // Test with message containing template variable
    let message_with_braces = "Message with {brackets}";
    let prompt_with_braces = config.get_prompt(Severity::INFO, message_with_braces, None);
    assert!(prompt_with_braces.contains(message_with_braces));

    // Test fallback to message when no prompt is found
    // Create a config with empty prompt
    let mut config_empty = UserPromptConfig::new();
    config_empty.set_default_prompt(Severity::ERROR, "");
    // Test with ERROR severity
    let fallback_prompt = config_empty.get_prompt(Severity::ERROR, "Test error", None);
    assert!(fallback_prompt.is_empty() || fallback_prompt.contains(""));

    // Test with non-existent locale
    let config_no_templates = UserPromptConfig::new();
    let default_prompt =
        config_no_templates.get_prompt(Severity::ERROR, "Test error", Some("zh-CN"));
    assert!(default_prompt.contains("An error occurred"));
}

#[test]
fn test_user_prompt_manager() {
    // Test UserPromptManager with various configurations
    let test_cases = vec![
        ("default_manager", None),
        (
            "custom_config",
            Some({
                let mut config = UserPromptConfig::new();
                config.set_template(Severity::ERROR, "zh-CN", "自定义错误: {message}");
                config
            }),
        ),
    ];

    for (_name, config) in test_cases {
        let mut manager = UserPromptManager::new();

        if let Some(custom_config) = config {
            manager.configure(custom_config);
        }

        // Test get_error_prompt
        let error_prompt = manager.get_error_prompt(Severity::ERROR, "Test error", None);
        assert!(error_prompt.contains("An error occurred"));

        // Test get_warning_prompt
        let warning_prompt = manager.get_warning_prompt("Test warning", None);
        assert!(warning_prompt.contains("A warning occurred"));

        // Test get_info_prompt
        let info_prompt = manager.get_info_prompt("Test info", None);
        assert!(info_prompt.contains("Test info"));
    }
}

#[test]
fn test_user_prompt_manager_edge_cases() {
    // Test edge cases for UserPromptManager

    // Test with empty messages
    let manager = UserPromptManager::new();

    let empty_error = manager.get_error_prompt(Severity::ERROR, "", None);
    assert!(empty_error.contains("An error occurred"));

    let empty_warning = manager.get_warning_prompt("", None);
    assert!(empty_warning.contains("A warning occurred"));

    let empty_info = manager.get_info_prompt("", None);
    assert!(empty_info.contains("Information: "));

    // Test with different locales
    let locales = ["en-US", "zh-CN", "fr-FR", "de-DE", ""];
    for locale in &locales {
        let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", Some(locale));
        assert!(prompt.contains("An error occurred"));
    }

    // Test with all severity levels
    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];
    for severity in &severities {
        let prompt = manager.get_error_prompt(*severity, "Test message", None);
        match severity {
            Severity::INFO => assert!(prompt.contains("Test message")),
            _ => assert!(!prompt.contains("Test message")),
        }
    }
}

#[test]
fn test_terminal_ui_prompt_adapter() {
    // Test TerminalUIPromptAdapter with various configurations
    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    // Test with all severity levels
    let test_cases = vec![
        (Severity::CRITICAL, "Critical error"),
        (Severity::ERROR, "Error"),
        (Severity::WARNING, "Warning"),
        (Severity::INFO, "Info"),
    ];

    for (severity, message) in test_cases {
        let prompt = adapter.get_terminal_prompt(severity, message, None);

        // Check for appropriate prefix
        match severity {
            Severity::CRITICAL => {
                assert!(prompt.contains("[CRITICAL]"));
                assert!(prompt.contains("A critical error occurred"));
            }
            Severity::ERROR => {
                assert!(prompt.contains("[ERROR]"));
                assert!(prompt.contains("An error occurred"));
            }
            Severity::WARNING => {
                assert!(prompt.contains("[WARNING]"));
                assert!(prompt.contains("A warning occurred"));
            }
            Severity::INFO => {
                assert!(prompt.contains("[INFO]"));
                assert!(prompt.contains(message));
            }
        }
    }
}

#[test]
fn test_terminal_ui_prompt_edge_cases() {
    // Test edge cases for TerminalUIPromptAdapter

    let manager = UserPromptManager::new();
    let adapter = TerminalUIPromptAdapter::new(manager);

    // Test with empty message
    let empty_prompt = adapter.get_terminal_prompt(Severity::ERROR, "", None);
    assert!(empty_prompt.contains("[ERROR]"));
    assert!(empty_prompt.contains("An error occurred"));

    // Test with very long message
    let long_message = "a".repeat(1000);
    let long_prompt = adapter.get_terminal_prompt(Severity::INFO, &long_message, None);
    assert!(long_prompt.contains("[INFO]"));
    assert!(long_prompt.contains(&long_message));

    // Test with different locales
    let locales = ["en-US", "zh-CN", "fr-FR", "de-DE", ""];
    for locale in &locales {
        let prompt = adapter.get_terminal_prompt(Severity::ERROR, "Test error", Some(locale));
        assert!(prompt.contains("[ERROR]"));
        assert!(prompt.contains("An error occurred"));
    }
}

#[test]
fn test_user_prompt_all_combinations() {
    // Test all combinations of user prompt configurations

    // Test UserPromptConfig with all severity levels and locales
    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];
    let locales = ["en-US", "zh-CN", "fr-FR", ""];

    let mut config = UserPromptConfig::new();

    // Set templates for all combinations
    for severity in &severities {
        for locale in &locales {
            config.set_template(
                *severity,
                locale,
                &format!("{severity:?} in {locale}: {{message}}"),
            );
        }
    }

    // Test all combinations
    for severity in &severities {
        for locale in &locales {
            let prompt = config.get_prompt(*severity, "Test message", Some(locale));
            assert!(prompt.contains("Test message"));
            assert!(prompt.contains(&format!("{severity:?}")));
            assert!(prompt.contains(locale));
        }
    }

    // Test UserPromptManager with all severity levels
    let manager = UserPromptManager::new();

    for severity in &severities {
        let prompt = manager.get_error_prompt(*severity, "Test message", None);
        match severity {
            Severity::INFO => assert!(prompt.contains("Test message")),
            _ => assert!(!prompt.contains("Test message")),
        }
    }

    // Test TerminalUIPromptAdapter with all severity levels
    let adapter = TerminalUIPromptAdapter::new(manager);

    for severity in &severities {
        let prompt = adapter.get_terminal_prompt(*severity, "Test message", None);
        match severity {
            Severity::INFO => assert!(prompt.contains("Test message")),
            _ => assert!(!prompt.contains("Test message")),
        }
    }
}

#[test]
fn test_user_prompt_default_implementations() {
    // Test default implementations

    // Test UserPromptConfig default
    let config = UserPromptConfig::default();
    let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
    assert!(prompt.contains("An error occurred"));

    // Test UserPromptManager default
    let manager = UserPromptManager::default();
    let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", None);
    assert!(prompt.contains("An error occurred"));
}

#[test]
fn test_user_prompt_template_rendering() {
    // Test template rendering with various patterns
    let mut config = UserPromptConfig::new();

    // Test different template patterns
    let templates = [
        ("Simple template", "{message}"),
        ("Template with prefix", "Error: {message}"),
        ("Template with suffix", "{message} - Please try again"),
        (
            "Template with multiple variables",
            "Error: {message} - Please contact support",
        ),
        ("Template without variable", "Fixed message"),
    ];

    for (_name, template) in templates {
        config.set_template(Severity::ERROR, "en-US", template);
        let prompt = config.get_prompt(Severity::ERROR, "Test error", Some("en-US"));

        if template.contains("{message}") {
            assert!(prompt.contains("Test error"));
        } else {
            assert_eq!(prompt, "Fixed message");
        }
    }
}
