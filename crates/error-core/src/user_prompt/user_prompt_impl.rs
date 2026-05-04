//! User-friendly prompt design
//! 
//! This module defines user-friendly prompt design for different severity levels,
//! including template variables and multi-language support.

use std::collections::HashMap;
use crate::classification::Severity;

/// User prompt configuration
/// 
/// Represents the configuration for user-friendly prompts.
pub struct UserPromptConfig {
    /// Default locale
    default_locale: String,
    /// Prompt templates by severity and locale
    templates: HashMap<(Severity, String), String>,
    /// Default prompts by severity
    default_prompts: HashMap<Severity, String>,
}

impl Default for UserPromptConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl UserPromptConfig {
    /// Create a new `UserPromptConfig`
    #[must_use]
    pub fn new() -> Self {
        let mut config = Self {
            default_locale: "en-US".to_string(),
            templates: HashMap::new(),
            default_prompts: HashMap::new(),
        };
        
        // Set default prompts
        config.set_default_prompt(Severity::CRITICAL, "A critical error occurred. Please contact support.");
        config.set_default_prompt(Severity::ERROR, "An error occurred. Please try again later.");
        config.set_default_prompt(Severity::WARNING, "A warning occurred. Please review your actions.");
        config.set_default_prompt(Severity::INFO, "Information: {message}");
        
        config
    }
    
    /// Set the default locale
    pub fn set_default_locale(&mut self, locale: &str) {
        self.default_locale = locale.to_string();
    }
    
    /// Set a default prompt for a severity level
    pub fn set_default_prompt(&mut self, severity: Severity, prompt: &str) {
        self.default_prompts.insert(severity, prompt.to_string());
    }
    
    /// Set a prompt template for a severity level and locale
    pub fn set_template(&mut self, severity: Severity, locale: &str, template: &str) {
        self.templates.insert((severity, locale.to_string()), template.to_string());
    }
    
    /// Get a user-friendly prompt
    #[must_use]
    pub fn get_prompt(&self, severity: Severity, message: &str, locale: Option<&str>) -> String {
        let locale = locale.unwrap_or(&self.default_locale);
        
        // Try to get the template for the specific severity and locale
        self.templates
            .get(&(severity, locale.to_string()))
            .map_or_else(
                || {
                    self.default_prompts
                        .get(&severity)
                        .map_or_else(|| message.to_string(), |prompt| Self::render_template(prompt, message))
                },
                |template| Self::render_template(template, message),
            )
    }
    
    /// Render a template with variables
    #[allow(clippy::literal_string_with_formatting_args)]
    fn render_template(template: &str, message: &str) -> String {
        template.replace("{message}", message)
    }
}

/// User prompt manager
/// 
/// Manages user-friendly prompts for different contexts and environments.
pub struct UserPromptManager {
    config: UserPromptConfig,
}

impl Default for UserPromptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UserPromptManager {
    /// Create a new `UserPromptManager`
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: UserPromptConfig::new(),
        }
    }
    
    /// Get a user-friendly prompt for an error
    #[must_use]
    pub fn get_error_prompt(&self, severity: Severity, message: &str, locale: Option<&str>) -> String {
        self.config.get_prompt(severity, message, locale)
    }
    
    /// Get a user-friendly prompt for a warning
    #[must_use]
    pub fn get_warning_prompt(&self, message: &str, locale: Option<&str>) -> String {
        self.config.get_prompt(Severity::WARNING, message, locale)
    }
    
    /// Get a user-friendly prompt for an info message
    #[must_use]
    pub fn get_info_prompt(&self, message: &str, locale: Option<&str>) -> String {
        self.config.get_prompt(Severity::INFO, message, locale)
    }
    
    /// Configure the prompt manager
    pub fn configure(&mut self, config: UserPromptConfig) {
        self.config = config;
    }
}

/// Terminal UI prompt adapter
/// 
/// Adapts user prompts for terminal UI environments.
pub struct TerminalUIPromptAdapter {
    manager: UserPromptManager,
}

impl TerminalUIPromptAdapter {
    /// Create a new `TerminalUIPromptAdapter`
    #[must_use]
    pub const fn new(manager: UserPromptManager) -> Self {
        Self { manager }
    }
    
    /// Get a terminal-friendly prompt
    #[must_use]
    pub fn get_terminal_prompt(&self, severity: Severity, message: &str, locale: Option<&str>) -> String {
        let base_prompt = self.manager.get_error_prompt(severity, message, locale);
        Self::format_terminal_prompt(severity, &base_prompt)
    }
    
    fn format_terminal_prompt(severity: Severity, prompt: &str) -> String {
        // NOTE: ANSI 转义序列在 Windows 旧版终端上可能不显示颜色。
        // Windows 10+ 需启用虚拟终端处理 (ENABLE_VIRTUAL_TERMINAL_PROCESSING)。
        // 跨平台方案应考虑使用 `console` 或 `termcolor` crate。
        let prefix = match severity {
            Severity::CRITICAL => "\x1b[31m[CRITICAL]\x1b[0m",
            Severity::ERROR => "\x1b[31m[ERROR]\x1b[0m",
            Severity::WARNING => "\x1b[33m[WARNING]\x1b[0m",
            Severity::INFO => "\x1b[32m[INFO]\x1b[0m",
        };
        
        format!("{prefix} {prompt}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_user_prompt_config() {
        let mut config = UserPromptConfig::new();
        config.set_template(Severity::ERROR, "zh-CN", "发生错误: {message}");
        
        let prompt = config.get_prompt(Severity::ERROR, "Network timeout", Some("zh-CN"));
        assert_eq!(prompt, "发生错误: Network timeout");
        
        let prompt_en = config.get_prompt(Severity::ERROR, "Network timeout", Some("en-US"));
        assert_eq!(prompt_en, "An error occurred. Please try again later.");
    }
    
    #[test]
    fn test_terminal_ui_prompt() {
        let manager = UserPromptManager::new();
        let adapter = TerminalUIPromptAdapter::new(manager);
        
        let prompt = adapter.get_terminal_prompt(Severity::ERROR, "Test error", None);
        assert!(prompt.contains("[ERROR]"));
        assert!(prompt.contains("An error occurred. Please try again later."));
    }

    #[test]
    fn test_user_prompt_manager_default() {
        let manager = UserPromptManager::default();
        let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", None);
        assert!(prompt.contains("An error occurred. Please try again later."));
    }

    #[test]
    fn test_user_prompt_manager_warning_prompt() {
        let manager = UserPromptManager::new();
        let prompt = manager.get_warning_prompt("Test warning", None);
        assert!(prompt.contains("A warning occurred. Please review your actions."));
    }

    #[test]
    fn test_user_prompt_manager_info_prompt() {
        let manager = UserPromptManager::new();
        let prompt = manager.get_info_prompt("Test info", None);
        assert!(prompt.contains("Information: Test info"));
    }

    #[test]
    fn test_user_prompt_manager_configure() {
        let mut manager = UserPromptManager::new();
        let mut config = UserPromptConfig::new();
        config.set_template(Severity::ERROR, "zh-CN", "自定义错误: {message}");
        manager.configure(config);
        
        let prompt = manager.get_error_prompt(Severity::ERROR, "Test error", Some("zh-CN"));
        assert_eq!(prompt, "自定义错误: Test error");
    }

    #[test]
    fn test_user_prompt_config_default_branch() {
        let mut config = UserPromptConfig::new();
        // 清空默认提示，触发默认分支
        config.default_prompts.clear();
        
        let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
        assert_eq!(prompt, "Test error");
    }

    #[test]
    fn test_user_prompt_config_set_default_locale() {
        let mut config = UserPromptConfig::new();
        config.set_default_locale("fr-FR");

        // 设置法语模板
        config.set_template(Severity::ERROR, "fr-FR", "Erreur: {message}");

        // 测试默认语言是否生效
        let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
        assert_eq!(prompt, "Erreur: Test error");
    }

    #[test]
    fn test_user_prompt_config_default_impl() {
        // 测试 UserPromptConfig 的 Default 实现
        let config = UserPromptConfig::default();
        let prompt = config.get_prompt(Severity::ERROR, "Test error", None);
        assert!(prompt.contains("An error occurred. Please try again later."));
    }

    #[test]
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
}
