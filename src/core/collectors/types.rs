use super::error::CollectorError;

/// A convenient type alias for results returned by collectors.
///
/// Throughout the collector system, operations that can fail (such as reading files,
/// parsing metrics, or accessing system resources) return a `Result` where the
/// error type is always our domain-specific `CollectorError`.
///
/// Using this alias keeps the code clean and consistent, making it immediately
/// clear that any function returning `CollectorResult<T>` may encounter one of
/// the well-defined collector-related errors.
pub type CollectorResult<T> = std::result::Result<T, CollectorError>;
