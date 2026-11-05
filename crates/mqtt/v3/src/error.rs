use std::{io, path::PathBuf};

use thiserror::Error;

/// A configuration value that cannot be used to build a connection.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("transport.{field}: {reason}")]
pub struct ConfigError {
    pub field: &'static str,
    pub reason: String,
}

impl ConfigError {
    pub(crate) fn new(field: &'static str, reason: impl Into<String>) -> Self {
        Self {
            field,
            reason: reason.into(),
        }
    }
}

/// Errors reported by the transport.
#[derive(Debug, Error)]
pub enum TransferError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error("failed to read {path}: {source}")]
    ReadFile { path: PathBuf, source: io::Error },

    #[error("serialization failed: {0}")]
    Serialization(String),

    #[error("compression failed: {0}")]
    Compression(#[source] io::Error),

    #[error("payload of {size} bytes exceeds max_packet_size ({max} bytes)")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("invalid QoS {0}, must be 0, 1 or 2")]
    InvalidQos(u8),

    #[error("framing: {0}")]
    Framing(String),

    /// The bounded request queue towards the event loop is full. This happens
    /// when the broker is unreachable or acknowledges slower than the caller
    /// publishes.
    #[error("transport queue is full")]
    QueueFull,

    /// The connection driver has stopped; no further publishes are possible.
    #[error("transport is closed")]
    Closed,

    /// The connection driver gave up because of an unrecoverable error.
    #[error("connection failed: {0}")]
    Connection(#[source] Box<rumqttc::ConnectionError>),
}

impl From<rumqttc::ConnectionError> for TransferError {
    fn from(err: rumqttc::ConnectionError) -> Self {
        TransferError::Connection(Box::new(err))
    }
}

impl From<rumqttc::ClientError> for TransferError {
    fn from(err: rumqttc::ClientError) -> Self {
        match err {
            rumqttc::ClientError::TryRequest(_) => TransferError::QueueFull,
            rumqttc::ClientError::Request(_) => TransferError::Closed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_display_names_field() {
        let err = ConfigError::new("port", "must not be 0");
        assert_eq!(err.to_string(), "transport.port: must not be 0");
    }

    #[test]
    fn client_errors_map_to_queue_state() {
        let publish = rumqttc::Publish::new("t", rumqttc::QoS::AtMostOnce, Vec::new());
        let req = rumqttc::Request::Publish(publish);
        assert!(matches!(
            TransferError::from(rumqttc::ClientError::TryRequest(req)),
            TransferError::QueueFull
        ));
        let publish = rumqttc::Publish::new("t", rumqttc::QoS::AtMostOnce, Vec::new());
        let req = rumqttc::Request::Publish(publish);
        assert!(matches!(
            TransferError::from(rumqttc::ClientError::Request(req)),
            TransferError::Closed
        ));
    }
}
