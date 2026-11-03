//! Configuration loading and validation.
//!
//! The file is located through `BLAZEBEE_CONFIG` or `/etc/blazebee/config.toml`,
//! `${NAME}` references are replaced with environment variables, the TOML is
//! parsed with unknown keys rejected, and every section is validated before
//! the process does anything else.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::core::collectors;

pub mod env;
pub mod logger;
pub mod metrics;

pub use logger::LoggerConfig;
pub use metrics::MetricsConfig;

pub const ENV_CONFIG_PATH: &str = "BLAZEBEE_CONFIG";
pub const DEFAULT_CONFIG_PATH: &str = "/etc/blazebee/config.toml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no configuration file: set {ENV_CONFIG_PATH} or create {DEFAULT_CONFIG_PATH}")]
    NotFound,

    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path}: {source}")]
    Env {
        path: PathBuf,
        #[source]
        source: env::EnvError,
    },

    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("{0}")]
    Invalid(String),

    #[error(transparent)]
    Transport(#[from] blazebee_mqtt_v3::ConfigError),

    #[error("metrics: collector '{name}' is not available in this build; available: {available}")]
    UnknownCollector { name: String, available: String },
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub logger: LoggerConfig,
    pub metrics: MetricsConfig,
    pub transport: blazebee_mqtt_v3::Config,
    #[serde(skip)]
    source: PathBuf,
}

impl Config {
    /// Loads the configuration from `BLAZEBEE_CONFIG` or the default path.
    pub fn load() -> Result<Config, ConfigError> {
        let path = match std::env::var_os(ENV_CONFIG_PATH) {
            Some(path) => PathBuf::from(path),
            None if Path::new(DEFAULT_CONFIG_PATH).is_file() => PathBuf::from(DEFAULT_CONFIG_PATH),
            None => return Err(ConfigError::NotFound),
        };
        Self::from_path(&path)
    }

    pub fn from_path(path: &Path) -> Result<Config, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text, path, |name| std::env::var(name).ok())
    }

    /// Parses `text`, resolving `${NAME}` in string values through `lookup`,
    /// then validates.
    pub fn parse(
        text: &str,
        path: &Path,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Config, ConfigError> {
        let parse_error = |source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        };
        let mut table: toml::Value = toml::from_str(text).map_err(parse_error)?;
        env::expand_value(&mut table, &lookup).map_err(|source| ConfigError::Env {
            path: path.to_path_buf(),
            source,
        })?;
        let mut config: Config = table.try_into().map_err(parse_error)?;
        config.source = path.to_path_buf();
        if config.transport.client_id.is_empty() {
            config.transport.client_id = format!("blazebee-{}", hostname());
        }
        config.validate()?;
        Ok(config)
    }

    /// Path the configuration was loaded from.
    pub fn source(&self) -> &Path {
        &self.source
    }

    fn validate(&self) -> Result<(), ConfigError> {
        self.logger.validate().map_err(ConfigError::Invalid)?;
        self.metrics.validate().map_err(ConfigError::Invalid)?;
        for entry in &self.metrics.collectors.enabled {
            if collectors::find(&entry.name).is_none() {
                let mut available: Vec<&str> = collectors::names().collect();
                available.sort_unstable();
                return Err(ConfigError::UnknownCollector {
                    name: entry.name.clone(),
                    available: available.join(", "),
                });
            }
        }
        self.transport.validate()?;
        let needed = self.metrics.collectors.enabled.len() + 1;
        if usize::from(self.transport.request_channel_capacity) < needed {
            return Err(ConfigError::Invalid(format!(
                "transport.request_channel_capacity must be at least {needed} (enabled collectors + status)"
            )));
        }
        Ok(())
    }
}

/// Host name as reported by the kernel, or `unknown`.
pub fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: `buf` is a valid, writable buffer of the given length; gethostname
    // writes at most `len` bytes. The last byte is forced to NUL below so the
    // result is always terminated even if the name was truncated.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return "unknown".to_string();
    }
    buf[buf.len() - 1] = 0;
    let end = buf.iter().position(|b| *b == 0).unwrap_or(0);
    match std::str::from_utf8(&buf[..end]) {
        Ok(name) if !name.is_empty() => name.to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
        [transport]
        host = "broker"
        [metrics]
        collection_interval = 2
        [[metrics.collectors.enabled]]
        name = "cpu"
        [metrics.collectors.enabled.metadata]
        topic = "metrics/cpu"
        qos = 0
        retain = true
    "#;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("test.toml"), |name| match name {
            "HOSTNAME" => Some("host1".into()),
            "MQTT_PASSWORD" => Some("s3cret".into()),
            _ => None,
        })
    }

    #[test]
    fn minimal_config_loads_and_fills_client_id() {
        let config = parse(MINIMAL).unwrap();
        assert_eq!(config.metrics.collection_interval, 2);
        assert_eq!(config.metrics.collectors.enabled[0].name, "cpu");
        assert!(config.transport.client_id.starts_with("blazebee-"));
        assert_eq!(config.source(), Path::new("test.toml"));
    }

    #[test]
    fn environment_substitution_applies_to_all_strings() {
        let text = MINIMAL.replace(
            "host = \"broker\"",
            "host = \"broker\"\nclient_id = \"bb-${HOSTNAME}\"\nusername = \"u\"\npassword = \"${MQTT_PASSWORD}\"",
        );
        let config = parse(&text).unwrap();
        assert_eq!(config.transport.client_id, "bb-host1");
        assert_eq!(config.transport.password.unwrap().expose(), "s3cret");
    }

    #[test]
    fn unset_variable_is_an_error_but_comments_are_ignored() {
        let text = MINIMAL.replace("host = \"broker\"", "host = \"${NOPE}\"");
        assert!(matches!(parse(&text), Err(ConfigError::Env { .. })));
        let text = format!("# ${{NOPE}} in a comment is fine\n{MINIMAL}");
        parse(&text).unwrap();
    }

    #[test]
    fn unknown_keys_and_misplaced_interval_are_rejected() {
        let text = MINIMAL.replace("collection_interval = 2", "collection_interva = 2");
        let err = parse(&text).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
        assert!(err.to_string().contains("collection_interva"), "{err}");
    }

    #[test]
    fn unknown_collector_lists_available_ones() {
        let text = MINIMAL.replace("name = \"cpu\"", "name = \"nope\"");
        let err = parse(&text).unwrap_err();
        assert!(matches!(err, ConfigError::UnknownCollector { .. }), "{err}");
        assert!(err.to_string().contains("cpu"), "{err}");
    }

    #[test]
    fn zero_interval_and_no_collectors_are_rejected() {
        let text = MINIMAL.replace("collection_interval = 2", "collection_interval = 0");
        assert!(matches!(parse(&text), Err(ConfigError::Invalid(_))));
        let text = "[transport]\nhost = \"b\"\n";
        assert!(matches!(parse(text), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn duplicate_collectors_and_bad_topics_are_rejected() {
        let dup = format!("{MINIMAL}\n[[metrics.collectors.enabled]]\nname = \"cpu\"\n[metrics.collectors.enabled.metadata]\ntopic = \"x\"\n");
        assert!(matches!(parse(&dup), Err(ConfigError::Invalid(_))));
        let text = MINIMAL.replace("topic = \"metrics/cpu\"", "topic = \"metrics/#\"");
        assert!(matches!(parse(&text), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn channel_capacity_must_cover_collectors() {
        let text = MINIMAL.replace(
            "host = \"broker\"",
            "host = \"broker\"\nrequest_channel_capacity = 1",
        );
        let err = parse(&text).unwrap_err();
        assert!(
            err.to_string().contains("request_channel_capacity"),
            "{err}"
        );
    }

    #[test]
    fn transport_validation_errors_propagate() {
        let text = MINIMAL.replace("host = \"broker\"", "host = \"broker\"\nport = 0");
        assert!(matches!(parse(&text), Err(ConfigError::Transport(_))));
    }

    #[test]
    fn hostname_is_non_empty() {
        assert!(!hostname().is_empty());
    }
}
