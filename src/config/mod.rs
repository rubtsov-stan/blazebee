//! Application configuration loading, validation, and management.
//!
//! This module provides the top-level `Config` structure that aggregates
//! logging, metrics, and transport configurations. It handles loading from
//! TOML files, environment overrides, validation, and optional creation of
//! default configuration files.
//!
//! The configuration is loaded early in the application lifecycle and is
//! intended to remain immutable thereafter.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use validator::Validate;

use super::config::{logger::LoggerConfig, metrics::MetricsConfig};

pub mod logger;
pub mod metrics;

/// Simple macros for printing timestamped messages before the tracing subscriber
/// is initialized. These are used during early configuration loading.
#[macro_export]
macro_rules! print_info {
    ($($arg:tt)*) => {
        println!("{} INFO {}",
            time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! print_warn {
    ($($arg:tt)*) => {
        println!("{} WARN {}",
            time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! print_error {
    ($($arg:tt)*) => {
        println!("{} ERROR {}",
            time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            format_args!($($arg)*)
        );
    };
}

/// Errors that can occur during configuration loading, parsing, validation,
/// or serialization.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Generic configuration-related error with a descriptive message.
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO error while accessing configuration files.
    #[error("IO error while reading configuration: {0}")]
    IoError(#[from] std::io::Error),

    /// Failure to parse the TOML configuration file.
    #[error("Parse error while reading configuration: {0}")]
    ParseError(String),

    /// Validation failure after successful parsing.
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Failure during serialization (e.g., when saving a configuration file).
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Top-level application configuration.
///
/// Combines logging, metrics, and transport settings into a single structure.
/// The `transport` field is flattened from the underlying transport-specific
/// configuration when the `blazebee-mqtt-v4` feature is enabled.
#[derive(Serialize, Deserialize, Debug, Validate, Clone, Default)]
#[serde(default)]
pub struct Config {
    /// Logging subsystem configuration.
    pub logger: LoggerConfig,

    /// Metrics collection and publishing configuration.
    pub metrics: MetricsConfig,

    /// Transport-layer configuration (flattened for ergonomic TOML usage).
    pub transport: TransportConfig,
}

#[cfg(feature = "blazebee-mqtt-v4")]
pub type TransportConfig = blazebee_mqtt_v4::Config;

impl Config {
    /// Constructs a new configuration by locating and loading the config file.
    ///
    /// # Errors
    ///
    /// Returns a `ConfigError` if the configuration file cannot be found,
    /// read, parsed, or validated.
    pub fn new() -> Result<Self, ConfigError> {
        let config_path = Self::get_config_path()?;
        Self::load(&config_path)
    }

    /// Determines the configuration file path.
    ///
    /// Priority:
    /// 1. `BLAZEBEE_CONFIG` environment variable
    /// 2. `/etc/blazebee/config.toml`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::Config` if no suitable file is found.
    fn get_config_path() -> Result<PathBuf, ConfigError> {
        if let Ok(config_path) = std::env::var("BLAZEBEE_CONFIG") {
            let path = PathBuf::from(config_path);
            print_info!("Using config from BLAZEBEE_CONFIG: {}", path.display());
            return Ok(path);
        }

        let fallback = Path::new("/etc/blazebee/config.toml");
        if fallback.exists() {
            print_info!("Using default config path: {}", fallback.display());
            return Ok(fallback.to_path_buf());
        }

        Err(ConfigError::Config(
            "No configuration file found.".to_string(),
        ))
    }

    /// Loads and validates configuration from the specified path.
    ///
    /// # Errors
    ///
    /// Propagates IO, parsing, and validation errors as `ConfigError`.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        print_info!("Loading configuration from: {}", path.display());

        if !path.exists() {
            return Err(ConfigError::Config(path.to_string_lossy().to_string()));
        }

        let config_str = fs::read_to_string(path)?;
        let config: Config =
            toml::from_str(&config_str).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        config
            .validate()
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;

        print_info!("Successfully loaded config from: {}", path.display());
        Ok(config)
    }

    /// Serializes and writes the configuration to the specified path.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::SerializationError` or IO error on failure.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let config_str = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        fs::write(path, config_str)?;
        print_info!("Config saved to: {}", path.display());
        Ok(())
    }

    /// Writes a default configuration file to a standard location if it does not already exist.
    ///
    /// This is typically used during first-run setup or installation scripts.
    ///
    /// # Errors
    ///
    /// Propagates errors from serialization or file writing.
    pub fn write_default_config(&self) -> Result<(), ConfigError> {
        let default_path = Path::new("/var/lib/blazebee/config.toml");
        if default_path.exists() {
            print_warn!(
                "Default config not written: already exists at {}",
                default_path.display()
            );
            return Ok(());
        }

        self.save(default_path)?;
        print_info!(
            "Default configuration created at {}",
            default_path.display()
        );
        Ok(())
    }
}
