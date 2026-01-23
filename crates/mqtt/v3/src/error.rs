//! Comprehensive error handling for MQTT transfer operations.
//!
//! This module defines `TransferError`, the unified error type for all operations
//! in the MQTT manager. It aggregates errors from multiple sources (network, serialization,
//! configuration) into a single type that application code can pattern-match on.
//!
//! # Error Categories
//!
//! The error variants fall into logical categories:
//!
//! **Configuration Errors** (can be caught at startup):
//! - `InvalidMetadata`: Malformed endpoint configuration
//! - `ClientSetup`: Issues initializing the client
//! - `ConfigError`: Validation failures in settings
//!
//! **Serialization Errors** (data format issues):
//! - `Serialization`: Failed to encode data (usually schema/size issues)
//! - `Deserialization`: Failed to decode received data (corrupted or mismatched format)
//!
//! **Runtime Errors** (transient or fatal connectivity issues):
//! - `ClientTransfer`: Failed to send a publish/subscribe to broker
//! - `ClientConnection`: Network-level connection error
//! - `ConnectionConnectionState`: MQTT state machine violation
//! - `RetriesPolicy`: Backoff exhausted, gave up retrying
//! - `Io`: File I/O errors (e.g., TLS cert loading)
//!
//! # Usage
//!
//! Most functions return `Result<T, TransferError>`. Application code should handle
//! errors based on recoverability:
//!
//! ```ignore
//! match publisher.publish(data, &metadata).await {
//!     Ok(()) => println!("Sent successfully"),
//!     Err(TransferError::RetriesPolicy(_)) => {
//!         eprintln!("Connection exhausted retries, shutting down");
//!         return;
//!     }
//!     Err(TransferError::Serialization(msg)) => {
//!         eprintln!("Invalid data format: {}", msg);
//!         // Don't retry—user error
//!     }
//!     Err(e) => {
//!         eprintln!("Transient error: {}, will retry", e);
//!         // Likely recoverable; connection manager will retry automatically
//!     }
//! }
//! ```
//!
//! # Display vs Debug
//!
//! Error messages are optimized for end-user readability via `Display`.
//! For debugging, use `Debug` format which includes more context.

use thiserror::Error;

/// The unified error type for MQTT transfer operations.
///
/// This enum covers all failure modes in the MQTT client: configuration issues,
/// network failures, serialization problems, and policy exhaustion. Each variant
/// includes context about what went wrong and (usually) suggestions for remediation.
#[derive(Debug, Error)]
pub enum TransferError {
    /// Endpoint metadata is invalid or incomplete.
    ///
    /// This typically means:
    /// - QoS value is not 0, 1, or 2
    /// - Topic string is empty or exceeds 65535 bytes
    /// - Required fields are missing
    ///
    /// This is a programming error and should be caught at startup validation.
    ///
    /// # Example
    /// ```ignore
    /// let bad_qos = EndpointMetadata {
    ///     qos: 3,  // Invalid! Only 0-2 allowed
    ///     topic: "test".into(),
    ///     retain: false,
    /// };
    /// // Would produce: InvalidMetadata("Invalid QoS value, must be 0, 1, or 2")
    /// ```
    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),

    /// Data serialization to bytes failed.
    ///
    /// Possible causes:
    /// - Data structure is too large to serialize (exceeds max_packet_size)
    /// - Serializer encountered a type it cannot handle
    /// - Schema changed between versions (serde mismatch)
    ///
    /// Recovery: Fix the data, or increase packet size limits. Usually not retryable.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Data deserialization from bytes failed.
    ///
    /// Possible causes:
    /// - Received data is corrupted (network bit flip)
    /// - Schema mismatch (sender uses different format)
    /// - Compression/decompression issue
    ///
    /// Recovery: Log and skip message. May indicate compatibility issue.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// MQTT client initialization failed.
    ///
    /// Possible causes:
    /// - TLS certificate paths invalid or unreadable
    /// - Host/port configuration is malformed
    /// - Insufficient memory for client buffers
    ///
    /// Recovery: Check configuration and file permissions. This typically means
    /// the application won't start—catch at startup and fail fast.
    ///
    /// # Example
    /// ```ignore
    /// // Would occur if TLS cert doesn't exist:
    /// // ClientSetup("TLS configuration is not set")
    /// // or
    /// // ClientSetup("Invalid TLS configuration: File does not exist: /etc/mqtt/ca.crt")
    /// ```
    #[error("Client setup error: {0}")]
    ClientSetup(String),

    /// MQTT connection kernel initialization failed.
    ///
    /// Recovery: Check configuration and file permissions. This typically means
    /// the application won't start—catch at startup and fail fast.
    ///
    /// # Example
    /// ```ignore
    /// // Would occur if connection kernel fails to start:
    /// // ClientSetup("Connection kernel failed to start")
    /// // or
    /// // ConnectionKernel("Connection kernel not built")
    /// ```
    #[error("Connection kernel error: {0}")]
    ConnectionKernel(String),

    /// Configuration validation failed.
    ///
    /// The `Config` struct has validation rules (via the `validator` crate) that
    /// check things like:
    /// - Host string length (1-255 chars)
    /// - Port in valid range (1-65535)
    /// - Keep-alive in range (5-3600 seconds)
    ///
    /// Recovery: Fix configuration file or environment variables and restart.
    /// Usually caught during app startup.
    #[error("Configuration error: {0}")]
    ConfigError(#[from] validator::ValidationErrors),

    /// MQTT client failed to send a packet.
    ///
    /// This is a transient error indicating the local client couldn't queue
    /// the message (e.g., internal channel full, client shut down).
    ///
    /// Recovery: Likely the connection kernel is stopping. Will be retried
    /// automatically on reconnection if the message is queued locally.
    #[error("Client transfer error: {0}")]
    ClientTransfer(#[from] rumqttc::ClientError),

    /// MQTT connection to broker failed or was lost.
    ///
    /// Possible causes:
    /// - Network unreachable (router down, no internet)
    /// - Broker rejected CONNECT (invalid credentials, client-id collision)
    /// - Broker closed connection (timeout, protocol violation)
    /// - TLS handshake failed (cert mismatch, self-signed)
    ///
    /// Recovery: Automatic retry with exponential backoff via connection kernel.
    /// If max retries exhausted, app should log and possibly alert operators.
    ///
    /// Note: This wraps the error in a Box to avoid enum size blowup.
    #[error("Client connection error: {0}")]
    ClientConnection(#[from] Box<rumqttc::ConnectionError>),

    /// MQTT state machine encountered an invalid transition.
    ///
    /// This is usually a bug in the connection kernel or rumqttc library.
    /// Examples:
    /// - Trying to SUBSCRIBE before CONNACK received
    /// - Receiving unexpected packet in current state
    ///
    /// Recovery: File a bug report. Should not happen in normal operation.
    #[error("Client connection state error: {0}")]
    ConnectionConnectionState(#[from] rumqttc::StateError),

    /// Retry policy exhausted (max reconnection attempts exceeded).
    ///
    /// The exponential backoff algorithm gave up after many failed attempts.
    /// This indicates a sustained outage, not a transient glitch.
    ///
    /// Recovery: Log the error, alert operations, possibly trigger failover.
    /// The application is in an unrecoverable state—consider restarting the client.
    #[error("Retry policy error: {0}")]
    RetriesPolicy(#[from] super::backoff::BackoffError),

    /// I/O operation failed (file read/write, not network).
    ///
    /// Possible causes:
    /// - TLS certificate file not found or unreadable
    /// - Permission denied on config/cert files
    /// - Disk full (unlikely for reads, but possible for writes)
    ///
    /// Recovery: Check file paths, permissions, disk space. Usually caught
    /// at startup when loading TLS certificates.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Publish error: {0}")]
    Publish(#[from] rumqttc::mqttbytes::Error),

    /// Fame operations failed (max_package_size/ chunk max ... etc)
    /// Possible issues:
    /// Invalid max_package_size param
    /// Header len > max_package_size
    /// Frame len > max_package_size
    #[error("Framer error: {0}")]
    Framer(String),
}

/// Custom conversion from rumqttc's ConnectionError to TransferError.
///
/// We need this explicit impl to box the ConnectionError, since it can be large
/// and putting it directly in an enum variant would bloat the overall error type.
///
/// This is called automatically by the `?` operator when converting between types.
impl From<rumqttc::ConnectionError> for TransferError {
    fn from(err: rumqttc::ConnectionError) -> Self {
        TransferError::ClientConnection(Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_error_display() {
        let err = TransferError::InvalidMetadata("QoS must be 0-2".into());
        assert_eq!(err.to_string(), "Invalid metadata: QoS must be 0-2");
    }

    #[test]
    fn test_transfer_error_serialization() {
        let err = TransferError::Serialization("message too large".into());
        assert_eq!(err.to_string(), "Serialization error: message too large");
    }

    #[test]
    fn test_transfer_error_client_setup() {
        let err = TransferError::ClientSetup("TLS certificate not found".into());
        assert!(err.to_string().contains("TLS certificate not found"));
    }

    #[test]
    fn test_transfer_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let transfer_err: TransferError = io_err.into();
        assert!(transfer_err.to_string().contains("file not found"));
    }

    #[test]
    fn test_transfer_error_debug() {
        let err = TransferError::InvalidMetadata("test".into());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidMetadata"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_transfer_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(TransferError::Serialization("test".into()));
        assert_eq!(err.to_string(), "Serialization error: test");
    }
}
