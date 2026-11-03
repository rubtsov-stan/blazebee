//! Transport configuration. Every struct rejects unknown keys so that a typo
//! in the TOML file is reported at startup instead of being silently ignored.

use std::{fmt, path::PathBuf};

use serde::Deserialize;

use crate::error::ConfigError;

/// Largest MQTT 3.1.1 packet (the Remaining Length field is at most 4 bytes).
pub const MAX_PACKET_SIZE_LIMIT: u32 = 268_435_455;

/// Smallest keep-alive accepted. Shorter intervals are rejected by `rumqttc`.
pub const MIN_KEEP_ALIVE_SECS: u16 = 5;

/// Broker connection settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Prefix applied to every published topic, including the status topic.
    /// May be empty.
    pub base_topic: String,
    pub host: String,
    pub port: u16,
    /// Must be set before `Transport::connect`; the application derives a
    /// stable default from the host name when the file leaves it empty.
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<Secret>,
    /// `false` keeps the session (and queued QoS 1/2 messages) on the broker
    /// between connections. Requires a stable `client_id`.
    pub clean_session: bool,
    /// Keep-alive interval in seconds (5..=65535).
    pub keep_alive: u16,
    /// Maximum number of unacknowledged QoS 1/2 publishes.
    pub max_inflight: u16,
    /// Upper bound for a single MQTT packet, in bytes. Payloads that would
    /// exceed it are rejected before they reach the network.
    pub max_packet_size: u32,
    /// Capacity of the bounded queue between publishers and the event loop.
    pub request_channel_capacity: u16,
    pub tls: TlsConfig,
    pub serialization: SerializationConfig,
    pub framing: FramingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            base_topic: "blazebee_mqtt_v3".to_string(),
            host: "localhost".to_string(),
            port: 1883,
            client_id: String::new(),
            username: None,
            password: None,
            clean_session: false,
            keep_alive: 60,
            max_inflight: 10,
            max_packet_size: 1_048_576,
            request_channel_capacity: 64,
            tls: TlsConfig::default(),
            serialization: SerializationConfig::default(),
            framing: FramingConfig::default(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let host = self.host.trim();
        if host.is_empty() || host.chars().any(char::is_whitespace) {
            return Err(ConfigError::new(
                "host",
                "must be a host name or IP address",
            ));
        }
        if self.port == 0 {
            return Err(ConfigError::new("port", "must be in 1..=65535"));
        }
        if self.client_id.is_empty() {
            return Err(ConfigError::new("client_id", "must not be empty"));
        }
        if self.client_id.contains('\0') {
            return Err(ConfigError::new("client_id", "must not contain NUL"));
        }
        if self.keep_alive < MIN_KEEP_ALIVE_SECS {
            return Err(ConfigError::new(
                "keep_alive",
                format!("must be at least {MIN_KEEP_ALIVE_SECS} seconds"),
            ));
        }
        if self.max_inflight == 0 {
            return Err(ConfigError::new("max_inflight", "must be at least 1"));
        }
        if !(64..=MAX_PACKET_SIZE_LIMIT).contains(&self.max_packet_size) {
            return Err(ConfigError::new(
                "max_packet_size",
                format!("must be in 64..={MAX_PACKET_SIZE_LIMIT} bytes"),
            ));
        }
        if self.request_channel_capacity == 0 {
            return Err(ConfigError::new(
                "request_channel_capacity",
                "must be at least 1",
            ));
        }
        if self.password.is_some() && self.username.is_none() {
            return Err(ConfigError::new(
                "password",
                "requires username (MQTT 3.1.1)",
            ));
        }
        validate_topic_chars("base_topic", &self.base_topic)?;
        self.tls.validate()?;
        Ok(())
    }
}

/// Password or other credential; never printed by `Debug`.
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// TLS settings. TLS is used when `enabled` is set or when a CA certificate
/// path is given; with no CA path the system trust store is used.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TlsConfig {
    pub enabled: bool,
    /// PEM bundle used to verify the broker certificate.
    pub ca_cert_path: Option<PathBuf>,
    /// PEM client certificate for mutual TLS; requires `client_key_path`.
    pub client_cert_path: Option<PathBuf>,
    /// Unencrypted PEM private key matching `client_cert_path`.
    pub client_key_path: Option<PathBuf>,
}

impl TlsConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled || self.ca_cert_path.is_some()
    }

    pub fn has_client_auth(&self) -> bool {
        self.client_cert_path.is_some() && self.client_key_path.is_some()
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if !self.is_enabled() {
            return Ok(());
        }
        if self.client_cert_path.is_some() != self.client_key_path.is_some() {
            return Err(ConfigError::new(
                "tls",
                "client_cert_path and client_key_path must be set together",
            ));
        }
        for (field, path) in [
            ("tls.ca_cert_path", &self.ca_cert_path),
            ("tls.client_cert_path", &self.client_cert_path),
            ("tls.client_key_path", &self.client_key_path),
        ] {
            if let Some(path) = path {
                if !path.is_file() {
                    return Err(ConfigError::new(
                        field,
                        format!("{} is not a readable file", path.display()),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Wire encoding of a published value.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum SerializationFormat {
    Json = 0,
    #[serde(rename = "msgpack")]
    MessagePack = 1,
    Cbor = 2,
}

impl fmt::Display for SerializationFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Json => "json",
            Self::MessagePack => "msgpack",
            Self::Cbor => "cbor",
        })
    }
}

/// Compression applied after serialization.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    None,
    Gzip,
    Zstd,
}

impl fmt::Display for CompressionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::Gzip => "gzip",
            Self::Zstd => "zstd",
        })
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SerializationConfig {
    pub format: SerializationFormat,
    pub compression: CompressionType,
    /// Payloads shorter than this many bytes are sent uncompressed.
    pub compression_threshold: u32,
}

impl Default for SerializationConfig {
    /// Plain JSON: readable by any subscriber without out-of-band knowledge.
    fn default() -> Self {
        Self {
            format: SerializationFormat::Json,
            compression: CompressionType::None,
            compression_threshold: 1024,
        }
    }
}

/// When enabled every payload is split into frames that each fit into
/// `max_packet_size`; each frame carries a 21-byte header. Consumers must
/// reassemble frames, so leave this off unless the receiving side expects it.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FramingConfig {
    pub enabled: bool,
}

/// Where and how a value is published.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct EndpointMetadata {
    /// Topic relative to `base_topic`.
    pub topic: String,
    /// 0, 1 or 2.
    pub qos: u8,
    pub retain: bool,
}

impl EndpointMetadata {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.topic.is_empty() {
            return Err(ConfigError::new("topic", "must not be empty"));
        }
        validate_topic_chars("topic", &self.topic)?;
        if self.qos > 2 {
            return Err(ConfigError::new("qos", "must be 0, 1 or 2"));
        }
        Ok(())
    }
}

fn validate_topic_chars(field: &'static str, topic: &str) -> Result<(), ConfigError> {
    if topic.len() > 65_535 {
        return Err(ConfigError::new(field, "must be at most 65535 bytes"));
    }
    if topic.contains(['+', '#']) {
        return Err(ConfigError::new(
            field,
            "publish topics must not contain wildcards",
        ));
    }
    if topic.contains('\0') {
        return Err(ConfigError::new(field, "must not contain NUL"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Config {
        Config {
            client_id: "test".into(),
            ..Config::default()
        }
    }

    #[test]
    fn defaults_validate_once_client_id_is_set() {
        assert!(Config::default().validate().is_err());
        valid().validate().unwrap();
    }

    #[test]
    fn rejects_bad_scalars() {
        let mut c = valid();
        c.port = 0;
        assert_eq!(c.validate().unwrap_err().field, "port");
        let mut c = valid();
        c.keep_alive = 1;
        assert_eq!(c.validate().unwrap_err().field, "keep_alive");
        let mut c = valid();
        c.max_packet_size = 10;
        assert_eq!(c.validate().unwrap_err().field, "max_packet_size");
        let mut c = valid();
        c.host = "a b".into();
        assert_eq!(c.validate().unwrap_err().field, "host");
        let mut c = valid();
        c.password = Some(Secret::new("x"));
        assert_eq!(c.validate().unwrap_err().field, "password");
        let mut c = valid();
        c.base_topic = "a/#".into();
        assert_eq!(c.validate().unwrap_err().field, "base_topic");
    }

    #[test]
    fn tls_requires_matching_client_files() {
        let mut c = valid();
        c.tls.enabled = true;
        c.tls.client_cert_path = Some("/nonexistent/cert.pem".into());
        assert_eq!(c.validate().unwrap_err().field, "tls");
        c.tls.client_key_path = Some("/nonexistent/key.pem".into());
        assert_eq!(c.validate().unwrap_err().field, "tls.client_cert_path");
        let mut c = valid();
        c.tls.ca_cert_path = Some("/nonexistent/ca.pem".into());
        assert!(c.tls.is_enabled());
        assert_eq!(c.validate().unwrap_err().field, "tls.ca_cert_path");
    }

    #[test]
    fn tls_with_existing_files_validates() {
        let dir = tempfile::tempdir().unwrap();
        let ca = dir.path().join("ca.pem");
        std::fs::write(&ca, "x").unwrap();
        let mut c = valid();
        c.tls.ca_cert_path = Some(ca);
        c.validate().unwrap();
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let err = toml::from_str::<Config>("hots = \"x\"").unwrap_err();
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    #[test]
    fn parses_formats_and_secret() {
        let c: Config = toml::from_str(
            r#"
            host = "broker"
            username = "u"
            password = "p"
            [serialization]
            format = "msgpack"
            compression = "zstd"
            compression_threshold = 10
            [tls]
            enabled = true
            "#,
        )
        .unwrap();
        assert_eq!(c.serialization.format, SerializationFormat::MessagePack);
        assert_eq!(c.serialization.compression, CompressionType::Zstd);
        assert_eq!(c.password.as_ref().unwrap().expose(), "p");
        assert_eq!(format!("{:?}", c.password), "Some(Secret(***))");
        assert!(c.tls.is_enabled());
    }

    #[test]
    fn endpoint_metadata_validation() {
        let ok = EndpointMetadata {
            topic: "metrics/cpu".into(),
            qos: 1,
            retain: true,
        };
        ok.validate().unwrap();
        let mut bad = ok.clone();
        bad.qos = 3;
        assert_eq!(bad.validate().unwrap_err().field, "qos");
        let mut bad = ok.clone();
        bad.topic = "metrics/+".into();
        assert_eq!(bad.validate().unwrap_err().field, "topic");
        assert_eq!(
            EndpointMetadata::default().validate().unwrap_err().field,
            "topic"
        );
    }
}
