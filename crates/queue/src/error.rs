use thiserror::Error;

pub type Result<T> = std::result::Result<T, QueueError>;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Json serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("CRC verification failed")]
    CrcMismatch,

    #[error("Queue is closed")]
    QueueClosed,

    #[error("Offset out of range: {offset}, available: {min_available}..{max_available}")]
    OffsetOutOfRange {
        offset: u64,
        min_available: u64,
        max_available: u64,
    },

    #[error("Message too large: {size} > {max_size}")]
    MessageTooLarge { size: usize, max_size: usize },

    #[error("Segment corruption detected")]
    SegmentCorrupted,

    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Consumer group error: {0}")]
    ConsumerGroupError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Unknown error: {0}")]
    Other(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
}
