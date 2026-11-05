//! Logging configuration.

use serde::Deserialize;

/// Console log layout.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Compact,
    Pretty,
    Json,
}

/// Timestamp style for console output.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimestampFormat {
    /// `2024-05-01T12:00:00.123456Z`
    #[default]
    #[serde(alias = "Rfc3339")]
    Rfc3339,
    /// Seconds since the Unix epoch with microseconds, e.g. `1714564800.123456`.
    #[serde(alias = "Unix")]
    Unix,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoggerConfig {
    /// A level (`trace`, `debug`, `info`, `warn`, `error`, `off`) or a full
    /// `tracing` directive list such as `info,rumqttc=warn`. `RUST_LOG`
    /// overrides this value when set.
    pub level: String,
    pub timestamp_format: TimestampFormat,
    pub console: ConsoleConfig,
    pub journald: JournaldConfig,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            timestamp_format: TimestampFormat::default(),
            console: ConsoleConfig::default(),
            journald: JournaldConfig::default(),
        }
    }
}

impl LoggerConfig {
    pub fn validate(&self) -> Result<(), String> {
        // A bare word must be a level; EnvFilter would accept any word as a target name.
        if self.level.contains(['=', ',']) {
            tracing_subscriber::EnvFilter::try_new(&self.level)
                .map_err(|e| format!("logger.level: {e}"))?;
        } else {
            self.level
                .parse::<tracing_subscriber::filter::LevelFilter>()
                .map_err(|_| format!("logger.level: '{}' is not a log level", self.level))?;
        }
        if self.journald.enabled && self.journald.identifier.is_empty() {
            return Err("logger.journald.identifier must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConsoleConfig {
    pub enabled: bool,
    pub format: LogFormat,
    /// Include the module path of the event.
    pub show_target: bool,
    pub show_thread_ids: bool,
    /// Emit span close events with timing.
    pub show_spans: bool,
    pub ansi_colors: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: LogFormat::Compact,
            show_target: false,
            show_thread_ids: false,
            show_spans: false,
            ansi_colors: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JournaldConfig {
    pub enabled: bool,
    /// `SYSLOG_IDENTIFIER` of the journal entries.
    pub identifier: String,
}

impl Default for JournaldConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            identifier: "blazebee".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formats_with_aliases() {
        let c: LoggerConfig = toml::from_str(
            "level = \"debug,rumqttc=warn\"\ntimestamp_format = \"Unix\"\n[console]\nformat = \"json\"\n[journald]\nenabled = true\n",
        )
        .unwrap();
        assert_eq!(c.timestamp_format, TimestampFormat::Unix);
        assert_eq!(c.console.format, LogFormat::Json);
        assert_eq!(c.journald.identifier, "blazebee");
        c.validate().unwrap();
    }

    #[test]
    fn rejects_bad_level_and_empty_identifier() {
        let c = LoggerConfig {
            level: "loud".into(),
            ..Default::default()
        };
        assert!(c.validate().unwrap_err().contains("logger.level"));
        let c = LoggerConfig {
            level: "info,nosuchlevel".into(),
            ..Default::default()
        };
        assert!(c.validate().is_ok(), "directive lists are passed through");
        let c = LoggerConfig {
            journald: JournaldConfig {
                enabled: true,
                identifier: String::new(),
            },
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }
}
