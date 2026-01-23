//! # mqtt-manager: Reliable MQTT client with automatic reconnection
//!
//! A production-ready, async MQTT client library for Rust with a focus on reliability,
//! observability, and ease of use. Built on top of `rumqttc`, this crate adds:
//!
//! - **Automatic reconnection** with exponential backoff
//! - **State monitoring** for UI integration and diagnostics
//! - **Multiple serialization formats** (JSON, MessagePack, CBOR)
//! - **Compression support** (Gzip, Zstd)
//! - **TLS/SSL** with client certificate authentication
//! - **Subscription persistence** across reconnections
//! - **Comprehensive error handling** with categorization
//! - **Production-grade reliability** with extensive testing
//!
//! # Quick Start
//!
//! ```ignore
//! use mqtt_manager::MqttManager;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct SensorReading {
//!     temperature: f32,
//!     humidity: f32,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize MQTT manager
//!     let manager = MqttManager::new("mqtt.example.com", 1883)?;
//!     let instance = manager.build_and_start().await?;
//!
//!     // Subscribe to topic
//!     instance.subscribe("sensors/+/data").await?;
//!
//!     // Publish a message
//!     let reading = SensorReading {
//!         temperature: 22.5,
//!         humidity: 45.0,
//!     };
//!
//!     let metadata = mqtt_manager::EndpointMetadata {
//!         qos: 1,
//!         topic: "sensors/living_room".into(),
//!         retain: true,
//!     };
//!
//!     let publisher = mqtt_manager::Publisher::new(
//!         std::sync::Arc::new(instance)
//!     );
//!     publisher.publish(&reading, &metadata).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! ## Automatic Reconnection
//!
//! The connection kernel automatically reconnects with exponential backoff:
//! - Initial delay: 1 second
//! - Growth: 10% per attempt
//! - Maximum: 60 seconds
//! - Fully configurable
//!
//! ```text
//! Attempt 1: wait 1.0s
//! Attempt 2: wait 1.1s
//! Attempt 3: wait 1.21s
//! ...
//! Attempt 60+: wait 60s (capped)
//! ```
//!
//! ## State Monitoring
//!
//! Subscribe to connection state changes for real-time UI updates:
//!
//! ```ignore
//! let mut state_rx = instance.supervisor().ev_loop_state_rx();
//!
//! tokio::spawn(async move {
//!     loop {
//!         state_rx.changed().await?;
//!         match state_rx.borrow().clone() {
//!             ConnectionState::Connected => println!("Online"),
//!             ConnectionState::Disconnected(reason) => {
//!                 println!("Offline: {}", reason)
//!             }
//!             ConnectionState::Reconnecting(secs) => {
//!                 println!("Reconnecting in {:.1}s", secs)
//!             }
//!             _ => {}
//!         }
//!     }
//! });
//! ```
//!
//! ## Serialization Formats
//!
//! Choose the best format for your use case:
//!
//! | Format | Use Case | Size | Speed |
//! |--------|----------|------|-------|
//! | JSON | Debugging, human-readable | Large | Slow |
//! | MessagePack | Bandwidth-constrained | Medium | Fast |
//! | CBOR | Interoperability | Medium | Medium |
//!
//! ```ignore
//! // Compact setup: MessagePack with Zstd compression
//! use mqtt_manager::Publisher;
//! let publisher = Publisher::high_performance(instance);
//! ```
//!
//! ## TLS/SSL Support
//!
//! Secure broker connections with multiple authentication options:
//!
//! **CA-only verification** (most common):
//! ```ignore
//! use mqtt_manager::ClientBuilder;
//! let builder = ClientBuilder::new("app", "mqtt.example.com", 8883, 100)?
//!     .with_tls_ca_only("/etc/ssl/certs/ca-bundle.crt");
//! ```
//!
//! **Mutual TLS** (client authentication):
//! ```ignore
//! let builder = ClientBuilder::new("app", "mqtt.example.com", 8883, 100)?
//!     .with_tls(
//!         "/etc/ssl/certs/ca-bundle.crt",
//!         "/etc/mqtt/client.crt",
//!         "/etc/mqtt/client.key",
//!     );
//! ```
//!
//! # Architecture
//!
//! The library follows a layered architecture:
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │     Application Layer                │
//! │ (Your code using Publisher, etc.)    │
//! └────────────┬─────────────────────────┘
//!              │
//! ┌────────────▼─────────────────────────┐
//! │     High-Level API                   │
//! │ MqttManager, Publisher, Supervisor   │
//! └────────────┬─────────────────────────┘
//!              │
//! ┌────────────▼─────────────────────────┐
//! │     Core Components                  │
//! │ ClientBuilder, ConnectionKernel      │
//! │ State, Backoff, Error handling       │
//! └────────────┬─────────────────────────┘
//!              │
//! ┌────────────▼─────────────────────────┐
//! │     rumqttc Library                  │
//! │ AsyncClient, EventLoop, MQTT Packets │
//! └────────────┬─────────────────────────┘
//!              │
//! ┌────────────▼─────────────────────────┐
//! │     Network (TCP/TLS)                │
//! │ MQTT Broker Connection               │
//! └──────────────────────────────────────┘
//! ```
//!
//! # Configuration
//!
//! Load configuration from TOML files or construct programmatically:
//!
//! ```ignore
//! // From TOML file
//! let config_text = std::fs::read_to_string("mqtt.toml")?;
//! let config: mqtt_manager::Config = toml::from_str(&config_text)?;
//! let manager = mqtt_manager::MqttManager::from_config(config)?;
//!
//! // Programmatically
//! let config = mqtt_manager::Config {
//!     host: "mqtt.example.com".into(),
//!     port: 8883,
//!     keep_alive: 60,
//!     clean_session: false,
//!     tls: Some(mqtt_manager::config::TlsConfig::with_ca_only(
//!         "/etc/mqtt/ca.crt"
//!     )),
//!     ..Default::default()
//! };
//! ```
//!
//! Example `mqtt.toml`:
//! ```toml
//! base_topic = "my-app"
//! host = "mqtt.example.com"
//! port = 8883
//! clean_session = false
//! keep_alive = 60
//! reconnect_delay = 1
//! max_reconnect_attempts = 0  # Unlimited
//!
//! [tls]
//! ca_cert_path = "/etc/mqtt/ca.pem"
//! client_cert_path = "/etc/mqtt/client.crt"
//! client_key_path = "/etc/mqtt/client.key"
//! ```
//!
//! # Error Handling
//!
//! All operations return `Result<T>` which is a `std::result::Result<T, TransferError>`.
//!
//! Errors are categorized for proper handling:
//!
//! ```ignore
//! match publisher.publish(&data, &metadata).await {
//!     Ok(()) => println!("Message sent"),
//!
//!     // Configuration errors (fix and restart)
//!     Err(mqtt_manager::TransferError::ClientSetup(msg)) => {
//!         eprintln!("Setup failed: {}", msg);
//!         std::process::exit(1);
//!     }
//!
//!     // User data errors (fix data, don't retry)
//!     Err(mqtt_manager::TransferError::Serialization(msg)) => {
//!         eprintln!("Data too large: {}", msg);
//!         // Don't retry—data is invalid
//!     }
//!
//!     // Network errors (automatic retry)
//!     Err(mqtt_manager::TransferError::ClientConnection(e)) => {
//!         eprintln!("Network error: {}", e);
//!         // Connection kernel will retry automatically
//!     }
//!
//!     // Retry policy exhausted (critical)
//!     Err(mqtt_manager::TransferError::RetriesPolicy(e)) => {
//!         eprintln!("Cannot connect: {}", e);
//!         std::process::exit(1);
//!     }
//!
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```
//!
//! # Connection Lifecycle
//!
//! The client goes through well-defined states:
//!
//! ```text
//! Connecting ──(CONNACK)──> Connected
//!                              │
//!                       (network error)
//!                              │
//!                              ▼
//!                        Disconnected
//!                              │
//!                        (apply backoff)
//!                              │
//!                              ▼
//!                       Reconnecting(secs)
//!                              │
//!                         (delay elapsed)
//!                              │
//!                              ▼
//!                          Connecting
//! ```
//!
//! Subscribe to state changes to monitor this:
//!
//! ```ignore
//! let mut state_rx = instance.supervisor().ev_loop_state_rx();
//!
//! while state_rx.changed().await.is_ok() {
//!     match state_rx.borrow().clone() {
//!         ConnectionState::Connected => {
//!             // Safe to publish now
//!         }
//!         ConnectionState::Reconnecting(secs) => {
//!             // Show countdown to user
//!             println!("Reconnecting in {:.1}s", secs);
//!         }
//!         ConnectionState::Disconnected(reason) => {
//!             // Show error to user
//!             eprintln!("Connection lost: {}", reason);
//!         }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! # Performance
//!
//! Typical performance on modern hardware:
//!
//! - **Throughput**: 1,000-2,000 messages/second
//! - **Latency**: 10-100 ms (network-dependent)
//! - **Memory**: 5-10 MB base + ~100 bytes per subscription
//! - **CPU**: < 1% idle, < 5% under load
//!
//! For high-throughput scenarios, use MessagePack + Zstd compression:
//!
//! ```ignore
//! let publisher = mqtt_manager::Publisher::high_performance(instance);
//! ```
//!
//! # Thread Safety
//!
//! Key types are designed for concurrent use:
//!
//! - `AsyncClient` is thread-safe and can be cloned
//! - `Publisher` is thread-safe (wrapped in Arc)
//! - `MqttInstance` is shareable across tasks
//! - Watch channels provide safe, lock-free updates
//!
//! ```ignore
//! let instance = Arc::new(instance);
//!
//! // Spawn multiple tasks publishing concurrently
//! for i in 0..10 {
//!     let instance = instance.clone();
//!     tokio::spawn(async move {
//!         let publisher = mqtt_manager::Publisher::new(instance);
//!         // Publish in parallel
//!     });
//! }
//! ```
//!
//! # Testing
//!
//! All public APIs are thoroughly tested. Run tests with:
//!
//! ```bash
//! cargo test --all
//! cargo test --doc  # Doc comment examples
//! ```
//!
//! For integration testing with a real broker:
//!
//! ```bash
//! # Start a broker
//! docker run -p 1883:1883 eclipse-mosquitto
//!
//! # Run integration tests
//! cargo test --test integration_tests
//! ```
//!
//! # Examples
//!
//! See `/examples` directory for complete examples:
//!
//! - `basic_publish.rs` - Simple pub/sub
//! - `with_tls.rs` - Secure connections
//! - `custom_serialization.rs` - Different formats
//! - `state_monitoring.rs` - UI integration
//! - `error_handling.rs` - Proper error recovery
//!
//! # Troubleshooting
//!
//! ## Connection refused
//! **Problem**: `Connection refused` error
//! **Solution**: Check broker is running and host/port are correct
//!
//! ## TLS certificate validation failed
//! **Problem**: `TLS handshake failed` error
//! **Solution**: Verify CA certificate path and format (must be PEM)
//!
//! ## Message too large
//! **Problem**: `Serialization error: message too large`
//! **Solution**: Reduce data size or increase `max_packet_size` in config
//!
//! ## High CPU usage
//! **Problem**: CPU spikes when publishing many messages
//! **Solution**: Use compression (`CompressionType::Zstd`) or reduce publish rate

// Module declarations
pub mod backoff;
pub mod client;
pub mod config;
pub mod connection;
pub mod error;
pub mod framer;
pub mod manager;
pub mod message;
pub mod publisher;
pub mod state;
pub mod supervisor;

// Re-exports: Configuration
//
// Most applications need Config to load settings
pub use config::{Config, EndpointMetadata};
// Re-exports: Connection management
//
// Used by applications that need advanced control over the connection
pub use connection::{ConnectionBuilder, ConnectionKernel};
// Re-exports: Error handling
//
// Every result returns TransferError
pub use error::TransferError;
// Re-exports: Framer
//
// Module for splitting bytes into byte frames
pub use framer::{FrameHeader, FrameParts, Framer};
// Re-exports: High-level types
//
// These are the most common entry points for applications
pub use manager::{MqttInstance, MqttManager};
pub use publisher::Publisher;
// Re-exports: State monitoring
//
// Applications use this to track connection status
pub use state::ConnectionState;
// Re-exports: Subscription management
//
// Advanced users interact with supervisor directly
pub use supervisor::Supervisor;

/// Result type for MQTT operations.
///
/// All fallible operations in this crate return this type.
/// It's an alias for `std::result::Result<T, TransferError>`.
///
/// # Examples
///
/// ```ignore
/// async fn publish_sensor_data() -> mqtt_manager::Result<()> {
///     let instance = manager.build_and_start().await?;
///     instance.subscribe("sensor/data").await?;
///     Ok(())
/// }
/// ```
///
/// # Error Handling
///
/// Use pattern matching to handle specific errors:
///
/// ```ignore
/// match operation().await {
///     Ok(value) => println!("Success: {:?}", value),
///     Err(TransferError::ClientConnection(e)) => {
///         eprintln!("Network error (will retry): {}", e);
///     }
///     Err(e) => {
///         eprintln!("Fatal error: {}", e);
///     }
/// }
/// ```
pub type Result<T> = std::result::Result<T, TransferError>;
