// ============================================================================
// logger.rs
// ============================================================================
//! Centralized logging configuration and initialization manager.
//!
//! The `LoggerManager` validates logging configuration and initializes
//! the global `tracing` subscriber with appropriate layers for console
//! and/or systemd journald output. It supports multiple log formats,
//! ANSI coloring, thread/span information, and environment-based filtering.

use std::io;

use thiserror::Error;
use tracing::instrument;
use tracing_subscriber::{fmt, fmt::format::FmtSpan, prelude::*, EnvFilter, Layer};
use validator::{Validate, ValidationErrors};

use crate::{
    config::logger::{ConsoleConfig, LogFormat, LoggerConfig},
    print_info, print_warn,
};

/// Errors that can occur during logger configuration or initialization.
#[derive(Error, Debug)]
pub enum LoggerError {
    /// General initialization failure with a descriptive message.
    #[error("Logger initialization error: {0}")]
    InitializationError(String),

    /// Validation errors from the logger configuration struct.
    #[error("Logger configuration validation error: {0}")]
    ValidationError(#[from] ValidationErrors),

    /// Failure to parse an environment-based filter directive.
    #[error("Environment filter error: {0}")]
    EnvFilterError(#[from] tracing_subscriber::filter::FromEnvError),

    /// IO error, typically during journald socket operations.
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// No output layers were successfully configured.
    #[error("No logging layers were configured or successfully initialized")]
    NoLayersConfigured,

    /// Journald logger failed to initialize while console output is disabled.
    #[error(
        "Failed to initialize journald logger, and console logger is enabled. Please check your configuration."
    )]
    JournaldFailedWithConsoleEnabled,
}

/// Manages logging configuration and global subscriber initialization.
pub struct LoggerManager {
    config: LoggerConfig,
}

impl LoggerManager {
    /// Creates a new `LoggerManager` and validates the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The logging configuration to use.
    ///
    /// # Errors
    ///
    /// Returns `LoggerError::ValidationError` if configuration validation fails.
    pub fn new(config: LoggerConfig) -> Result<Self, LoggerError> {
        config.validate()?;

        Ok(LoggerManager { config })
    }
    /// Initializes the global `tracing` subscriber with configured layers.
    ///
    /// This method builds console and/or journald layers based on the configuration
    /// and registers them with the global registry. It must be called once at
    /// application startup before any tracing macros are used.
    ///
    /// # Errors
    ///
    /// Returns an error if no valid layers can be created or if journald initialization
    /// fails when it is the only enabled output.
    #[instrument(skip(self))]
    pub fn init(&mut self) -> Result<(), LoggerError> {
        let mut layers = Vec::new();
        match &self.config.console {
            Some(console_config) if console_config.enabled => {
                let console_filter = EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new(&self.config.level));

                let console_layer = self.init_console_logger(console_config, console_filter)?;
                layers.push(console_layer);
            }
            _ => {}
        }
        // Journald layer (Linux/systemd only)
        match &self.config.journald {
            Some(journald_config) if journald_config.enabled => {
                let journald_filter = EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new(&self.config.level));

                match self.init_journald_logger(journald_filter) {
                    Ok(journald_layer) => {
                        layers.push(journald_layer);
                        print_info!(
                            "Systemd journald logger initialized with identifier: {}",
                            journald_config.identifier
                        );
                    }
                    Err(e) => {
                        print_warn!("Failed to initialize systemd journald logger: {}", e);
                        if self.config.console.as_ref().is_some_and(|c| c.enabled) {
                            return Err(LoggerError::JournaldFailedWithConsoleEnabled);
                        }
                    }
                }
            }
            _ => {}
        }
        // Ensure at least one layer is available
        if layers.is_empty() {
            print_warn!("No logging layers were initialized. Please check your configuration.");
            return Err(LoggerError::NoLayersConfigured);
        }
        tracing_subscriber::registry().with(layers).init();
        Ok(())
    }
    /// Constructs a console output layer according to the provided configuration.
    fn init_console_logger(
        &self,
        config: &ConsoleConfig,
        filter: EnvFilter,
    ) -> Result<Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>, LoggerError> {
        let writer = io::stdout;
        let layer = match config.format {
            LogFormat::Json => fmt::layer()
                .json()
                .with_target(config.show_target)
                .with_thread_ids(config.show_thread_ids)
                .with_span_events(if config.show_spans {
                    FmtSpan::CLOSE
                } else {
                    FmtSpan::NONE
                })
                .with_ansi(config.ansi_colors)
                .with_writer(writer)
                .with_filter(filter)
                .boxed(),
            LogFormat::Pretty => fmt::layer()
                .pretty()
                .with_target(config.show_target)
                .with_thread_ids(config.show_thread_ids)
                .with_span_events(if config.show_spans {
                    FmtSpan::CLOSE
                } else {
                    FmtSpan::NONE
                })
                .with_ansi(config.ansi_colors)
                .with_writer(writer)
                .with_filter(filter)
                .boxed(),
            LogFormat::Compact => fmt::layer()
                .compact()
                .with_target(config.show_target)
                .with_thread_ids(config.show_thread_ids)
                .with_span_events(if config.show_spans {
                    FmtSpan::CLOSE
                } else {
                    FmtSpan::NONE
                })
                .with_ansi(config.ansi_colors)
                .with_writer(writer)
                .with_filter(filter)
                .boxed(),
        };

        Ok(layer)
    }

    /// Constructs a journald output layer.
    ///
    /// This uses the `tracing_journald` crate and is only compiled on Unix platforms.
    fn init_journald_logger(
        &self,
        filter: EnvFilter,
    ) -> Result<Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>, LoggerError> {
        let journald_layer = tracing_journald::layer()?;
        Ok(journald_layer.with_filter(filter).boxed())
    }
}
