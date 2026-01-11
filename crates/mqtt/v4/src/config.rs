//! Configuration structures for MQTT connections and serialization.
//!
//! This module defines all configuration types used throughout the MQTT manager.
//! All configurations support serde deserialization, making them compatible with
//! TOML, JSON, YAML, and other formats.
//!
//! # Base Topic Configuration
//!
//! The `base_topic` field is a critical configuration parameter:
//! - Provides namespace isolation for different applications/tenants
//! - Applied automatically to all topic operations
//! - Can be empty string to disable namespace isolation
//! - Should not include trailing slashes (handled automatically)
//!
//! # Configuration Sources
//!
//! Configs can be loaded from:
//! - TOML files (recommended for production)
//! - JSON files
//! - Environment variables
//! - Programmatic construction with defaults
//!
//! # Validation
//!
//! All configs use the `validator` crate to enforce constraints:
//! - Invalid configs fail at load time, not at connect time
//! - Error messages are specific about which field and constraint failed
//! - Constraints are documented in field attributes
//!
//! # Examples
//!
//! ```ignore
//! // Load from TOML
//! let toml_str = std::fs::read_to_string("mqtt.toml")?;
//! let config: Config = toml::from_str(&toml_str)?;
//!
//! // Or construct programmatically
//! let config = Config {
//!     base_topic: "iot-gateway".into(),
//!     host: "mqtt.example.com".into(),
//!     port: 8883,
//!     tls: Some(TlsConfig::with_ca_only("/etc/mqtt/ca.crt")),
//!     ..Default::default()
//! };
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Main MQTT connection configuration.
///
/// All fields have validation constraints specified as attributes.
/// Validation is performed whenever this struct is deserialized.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct Config {
    /// Base topic prefix for application-level messages.
    ///
    /// Used as a namespace for topics published by this application.
    /// Automatically applied to all subscribe/publish/unsubscribe operations.
    ///
    /// # Examples
    /// - `base_topic = ""` - No prefix (default for compatibility)
    /// - `base_topic = "iot-gateway"` - Topics: `iot-gateway/sensor/temp`
    /// - `base_topic = "tenant/device"` - Multi-level: `tenant/device/status`
    ///
    /// # Validation
    /// - Length: 0-255 characters (0 = disabled)
    /// - Should not include wildcards (+ or #)
    /// - Should not start or end with slash (handled automatically)
    ///
    /// # Examples
    /// ```toml
    /// base_topic = "iot-gateway"
    /// ```
    #[validate(length(max = 255, message = "Base topic must not exceed 255 characters"))]
    pub base_topic: String,

    /// Broker hostname or IP address.
    ///
    /// Can be:
    /// - Hostname: "mqtt.example.com"
    /// - IP address: "192.168.1.1"
    /// - Localhost: "localhost" or "127.0.0.1"
    ///
    /// # Validation
    /// - Length: 1-255 characters
    /// - Actual DNS resolution happens at connection time, not validation time
    ///
    /// # Examples
    /// ```toml
    /// host = "mqtt.home.local"  # Local network
    /// host = "broker.hivemq.com"  # Public broker
    /// ```
    #[validate(length(
        min = 1,
        max = 255,
        message = "Host must be between 1 and 255 characters"
    ))]
    pub host: String,

    /// Broker port number.
    ///
    /// Standard ports:
    /// - 1883: Unencrypted MQTT
    /// - 8883: MQTT over TLS
    /// - 8884: MQTT with client auth
    /// - 9001: MQTT over WebSocket
    ///
    /// # Validation
    /// - Range: 1-65535
    ///
    /// # Examples
    /// ```toml
    /// port = 1883    # Standard unencrypted
    /// port = 8883    # Standard with TLS
    /// ```
    #[validate(range(min = 1, max = 65535, message = "Port must be between 1 and 65535"))]
    pub port: u16,

    /// Connection timeout in seconds.
    ///
    /// How long to wait for the TCP connection and MQTT CONNACK before giving up.
    /// If exceeded, treated as a connection error and triggers backoff/retry.
    ///
    /// # Validation
    /// - Range: 1-300 seconds
    ///
    /// # Typical Values
    /// - 10: For local networks
    /// - 30: Default, covers most cases
    /// - 60: For slow/unstable networks
    ///
    /// # Examples
    /// ```toml
    /// connection_timeout = 30
    /// ```
    #[validate(range(
        min = 1,
        max = 300,
        message = "Connection timeout must be between 1 and 300 seconds"
    ))]
    pub connection_timeout: u64,

    /// Whether to request a clean session from the broker.
    ///
    /// If true: Broker discards previous subscriptions and pending messages.
    /// If false: Broker retains subscriptions and queues messages (up to timeout).
    ///
    /// # Trade-off
    /// - true: Faster reconnection, less broker memory usage
    /// - false: Better message guarantees, higher broker load
    ///
    /// # Examples
    /// ```toml
    /// clean_session = false  # Retain subscriptions across disconnections
    /// ```
    pub clean_session: bool,

    /// Maximum number of QoS 1 and QoS 2 messages in flight simultaneously.
    ///
    /// Limits concurrency of acknowledged publishes. Higher values allow more
    /// throughput but consume broker resources.
    ///
    /// # Validation
    /// - Range: 1-1000
    ///
    /// # Typical Values
    /// - 10: Conservative, for resource-constrained scenarios
    /// - 100: Standard for most applications
    /// - 1000: High-throughput scenarios
    ///
    /// # Examples
    /// ```toml
    /// max_inflight = 100
    /// ```
    #[validate(range(
        min = 1,
        max = 1000,
        message = "Max inflight must be between 1 and 1000"
    ))]
    pub max_inflight: u16,

    /// Keep-alive interval in seconds.
    ///
    /// Client sends PING packets to broker at this interval if no other activity
    /// occurs. Broker will disconnect if no packets received for this duration.
    ///
    /// # Validation
    /// - Range: 5-3600 seconds
    ///
    /// # Typical Values
    /// - 30: Cellular networks (frequent pings)
    /// - 60: Standard (good balance)
    /// - 120: Stable networks (less overhead)
    ///
    /// # Examples
    /// ```toml
    /// keep_alive = 60
    /// ```
    #[validate(range(
        min = 5,
        max = 3600,
        message = "Keep alive must be between 5 and 3600 seconds"
    ))]
    pub keep_alive: u64,

    /// Unique identifier for this client.
    ///
    /// If empty, a UUID is generated at connection time.
    /// Client IDs are used by brokers to:
    /// - Enforce "client ID collision" policies (reject duplicate IDs)
    /// - Store persistent subscriptions (if clean_session = false)
    /// - Log and audit
    ///
    /// # Validation
    /// - Length: 1-36 characters
    /// - Empty string is allowed and will be replaced with UUID
    ///
    /// # Examples
    /// ```toml
    /// client_id = "my-app-001"
    /// client_id = ""  # Will generate UUID
    /// ```
    #[validate(length(
        min = 1,
        max = 36,
        message = "Client ID must be between 1 and 36 characters"
    ))]
    pub client_id: String,

    /// Maximum MQTT packet size (bytes).
    ///
    /// Applied to both incoming and outgoing messages. Provides early
    /// validation before network transmission. Brokers often have their
    /// own limits, so setting a smaller value here prevents wasted network
    /// traffic on too-large publishes.
    ///
    /// # Validation
    /// - Range: 64-65535 bytes
    /// - Optional (defaults to 65535)
    ///
    /// # Typical Values
    /// - 1024-4096: IoT devices with memory constraints
    /// - 65535: Default, covers most use cases
    /// - 262144: Large payloads (note: requires careful broker configuration)
    ///
    /// # Examples
    /// ```toml
    /// max_packet_size = 2048
    /// ```
    #[validate(range(
        min = 64,
        max = 65535,
        message = "Max packet size must be between 64 and 65535 bytes"
    ))]
    pub max_packet_size: Option<u16>,

    /// Capacity of the request/response channel.
    ///
    /// How many publishes can be queued internally before blocking.
    /// Higher values allow more buffering during transient network issues.
    ///
    /// # Validation
    /// - Range: 1-255
    /// - Optional (defaults to 10)
    ///
    /// # Typical Values
    /// - 10: Default, sufficient for most use cases
    /// - 50-100: For bursty traffic patterns
    /// - 255: Maximum buffering
    ///
    /// # Examples
    /// ```toml
    /// request_channel_capacity = 50
    /// ```
    #[validate(range(
        min = 1,
        max = 255,
        message = "Request channel capacity must be between 1 and 255"
    ))]
    pub request_channel_capacity: Option<u8>,

    /// Initial delay before first reconnection attempt (seconds).
    ///
    /// Used by exponential backoff algorithm. Prevents thundering herd
    /// when broker recovers from outage.
    ///
    /// # Validation
    /// - Range: 1-60 seconds
    ///
    /// # Typical Values
    /// - 1: Quick recovery (suitable for stable networks)
    /// - 5: Balanced approach
    /// - 10-30: Brokers with slow recovery
    ///
    /// # Examples
    /// ```toml
    /// reconnect_delay = 1
    /// ```
    #[validate(range(
        min = 1,
        max = 60,
        message = "Reconnect delay must be between 1 and 60 seconds"
    ))]
    pub reconnect_delay: u64,

    /// Maximum number of reconnection attempts.
    ///
    /// After this many failed attempts, the client gives up and signals
    /// an error. Set to 0 for infinite retries.
    ///
    /// # Validation
    /// - Range: 0-100 (0 = unlimited)
    ///
    /// # Typical Values
    /// - 0: Retry forever (for critical services)
    /// - 5: Reasonable limit (gives up after ~2 minutes with default backoff)
    /// - 10: Generous limit
    ///
    /// # Examples
    /// ```toml
    /// max_reconnect_attempts = 5
    /// ```
    #[validate(range(
        min = 0,
        max = 100,
        message = "Max reconnect attempts must be between 0 and 100"
    ))]
    pub max_reconnect_attempts: u64,

    /// Exponential backoff multiplier.
    ///
    /// Each retry delay is multiplied by this value (capped at a maximum).
    /// Higher values cause faster exponential growth but may retry too slowly.
    ///
    /// # Validation
    /// - Range: 1-30
    ///
    /// # Typical Values
    /// - 1.1: Gentle growth (1s -> 1.1s -> 1.21s -> ...)
    /// - 1.5: Moderate growth
    /// - 2.0: Aggressive growth (1s -> 2s -> 4s -> ...)
    ///
    /// # Examples
    /// ```toml
    /// reconnect_backoff_delimiter = 1.1  # 10% increase per attempt
    /// ```
    #[validate(range(
        min = 1,
        max = 30,
        message = "Reconnect backoff delimiter must be between 1 and 30"
    ))]
    pub reconnect_backoff_delimiter: u64,

    /// Optional TLS configuration.
    ///
    /// If present and enabled, broker connection will use TLS encryption.
    /// Requires valid certificate files on disk.
    ///
    /// # Examples
    /// ```toml
    /// [tls]
    /// ca_cert_path = "/etc/mqtt/ca.pem"
    /// client_cert_path = "/etc/mqtt/client.crt"
    /// client_key_path = "/etc/mqtt/client.key"
    /// ```
    #[validate(nested)]
    pub tls: Option<TlsConfig>,

    /// Optional serialization configuration for MQTT messages.
    /// See `SerializationConfig` for details.
    #[validate(nested)]
    pub serialization: Option<SerializationConfig>,
    /// Framing configuration.
    /// If enabled, Publisher::publish_framed will actually split payloads into frames.
    /// If disabled, publish_framed falls back to обычный publish.
    #[validate(nested)]
    pub framing: FramingConfig,
}

impl Default for Config {
    /// Creates a configuration with safe defaults for development/testing.
    ///
    /// **Warning**: These defaults are not suitable for production without review.
    /// - Host: localhost (only works on local machine)
    /// - No authentication configured
    /// - TLS enabled but with self-signed cert paths (will fail to connect)
    ///
    /// For production, always load config from file and validate externally.
    fn default() -> Self {
        Config {
            base_topic: "blazebee_mqtt_v4".to_string(), // Default base_topic for namespace isolation
            host: "localhost".to_string(),
            port: 1883,
            connection_timeout: 30,
            clean_session: false,
            max_inflight: 10,
            keep_alive: 60,
            client_id: Uuid::new_v4().to_string(),
            max_packet_size: Some(65_535),
            request_channel_capacity: Some(1),
            reconnect_delay: 5,
            max_reconnect_attempts: 5,
            reconnect_backoff_delimiter: 2,
            tls: Some(TlsConfig::default()),
            serialization: Some(SerializationConfig::default().into_config()),
            framing: FramingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct FramingConfig {
    /// Enables framing path in Publisher::publish_framed.
    pub enabled: bool,
}

impl Default for FramingConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

/// TLS/SSL configuration for secure broker connections.
///
/// Specifies paths to certificate files. Files are validated when
/// `ClientBuilder::build()` is called, not when this struct is created.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct TlsConfig {
    /// Path to CA (Certificate Authority) certificate file.
    ///
    /// Used to verify the broker's certificate. Must be in PEM format.
    /// Required for any TLS connection (with or without client auth).
    ///
    /// # Validation
    /// - File must exist and be readable
    /// - File must contain valid PEM-encoded certificate
    ///
    /// # Examples
    /// ```toml
    /// ca_cert_path = "/etc/ssl/certs/ca-bundle.crt"
    /// ```
    #[serde(default)]
    #[validate(custom(
        function = "validate_optional_file_exists",
        message = "CA certificate file does not exist"
    ))]
    pub ca_cert_path: Option<String>,

    /// Path to client certificate file.
    ///
    /// Used for mutual TLS (client authentication). Must be in PEM format.
    /// Must be paired with `client_key_path`. If either is provided, both
    /// must be provided.
    ///
    /// # Validation
    /// - File must exist and be readable
    /// - If provided, client_key_path must also be provided
    ///
    /// # Examples
    /// ```toml
    /// client_cert_path = "/etc/mqtt/client.crt"
    /// ```
    #[validate(custom(
        function = "validate_optional_file_exists",
        message = "Client certificate file does not exist"
    ))]
    pub client_cert_path: Option<String>,

    /// Path to client private key file.
    ///
    /// Used for mutual TLS (client authentication). Must be in PEM format
    /// and unencrypted (no passphrase). Must be paired with `client_cert_path`.
    ///
    /// # Security Warning
    /// - Keep this file private (chmod 600)
    /// - Never commit to version control
    /// - Never transmit unencrypted
    /// - Consider using environment variables or secure vaults (HashiCorp Vault, etc.)
    ///
    /// # Validation
    /// - File must exist and be readable
    /// - If provided, client_cert_path must also be provided
    ///
    /// # Examples
    /// ```toml
    /// client_key_path = "/etc/mqtt/client.key"
    /// ```
    #[validate(custom(
        function = "validate_optional_file_exists",
        message = "Client key file does not exist"
    ))]
    pub client_key_path: Option<String>,
}

impl Default for TlsConfig {
    /// Creates an empty TLS configuration (TLS disabled).
    fn default() -> Self {
        TlsConfig {
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
        }
    }
}

impl TlsConfig {
    /// Creates a TLS configuration with full mutual authentication.
    ///
    /// # Arguments
    /// - `ca_cert_path`: Path to CA certificate
    /// - `client_cert_path`: Path to client certificate
    /// - `client_key_path`: Path to client private key
    ///
    /// # Examples
    /// ```ignore
    /// let tls = TlsConfig::new(
    ///     "/etc/mqtt/ca.pem",
    ///     "/etc/mqtt/client.crt",
    ///     "/etc/mqtt/client.key",
    /// );
    /// ```
    pub fn new(
        ca_cert_path: impl Into<String>,
        client_cert_path: impl Into<String>,
        client_key_path: impl Into<String>,
    ) -> Self {
        TlsConfig {
            ca_cert_path: Some(ca_cert_path.into()),
            client_cert_path: Some(client_cert_path.into()),
            client_key_path: Some(client_key_path.into()),
        }
    }

    /// Creates a TLS configuration with CA-only verification (no client auth).
    ///
    /// # Arguments
    /// - `ca_cert_path`: Path to CA certificate
    ///
    /// # Examples
    /// ```ignore
    /// let tls = TlsConfig::with_ca_only("/etc/ssl/certs/ca-bundle.crt");
    /// ```
    pub fn with_ca_only(ca_cert_path: impl Into<String>) -> Self {
        TlsConfig {
            ca_cert_path: Some(ca_cert_path.into()),
            client_cert_path: None,
            client_key_path: None,
        }
    }

    /// Checks if client authentication (mutual TLS) is configured.
    ///
    /// Returns true only if BOTH client certificate and key are specified.
    /// Returns false if either is missing (even if both should be present—
    /// this helps detect configuration errors early).
    pub fn has_client_auth(&self) -> bool {
        self.client_cert_path.is_some() && self.client_key_path.is_some()
    }

    /// Checks if TLS is enabled (CA certificate is configured).
    pub fn is_enabled(&self) -> bool {
        self.ca_cert_path.is_some()
    }

    /// Validates the TLS configuration.
    ///
    /// Checks:
    /// 1. CA certificate path is present
    /// 2. CA certificate file exists and is readable
    /// 3. If client auth: both cert and key exist and are readable
    /// 4. If partial client auth (only cert or only key): error
    ///
    /// # Returns
    /// - `Ok(())`: Configuration is valid
    /// - `Err(ValidationError)`: Specific validation error with message
    ///
    /// # Examples
    /// ```ignore
    /// let tls = TlsConfig::new(...);
    /// tls.validate_config()?;  // Fails if files missing
    /// ```
    pub fn validate_config(&self) -> Result<(), ValidationError> {
        // CA certificate is mandatory if TLS is to be used
        if self.ca_cert_path.is_none() {
            return Err(ValidationError::new("missing_ca_cert")
                .with_message("CA certificate path is required".into()));
        }

        // Validate CA certificate file exists
        validate_file_path(self.ca_cert_path.as_ref().unwrap())?;

        // If client auth is partially configured, it's an error
        if self.has_client_auth() {
            // Both present—validate both
            validate_file_path(self.client_cert_path.as_ref().unwrap())?;
            validate_file_path(self.client_key_path.as_ref().unwrap())?;
        } else if self.client_cert_path.is_some() || self.client_key_path.is_some() {
            // One present, one missing—configuration error
            return Err(ValidationError::new("incomplete_client_auth").with_message(
                "Both client certificate and key must be provided or neither".into(),
            ));
        }

        Ok(())
    }
}

/// Validates that an optional file path exists and is readable.
///
/// Used as a custom validator for serde fields.
fn validate_optional_file_exists(path: &str) -> Result<(), ValidationError> {
    validate_file_path(path)
}

/// Validates that a file exists, is readable, and is actually a file (not directory).
///
/// # Checks
/// 1. Path is not empty
/// 2. File exists
/// 3. Path points to a file (not directory)
/// 4. File is readable
fn validate_file_path(path: &str) -> Result<(), ValidationError> {
    if path.is_empty() {
        return Err(
            ValidationError::new("empty_path").with_message("File path cannot be empty".into())
        );
    }

    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(ValidationError::new("file_not_found")
            .with_message(format!("File does not exist: {path}").into()));
    }

    if !path_obj.is_file() {
        return Err(ValidationError::new("not_a_file")
            .with_message(format!("Path is not a file: {path}").into()));
    }

    if let Err(_) = std::fs::metadata(path) {
        return Err(ValidationError::new("file_not_readable")
            .with_message(format!("File is not readable: {path}").into()));
    }

    Ok(())
}

/// Metadata for a single MQTT endpoint (publish/subscribe topic).
///
/// Specifies topic, QoS, and retain settings for a particular message channel.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct EndpointMetadata {
    /// MQTT Quality of Service level.
    ///
    /// - 0 (AtMostOnce): Message sent, no confirmation. May be lost.
    /// - 1 (AtLeastOnce): Message acknowledged by broker. May arrive multiple times.
    /// - 2 (ExactlyOnce): Full 4-way handshake. Exactly one delivery (slowest).
    ///
    /// # Validation
    /// - Must be 0, 1, or 2
    ///
    /// # Trade-off
    /// - QoS 0: Fastest, least reliable
    /// - QoS 1: Balanced (most common)
    /// - QoS 2: Reliable, slowest
    ///
    /// # Examples
    /// ```toml
    /// qos = 1  # Guaranteed delivery
    /// ```
    #[serde(default = "default_qos")]
    #[validate(range(min = 0, max = 2, message = "Invalid QoS value, must be 0, 1, or 2"))]
    pub qos: u8,

    /// The MQTT topic string.
    ///
    /// Topics are hierarchical using `/` as separator. Wildcards in subscriptions:
    /// - `+`: Single level wildcard (e.g., "home/+/temperature")
    /// - `#`: Multi-level wildcard (e.g., "home/#")
    ///
    /// Publish topics must be concrete (no wildcards).
    ///
    /// # Validation
    /// - Must not be empty
    /// - Max length: 65535 bytes (MQTT spec)
    ///
    /// # Examples
    /// ```toml
    /// topic = "sensors/living_room/temperature"
    /// topic = "home/+/status"  # For subscriptions
    /// topic = "home/#"  # For subscriptions
    /// ```
    #[validate(length(min = 1, message = "Topic must not be empty"))]
    pub topic: String,

    /// Whether broker should retain this message.
    ///
    /// If true, broker stores the message and sends it to new subscribers.
    /// Useful for state topics (e.g., "device/power/status").
    /// Wasteful for high-frequency topics (e.g., "sensors/temperature").
    ///
    /// # Examples
    /// ```toml
    /// retain = true  # For status/config topics
    /// retain = false  # For event topics
    /// ```
    pub retain: bool,
}

impl Default for EndpointMetadata {
    /// Creates metadata with sensible defaults.
    ///
    /// - QoS: AtLeastOnce (1)
    /// - Topic: Random UUID (will fail at validation)
    /// - Retain: false
    fn default() -> Self {
        Self {
            qos: default_qos(),
            topic: Uuid::new_v4().to_string(),
            retain: false,
        }
    }
}

/// Returns the default QoS value (AtLeastOnce = 1).
fn default_qos() -> u8 {
    rumqttc::QoS::AtLeastOnce as u8
}

/// Supported message serialization formats.
///
/// Different formats offer different trade-offs in size, speed, and human-readability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SerializationFormat {
    /// JSON (human-readable, larger payload).
    ///
    /// Pros:
    /// - Human-readable (good for debugging)
    /// - Tools support excellent (editors, validators)
    /// - Schema flexibility
    ///
    /// Cons:
    /// - 30-50% larger than binary formats
    /// - Slower parsing
    /// - No type information (relies on schema)
    ///
    /// Use when: Debuggability is important, bandwidth not constrained
    Json,

    /// MessagePack (binary, compact, fast).
    ///
    /// Pros:
    /// - 30-50% smaller than JSON
    /// - Fast serialization/deserialization
    /// - Schema evolution support
    ///
    /// Cons:
    /// - Binary (harder to debug)
    /// - Tools support less than JSON
    /// - Manual version management needed
    ///
    /// Use when: Bandwidth or speed matters, but not debugging
    #[serde(rename = "msgpack")]
    MessagePack,

    /// CBOR (binary, self-describing, compact).
    ///
    /// Pros:
    /// - Self-describing (type information in payload)
    /// - Compact (similar to MessagePack)
    /// - Good for interop with other systems
    ///
    /// Cons:
    /// - Slower than MessagePack
    /// - Less library support
    /// - Complex spec
    ///
    /// Use when: Interoperability with CBOR systems is needed
    Cbor,
}

impl Default for SerializationFormat {
    fn default() -> Self {
        Self::Json
    }
}

impl std::fmt::Display for SerializationFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "JSON"),
            Self::MessagePack => write!(f, "MessagePack"),
            Self::Cbor => write!(f, "CBOR"),
        }
    }
}

/// Compression algorithm for message payloads.
///
/// Applied after serialization to reduce message size on the wire.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    /// No compression (raw data).
    ///
    /// Lowest CPU overhead, maximum network usage.
    /// Use for small messages or when CPU is constrained.
    None,

    /// Gzip compression (streaming).
    ///
    /// Widely supported, good compression ratio (60-80% reduction).
    /// Higher CPU cost than Zstd.
    /// Use for compatibility or when compression speed doesn't matter.
    Gzip,

    /// Zstd compression (state-of-the-art).
    ///
    /// Better ratio and speed than Gzip, widely adopted.
    /// Recommended for most use cases.
    /// Use unless you need Gzip for compatibility.
    Zstd,
}

impl Default for CompressionType {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Display for CompressionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Gzip => write!(f, "Gzip"),
            Self::Zstd => write!(f, "Zstd"),
        }
    }
}

/// Serialization and compression configuration.
///
/// Controls how messages are encoded before transmission.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct SerializationConfig {
    /// Optional preset name for serialization settings.
    /// possible presets could define combinations of format and compression.
    /// If set, overrides individual format and compression settings.
    /// Examples: minimal, compact, efficient, high_performance
    #[serde(default)]
    pub preset: Option<String>,
    /// Which format to use for serialization.
    ///
    /// Defaults to JSON for readability. Change to MessagePack or CBOR
    /// for smaller payloads.
    #[serde(default)]
    pub format: SerializationFormat,

    /// Whether to compress data after serialization.
    ///
    /// Compression is only applied if the payload exceeds `compression_threshold`.
    #[serde(default)]
    pub compression: CompressionType,

    /// Minimum payload size (bytes) before compression is attempted.
    ///
    /// Compression overhead (~30 bytes) is wasted on small messages.
    /// This threshold prevents compression of tiny payloads.
    ///
    /// # Validation
    /// - Range: 1B-1MB
    ///
    /// # Typical Values
    /// - 100: Compress everything > 100 bytes
    /// - 1024 (default): Compress only > 1KB
    /// - 4096: Only compress large messages
    #[validate(range(
        min = 1,
        max = 1048576,
        message = "Compression threshold must be between 1B and 1MB"
    ))]
    pub compression_threshold: u32,
}

impl SerializationConfig {
    /// Check if compression should be applied for given payload size
    pub fn should_compress(&self, payload_size: usize) -> bool {
        self.compression != CompressionType::None
            && payload_size >= self.compression_threshold as usize
    }
    /// Converts the current config into a SerializationConfig,
    /// applying any preset if specified.
    pub fn into_config(self) -> SerializationConfig {
        if let Some(preset) = self.preset.clone() {
            match preset.as_str() {
                "minimal" => return SerializationConfig::default(), // JSON + no compression
                "compact" => {
                    return SerializationConfig {
                        format: SerializationFormat::MessagePack,
                        compression: CompressionType::None,
                        compression_threshold: 1024,
                        preset: Some(preset),
                    };
                }
                "efficient" => {
                    return SerializationConfig {
                        format: SerializationFormat::Json,
                        compression: CompressionType::Gzip,
                        compression_threshold: 1024,
                        preset: Some(preset),
                    };
                }
                "high_performance" => {
                    return SerializationConfig {
                        format: SerializationFormat::MessagePack,
                        compression: CompressionType::Zstd,
                        compression_threshold: 512,
                        preset: Some(preset),
                    };
                }
                "ultra_compact" => {
                    return SerializationConfig {
                        format: SerializationFormat::Cbor,
                        compression: CompressionType::Zstd,
                        compression_threshold: 256,
                        preset: Some(preset),
                    };
                }
                _ => {
                    warn!(
                        "Unknown serialization preset: '{}', use manual settings",
                        preset
                    );
                }
            }
        }

        SerializationConfig {
            format: self.format,
            compression: self.compression,
            compression_threshold: self.compression_threshold,
            preset: self.preset,
        }
    }
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            format: SerializationFormat::Json,
            compression: CompressionType::Gzip,
            compression_threshold: 1024,
            preset: Some("efficient".to_string()),
        }
    }
}
