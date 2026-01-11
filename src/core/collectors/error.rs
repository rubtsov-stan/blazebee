use thiserror::Error;

/// Custom error type for the collector system.
/// Uses `thiserror` for clean, automatic derivation of `Debug`, `Display`, and `Error`
/// traits, with context-rich error messages.
#[derive(Error, Debug)]
pub enum CollectorError {
    /// Failed to read a file from disk.
    /// Includes the file path and the underlying I/O error for debugging.
    #[error("Failed to read file {path}")]
    FileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Encountered a parsing error while extracting a metric.
    /// Provides the metric name, where it was found, and a reason for the failure.
    #[error("Failed to parse {metric} from {location}: {reason}")]
    ParseError {
        metric: String,
        location: String,
        reason: String,
    },

    /// A required field was not present in some data structure.
    /// Useful when parsing JSON, TOML, or other structured data.
    #[error("Missing required field: {field} in {location}")]
    MissingField { field: String, location: String },

    /// Data was found but did not conform to the expected format.
    #[error("Invalid format in {location}: {reason}")]
    InvalidFormat { location: String, reason: String },

    /// A low-level system call (e.g., ioctl, stat) failed.
    /// Captures the syscall name and the specific reason.
    #[error("System call failed: {syscall} - {reason}")]
    SystemCall { syscall: String, reason: String },

    /// A required file or directory path does not exist.
    #[error("Path not found: {path}")]
    PathNotFound { path: String },

    /// A catch-all for miscellaneous errors that don't fit other variants.
    /// Use sparingly; prefer more specific variants when possible.
    #[error("Other error: {0}")]
    Other(String),

    /// Tried to access a collector by name, but it was not registered.
    #[error("Collector not found for: {0}")]
    CollectorNotFound(String),

    /// A collector is registered but does not support the requested operation
    /// (e.g., a collector that only works on Linux but we're on Windows).
    #[error("Unsupported collector: {0}")]
    UnsupportedCollector(String),
    /// An error occurred while executing a system command.
    /// Includes the command that was run and the error message.
    #[error("Command '{command}' failed: {source}")]
    CommandExecution {
        command: String,
        #[source]
        source: std::io::Error,
    },
}
