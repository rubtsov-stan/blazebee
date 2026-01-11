//! Logging configuration structures and validation logic.
//!
//! This module defines the configuration types used for initializing the
//! application's logging subsystem. All structures support serialization
//! and deserialization via `serde` and include validation rules enforced
//! by the `validator` crate.

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Available formats for console log output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "pretty")]
    Pretty,
    #[serde(rename = "json")]
    Json,
}

impl Default for LogFormat {
    fn default() -> Self {
        LogFormat::Compact
    }
}

/// Formats available for timestamp representation in log entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimestampFormat {
    Rfc3339,
    Unix,
    Custom(String),
}

impl Default for TimestampFormat {
    fn default() -> Self {
        TimestampFormat::Rfc3339
    }
}

/// Top-level logging configuration.
///
/// Controls global log level, timestamp format, and output targets (console and/or journald).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct LoggerConfig {
    /// Global log level. Valid values: trace, debug, info, warn, error (case-insensitive).
    #[validate(custom(function = "validate_log_level"))]
    pub level: String,

    /// Optional console output configuration.
    #[validate(nested)]
    pub console: Option<ConsoleConfig>,

    /// Optional systemd journald output configuration.
    #[validate(nested)]
    pub journald: Option<JournaldConfig>,

    /// Timestamp format used across all outputs.
    #[validate(custom(function = "validate_timestamp_format"))]
    pub timestamp_format: TimestampFormat,
}

/// Validates that a custom timestamp format string is non-empty.
fn validate_timestamp_format(format: &TimestampFormat) -> Result<(), ValidationError> {
    match format {
        TimestampFormat::Custom(s) if s.is_empty() => {
            let mut err = ValidationError::new("invalid_timestamp_format");
            err.message = Some("Custom timestamp format cannot be empty".into());
            Err(err)
        }
        _ => Ok(()),
    }
}

/// Validates that the provided log level is one of the supported values.
fn validate_log_level(level: &str) -> Result<(), ValidationError> {
    match level.to_lowercase().as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => Ok(()),
        _ => {
            let mut err = ValidationError::new("invalid_log_level");
            err.message = Some(format!("Invalid log level: {}", level).into());
            Err(err)
        }
    }
}

impl Default for LoggerConfig {
    fn default() -> Self {
        LoggerConfig {
            level: "info".to_string(),
            timestamp_format: TimestampFormat::default(),
            console: Some(ConsoleConfig::default()),
            journald: Some(JournaldConfig::default()),
        }
    }
}

/// Configuration for console log output.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct ConsoleConfig {
    /// Whether console output is enabled.
    pub enabled: bool,

    /// Output format for console logs.
    #[serde(default)]
    pub format: LogFormat,

    /// Include the log target (module path) in output.
    pub show_target: bool,

    /// Include thread IDs in output.
    pub show_thread_ids: bool,

    /// Include span entry/exit events in output.
    pub show_spans: bool,

    /// Enable ANSI color codes in console output.
    pub ansi_colors: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        ConsoleConfig {
            enabled: true,
            format: LogFormat::default(),
            show_target: false,
            show_thread_ids: false,
            show_spans: false,
            ansi_colors: true,
        }
    }
}

/// Configuration for systemd journald output (Unix only).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct JournaldConfig {
    /// Whether journald output is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Identifier used for journal entries. Must be non-empty.
    #[validate(length(min = 1))]
    pub identifier: String,
}

impl Default for JournaldConfig {
    fn default() -> Self {
        JournaldConfig {
            enabled: false,
            identifier: "blazebee".to_string(),
        }
    }
}
