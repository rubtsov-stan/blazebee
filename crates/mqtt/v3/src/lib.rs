//! MQTT 3.1.1 transport for BlazeBee, built on [`rumqttc`].
//!
//! The crate owns the connection lifecycle and the payload encoding:
//!
//! * [`Transport::connect`] builds the client from a validated [`Config`],
//!   spawns the connection driver on the current Tokio runtime and returns a
//!   handle. The driver reconnects with capped exponential backoff for as
//!   long as the process lives, publishes `{base_topic}/status` (`online` /
//!   `offline`, retained) and registers a matching last-will message.
//! * [`Transport::state`] exposes the connection state through a
//!   [`tokio::sync::watch`] channel so callers can pause work while the broker
//!   is unreachable and stop on a fatal error.
//! * [`Publisher`] serializes a value (JSON, MessagePack or CBOR), optionally
//!   compresses it (gzip or zstd) and hands it to the client without
//!   blocking. The client queue is bounded; a full queue is reported as
//!   [`TransferError::QueueFull`] instead of stalling the caller.
//!
//! The crate never subscribes to topics and never receives application
//! messages. Everything is driven from a single task, so no locks are needed.

pub mod backoff;
pub mod config;
pub mod connection;
pub mod error;
pub mod framer;
pub mod publisher;
pub mod serializer;

pub use config::{
    CompressionType, Config, EndpointMetadata, FramingConfig, Secret, SerializationConfig,
    SerializationFormat, TlsConfig,
};
pub use connection::{ConnectionState, Transport};
pub use error::{ConfigError, TransferError};
pub use publisher::Publisher;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, TransferError>;
