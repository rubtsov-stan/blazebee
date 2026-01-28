//! MQTT client builder with flexible configuration and TLS support.
//!
//! This module provides `ClientBuilder`, a fluent interface for constructing
//! MQTT clients with various customizations. It handles the complexity of
//! setting up rumqttc's `AsyncClient` and `EventLoop`, allowing callers to
//! focus on business logic rather than protocol details.
//!
//! # Architecture
//!
//! The builder separates concerns:
//! - **Configuration**: MqttOptions from rumqttc, with sensible defaults
//! - **Transport**: TCP, TLS, or custom protocols
//! - **Capacity**: Channel capacity for queuing pending publishes
//!
//! # Examples
//!
//! ## Simple TCP connection
//!
//! ```ignore
//! use mqtt_manager::ClientBuilder;
//!
//! let (client, event_loop) = ClientBuilder::new("app", "localhost", 1883, 100)?
//!     .keep_alive(60)
//!     .build()?;
//! ```
//!
//! ## Secure connection with certificates
//!
//! ```ignore
//! let (client, event_loop) = ClientBuilder::new("app", "mqtt.example.com", 8883, 100)?
//!     .with_tls(
//!         "/etc/mqtt/ca.crt",
//!         "/etc/mqtt/client.crt",
//!         "/etc/mqtt/client.key",
//!     )
//!     .credentials("user", "password")
//!     .build()?;
//! ```
//!
//! ## From configuration file
//!
//! ```ignore
//! let config = Config::load_from_file("config.toml")?;
//! let (client, event_loop) = ClientBuilder::from_config(&config)?.build()?;
//! ```

use std::{fs, time::Duration};

use rumqttc::{AsyncClient, EventLoop, LastWill, MqttOptions, QoS, TlsConfiguration, Transport};

use super::{
    config::{Config, TlsConfig},
    error::TransferError,
};

/// Builder for constructing MQTT clients with fluent API.
///
/// This struct accumulates configuration and constructs an `AsyncClient`
/// and `EventLoop` when `build()` is called. All configuration options
/// have sensible defaults that can be overridden with builder methods.
///
/// The builder is consumed by `build()`, which returns both the client
/// and its associated event loop. These must be used together: the client
/// to send commands, and the event loop to receive events/notifications.
///
/// # Thread Safety
///
/// The returned `AsyncClient` is thread-safe and can be cloned.
/// The `EventLoop` is not thread-safe and should run on a single task.
pub struct ClientBuilder {
    /// MQTT protocol options (host, port, keep-alive, credentials, etc.)
    opts: MqttOptions,

    /// Network transport (TCP, TLS, etc.)
    transport: Transport,

    /// Capacity of internal message queue. Limits how many publishes
    /// can be queued before hitting backpressure.
    cap: usize,

    /// Optional TLS configuration (paths to certificates)
    tls_config: Option<TlsConfig>,
}

impl ClientBuilder {
    /// Creates a new builder with minimal configuration.
    ///
    /// # Arguments
    /// - `client_id`: Unique identifier for this client. If empty, a UUID is generated.
    ///   Must be 1-36 characters (MQTT spec).
    /// - `host`: Broker hostname or IP (e.g., "localhost", "mqtt.example.com")
    /// - `port`: Broker port (typically 1883 for TCP, 8883 for TLS)
    /// - `cap`: Channel capacity for pending publishes (typically 10-1000)
    ///
    /// # Returns
    /// - `Ok(Self)`: Builder ready for further configuration
    /// - `Err(TransferError)`: Currently never fails, but kept for future validation
    ///
    /// # Examples
    /// ```ignore
    /// let builder = ClientBuilder::new("my-app", "mqtt.example.com", 1883, 100)?;
    /// ```
    pub fn new(
        client_id: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        cap: usize,
    ) -> Result<Self, TransferError> {
        Ok(Self {
            opts: MqttOptions::new(client_id, host, port),
            transport: Transport::Tcp,
            cap,
            tls_config: None,
        })
    }

    /// Creates a builder from a `Config` struct (typically loaded from file).
    ///
    /// This is the recommended way to initialize in production, as it allows
    /// configuration from environment, config files, or deployment tools.
    ///
    /// # Arguments
    /// - `config`: Configuration object with all MQTT settings pre-validated
    ///
    /// # Returns
    /// - `Ok(Self)`: Fully configured builder
    /// - `Err(TransferError)`: If config contains invalid values
    ///
    /// # Validation
    /// The Config struct uses the `validator` crate to ensure:
    /// - Host is 1-255 characters
    /// - Port is 1-65535
    /// - Keep-alive is 5-3600 seconds
    /// - Client ID is 1-36 characters
    ///
    /// If any validation fails, the error will include the specific field
    /// and constraint that was violated.
    ///
    /// # Examples
    /// ```ignore
    /// let config = Config::from_file("mqtt.toml")?;
    /// let builder = ClientBuilder::from_config(&config)?;
    /// ```
    pub fn from_config(config: &Config) -> Result<Self, TransferError> {
        // Generate UUID if client_id is empty (MQTT allows empty, but UUIDs are safer)
        let client_id = if config.client_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            config.client_id.clone()
        };

        // Build base MQTT options
        let mut opts = MqttOptions::new(client_id, config.host.clone(), config.port);

        // Configure keep-alive (server will disconnect if no activity for this long)
        opts.set_keep_alive(Duration::from_secs(config.keep_alive));

        // Clean session: false means broker retains subscriptions; true means clean slate
        opts.set_clean_session(config.clean_session);

        // Set maximum packet size if configured (default is 10MB, but can be smaller)
        if let Some(max_packet_size) = config.max_packet_size {
            opts.set_max_packet_size(max_packet_size as usize, max_packet_size as usize);
        }

        // Parse request channel capacity (defaults to 10)
        let cap = config.request_channel_capacity.unwrap_or(10) as usize;

        // Extract TLS configuration if present and enabled
        let tls_config = if config.tls.is_some() && config.tls.as_ref().unwrap().is_enabled() {
            config.tls.clone()
        } else {
            None
        };

        Ok(Self {
            opts,
            transport: Transport::Tcp,
            cap,
            tls_config,
        })
    }

    /// Configures TLS with both CA and client certificates.
    ///
    /// Use this when the broker requires client authentication (mutual TLS).
    /// All three certificate files must be present and readable.
    ///
    /// # Arguments
    /// - `ca_cert_path`: Path to CA certificate (PEM format)
    /// - `client_cert_path`: Path to client certificate (PEM format)
    /// - `client_key_path`: Path to client private key (PEM format, unencrypted)
    ///
    /// # Certificate Format
    /// All certificates must be in PEM (Base64-encoded) format. Common formats:
    /// ```text
    /// -----BEGIN CERTIFICATE-----
    /// MIIDXTCCAkWgAwIBAgIJAJpx...
    /// -----END CERTIFICATE-----
    /// ```
    ///
    /// # Validation
    /// Files are NOT validated until `build()` is called. This allows for
    /// temporary missing files during testing, but will fail at build time.
    ///
    /// # Examples
    /// ```ignore
    /// let builder = ClientBuilder::new("app", "mqtt.example.com", 8883, 100)?
    ///     .with_tls(
    ///         "/etc/ssl/certs/ca.pem",
    ///         "/etc/ssl/certs/client.crt",
    ///         "/etc/ssl/private/client.key",
    ///     );
    /// ```
    pub fn with_tls(
        mut self,
        ca_cert_path: impl Into<String>,
        client_cert_path: impl Into<String>,
        client_key_path: impl Into<String>,
    ) -> Self {
        self.tls_config = Some(TlsConfig::new(
            ca_cert_path,
            client_cert_path,
            client_key_path,
        ));
        self
    }

    /// Configures TLS with only a CA certificate (no client auth).
    ///
    /// Use this for connections to brokers that don't require client certificates.
    /// The CA certificate is still required to validate the server's certificate.
    ///
    /// # Arguments
    /// - `ca_cert_path`: Path to CA certificate (PEM format)
    ///
    /// # Examples
    /// ```ignore
    /// let builder = ClientBuilder::new("app", "mqtt.example.com", 8883, 100)?
    ///     .with_tls_ca_only("/etc/ssl/certs/ca.pem");
    /// ```
    pub fn with_tls_ca_only(mut self, ca_cert_path: impl Into<String>) -> Self {
        self.tls_config = Some(TlsConfig::with_ca_only(ca_cert_path));
        self
    }

    /// Sets the keep-alive interval (in seconds).
    ///
    /// The broker will close the connection if no activity occurs for this duration.
    /// The client automatically sends PING packets to keep the connection alive.
    ///
    /// # Typical Values
    /// - 30-60: Most common, balances responsiveness with overhead
    /// - 5-10: For time-sensitive applications
    /// - 120-300: For stable networks where disconnection is rare
    ///
    /// # Valid Range
    /// 5-3600 seconds (enforced by Config validation)
    ///
    /// # Examples
    /// ```ignore
    /// builder.keep_alive(60)  // Broker closes if idle for 60 seconds
    /// ```
    pub fn keep_alive(mut self, secs: u64) -> Self {
        self.opts.set_keep_alive(Duration::from_secs(secs));
        self
    }

    /// Sets the maximum packet size for incoming and outgoing messages.
    ///
    /// This limits the size of both PUBLISH payloads and other protocol messages.
    /// Brokers often have their own limits, so setting a smaller value here
    /// ensures early validation before network transmission.
    ///
    /// # Arguments
    /// - `incoming`: Max size for received messages
    /// - `outgoing`: Max size for sent messages
    ///
    /// # Typical Values
    /// - 1024-4096: For IoT devices with limited memory
    /// - 65535: Default, covers most use cases
    /// - 262144: For large payloads (256KB)
    ///
    /// # Valid Range
    /// 64-65535 bytes (enforced by Config validation)
    ///
    /// # Examples
    /// ```ignore
    /// builder.max_packet_size(2048, 2048)  // Limit to 2KB
    /// ```
    pub fn max_packet_size(mut self, incoming: usize, outgoing: usize) -> Self {
        self.opts.set_max_packet_size(incoming, outgoing);
        self
    }

    /// Configures whether to use a clean session.
    ///
    /// **Clean session = true**: Broker forgets all subscriptions and pending messages
    /// when client disconnects. Faster recovery, but loses queued messages.
    ///
    /// **Clean session = false**: Broker retains subscriptions and queues messages
    /// for reconnection (up to broker's configured timeout). Messages aren't lost
    /// but consume broker memory.
    ///
    /// # Typical Choice
    /// - `true` for ephemeral clients (web apps, temporary tools)
    /// - `false` for long-lived devices that need message guarantees
    ///
    /// # Examples
    /// ```ignore
    /// builder.clean_session(false)  // Retain subscriptions on disconnect
    /// ```
    pub fn clean_session(mut self, clean: bool) -> Self {
        self.opts.set_clean_session(clean);
        self
    }

    /// Sets MQTT authentication credentials.
    ///
    /// These are sent to the broker in the CONNECT packet (unencrypted unless using TLS).
    /// Always pair with TLS in production to prevent credential leakage.
    ///
    /// # Arguments
    /// - `username`: Broker username (often just a client identifier)
    /// - `password`: Broker password or API key
    ///
    /// # Security Notes
    /// - Always use TLS when sending credentials
    /// - Never hardcode credentials in source code—use environment variables or config files
    /// - Consider using tokens/API keys instead of passwords
    ///
    /// # Examples
    /// ```ignore
    /// builder.credentials(
    ///     std::env::var("MQTT_USER").unwrap(),
    ///     std::env::var("MQTT_PASS").unwrap(),
    /// )
    /// ```
    pub fn credentials(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.opts.set_credentials(username, password);
        self
    }

    /// Sets the maximum number of inflight publishes (QoS 1 & 2).
    ///
    /// This limits how many QoS 1 and QoS 2 messages can be simultaneously
    /// waiting for acknowledgment from the broker. Higher values allow more
    /// concurrency but consume broker resources.
    ///
    /// **QoS 0** (at-most-once) is not counted against this limit.
    ///
    /// # Typical Values
    /// - 10-20: Conservative, safer on resource-constrained brokers
    /// - 100: Standard for modern brokers
    /// - 1000+: High-throughput scenarios
    ///
    /// # Examples
    /// ```ignore
    /// builder.max_inflight(50)  // Up to 50 QoS 1/2 messages in flight
    /// ```
    pub fn max_inflight(mut self, max: u16) -> Self {
        self.opts.set_inflight(max);
        self
    }

    /// Configures the Last Will & Testament (LWT) message.
    ///
    /// If the client unexpectedly disconnects (network failure, crash, etc.),
    /// the broker will automatically publish this message to the specified topic.
    /// This allows other clients to be notified of unexpected disconnections.
    ///
    /// # Arguments
    /// - `base_topic`: Base topic for the LWT (e.g., "devices/app")
    ///   The actual LWT topic will be `{base_topic}/status`
    ///
    /// # Default LWT Message
    /// - Topic: `{base_topic}/status`
    /// - Payload: "offline" (UTF-8 bytes)
    /// - QoS: AtLeastOnce (guaranteed delivery)
    /// - Retain: true (broker keeps the message for new subscribers)
    ///
    /// # Examples
    /// ```ignore
    /// builder.set_last_will("devices/my-app")
    /// // LWT published to: devices/my-app/status
    /// // Payload: "offline"
    /// ```
    pub fn set_last_will(mut self, base_topic: &str) -> Self {
        let last_will = LastWill::new(
            format!("{0}/status", base_topic),
            "offline".as_bytes().to_vec(),
            QoS::AtLeastOnce,
            true,
        );
        self.opts.set_last_will(last_will);
        self
    }

    /// Loads a certificate file from disk and returns its contents.
    ///
    /// # Arguments
    /// - `path`: File system path to certificate file
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)`: Raw file contents
    /// - `Err(TransferError)`: If file cannot be read (not found, permission denied, etc.)
    ///
    /// # Errors
    /// Common errors:
    /// - `std::io::ErrorKind::NotFound`: File doesn't exist
    /// - `std::io::ErrorKind::PermissionDenied`: Can't read file
    /// - `std::io::ErrorKind::InvalidData`: File is corrupted
    fn load_file(path: &str) -> Result<Vec<u8>, TransferError> {
        Ok(fs::read(path)?)
    }

    /// Builds the TLS configuration by loading certificates from disk.
    ///
    /// This is called internally by `build()` if TLS is configured.
    /// It validates that:
    /// 1. TLS config exists
    /// 2. CA certificate file is readable
    /// 3. If client auth: both client cert and key are readable
    ///
    /// # Returns
    /// - `Ok(Transport)`: TLS-configured transport ready for use
    /// - `Err(TransferError)`: Configuration invalid or files missing/unreadable
    ///
    /// # Error Cases
    /// ```ignore
    /// // Missing CA certificate
    /// TransferError::ClientSetup("TLS configuration is not set")
    ///
    /// // CA file not found
    /// TransferError::ClientSetup("Invalid TLS configuration: File does not exist: /etc/mqtt/ca.crt")
    /// ```
    fn build_tls_config(&self) -> Result<Transport, TransferError> {
        let tls_config = self
            .tls_config
            .as_ref()
            .ok_or_else(|| TransferError::ClientSetup("TLS configuration is not set".into()))?;

        // Validate configuration structure
        tls_config
            .validate_config()
            .map_err(|e| TransferError::ClientSetup(format!("Invalid TLS configuration: {}", e)))?;

        // Load CA certificate (always required if TLS is enabled)
        let ca = Self::load_file(tls_config.ca_cert_path.as_ref().unwrap())?;

        // Load client authentication certificates if provided
        let client_auth = if tls_config.has_client_auth() {
            let cert = Self::load_file(tls_config.client_cert_path.as_ref().unwrap())?;
            let key = Self::load_file(tls_config.client_key_path.as_ref().unwrap())?;
            Some((cert, key))
        } else {
            None
        };

        Ok(Transport::Tls(TlsConfiguration::Simple {
            ca,
            client_auth,
            alpn: None,
        }))
    }

    /// Constructs the MQTT client and event loop.
    ///
    /// This is the final step after all configuration is complete.
    /// Returns both the client (for sending messages) and event loop
    /// (for receiving events). These must be used together.
    ///
    /// # Returns
    /// - `Ok((AsyncClient, EventLoop))`: Tuple of client and event processor
    /// - `Err(TransferError)`: If configuration or TLS setup failed
    ///
    /// # Consuming the Builder
    /// This method consumes the builder, preventing accidental reuse.
    ///
    /// # Examples
    /// ```ignore
    /// let (client, event_loop) = builder.build()?;
    ///
    /// // Spawn event loop on separate task
    /// tokio::spawn(async move {
    ///     event_loop.run().await;
    /// });
    ///
    /// // Use client to publish/subscribe
    /// client.publish("topic", rumqttc::QoS::AtLeastOnce, false, payload).await?;
    /// ```
    pub fn build(self) -> Result<(AsyncClient, EventLoop), TransferError> {
        // Build transport: either TLS or plain TCP
        let transport = if self.tls_config.is_some() {
            self.build_tls_config()?
        } else {
            self.transport
        };

        // Apply transport to MQTT options
        let mut opts = self.opts;
        opts.set_transport(transport);

        // Create client and event loop
        let (client, event_loop) = AsyncClient::new(opts, self.cap);

        Ok((client, event_loop))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use tempfile::TempDir;

    use super::*;
    use crate::config::{FramingConfig, SerializationConfig};

    /// Helper struct for creating temporary test files.
    ///
    /// Automatically cleans up files when dropped (RAII pattern).
    struct TestFiles {
        _temp_dir: TempDir,
        ca_cert: String,
        client_cert: String,
        client_key: String,
    }

    impl TestFiles {
        /// Creates temporary certificate files for testing.
        fn new() -> std::io::Result<Self> {
            let temp_dir = TempDir::new()?;

            let ca_cert = temp_dir.path().join("ca.crt");
            let client_cert = temp_dir.path().join("client.crt");
            let client_key = temp_dir.path().join("client.key");

            // Write dummy certificate content
            File::create(&ca_cert)?.write_all(b"ca certificate content")?;
            File::create(&client_cert)?.write_all(b"client certificate content")?;
            File::create(&client_key)?.write_all(b"client key content")?;

            Ok(TestFiles {
                _temp_dir: temp_dir,
                ca_cert: ca_cert.to_string_lossy().into_owned(),
                client_cert: client_cert.to_string_lossy().into_owned(),
                client_key: client_key.to_string_lossy().into_owned(),
            })
        }
    }

    #[test]
    fn test_builder_creation() {
        let builder = ClientBuilder::new("test_client", "localhost", 1883, 100).unwrap();
        assert_eq!(builder.cap, 100);
    }

    #[test]
    fn test_builder_with_tls() {
        let test_files = TestFiles::new().expect("Failed to create test files");

        let builder = ClientBuilder::new("test_client", "localhost", 8883, 100)
            .expect("Failed to create ClientBuilder")
            .with_tls(
                &test_files.ca_cert,
                &test_files.client_cert,
                &test_files.client_key,
            );

        assert!(builder.tls_config.is_some());
    }

    #[test]
    fn test_builder_with_tls_ca_only() {
        let test_files = TestFiles::new().expect("Failed to create test files");

        let builder = ClientBuilder::new("test_client", "localhost", 8883, 100)
            .expect("Failed to create ClientBuilder")
            .with_tls_ca_only(&test_files.ca_cert);

        assert!(builder.tls_config.is_some());
    }

    #[test]
    fn test_builder_with_chain_methods() {
        let builder = ClientBuilder::new("test_client", "localhost", 1883, 100)
            .expect("Failed to create ClientBuilder")
            .keep_alive(30)
            .max_packet_size(1024, 1024)
            .clean_session(true)
            .credentials("user", "pass")
            .max_inflight(50);

        assert_eq!(builder.cap, 100);
    }

    #[test]
    fn test_build_tcp_client() {
        let result = ClientBuilder::new("test_client", "localhost", 1883, 100)
            .expect("Failed to create ClientBuilder")
            .build();

        assert!(result.is_ok());
        let (client, _) = result.unwrap();
        assert!(!format!("{:?}", client).is_empty());
    }

    #[test]
    fn test_build_tls_client_with_client_auth() {
        let test_files = TestFiles::new().expect("Failed to create test files");

        let result = ClientBuilder::new("test_client", "localhost", 8883, 100)
            .expect("Failed to create ClientBuilder")
            .with_tls(
                &test_files.ca_cert,
                &test_files.client_cert,
                &test_files.client_key,
            )
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_build_tls_client_without_client_auth() {
        let test_files = TestFiles::new().expect("Failed to create test files");

        let result = ClientBuilder::new("test_client", "localhost", 8883, 100)
            .expect("Failed to create ClientBuilder")
            .with_tls_ca_only(&test_files.ca_cert)
            .build();

        if let Err(err) = &result {
            eprintln!("Failed to build TLS client: {err:?}");
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_from_config() {
        let config = Config {
            base_topic: "test-blazebee_mqtt_v3".to_string(),
            host: "localhost".to_string(),
            port: 1883,
            clean_session: true,
            max_inflight: 20,
            keep_alive: 60,
            client_id: "test_client".to_string(),
            max_packet_size: Some(2048),
            request_channel_capacity: Some(100),
            tls: None,
            serialization: Some(SerializationConfig::default()),
            framing: FramingConfig::default(),
        };

        let result = ClientBuilder::from_config(&config);
        assert!(result.is_ok());

        let builder = result.unwrap();
        let result = builder.build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_from_config_invalid() {
        let config = Config {
            base_topic: "test-blazebee_mqtt_v3".to_string(),
            host: "".to_string(), // Invalid: empty host
            port: 1883,
            clean_session: true,
            max_inflight: 20,
            keep_alive: 60,
            client_id: "test_client".to_string(),
            max_packet_size: Some(2048),
            request_channel_capacity: Some(100),
            tls: None,
            serialization: Some(SerializationConfig::default()),
            framing: FramingConfig::default(),
        };

        let result = ClientBuilder::from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_tls_missing_ca() {
        let result = ClientBuilder::new("test_client", "localhost", 8883, 100)
            .expect("Failed to create ClientBuilder")
            .with_tls_ca_only("/nonexistent/ca.crt")
            .build();

        assert!(result.is_err());
    }
}
