use super::types::CollectorResult;

/// A core trait that every data collector must implement.
///
/// `DataProducer` defines the contract for any component that can gather
/// system metrics, statistics, or other telemetry data asynchronously.
/// It is designed to be object-safe when wrapped (see `DynCollector`), thread-safe,
/// and usable across async boundaries.
///
/// The trait is marked with `'static` to allow collectors to be stored in the
/// global registry without lifetime complications.
#[async_trait::async_trait]
pub trait DataProducer: Send + Sync + 'static {
    /// The type of data this producer returns.
    ///
    /// This is typically a struct containing the collected metrics.
    /// It must be `Send + Sync + 'static` so that it can be safely boxed
    /// as a dynamic `Serialize` trait object later in the pipeline.
    type Output: Send + Sync + 'static;

    /// Asynchronously collects and returns the data for this metric.
    ///
    /// Implementations should perform whatever I/O or computation is needed
    /// (e.g., reading `/proc` files, running system commands, querying hardware)
    /// and return the structured result.
    ///
    /// Any error during collection should be converted into a `CollectorError`
    /// and returned via the `CollectorResult` type alias (which is `Result<T, CollectorError>`).
    async fn produce(&self) -> CollectorResult<Self::Output>;
}
