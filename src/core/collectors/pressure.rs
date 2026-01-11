use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Represents a single pressure metric for a specific resource type (CPU, memory, or I/O).
/// Pressure stall information (PSI) measures the time tasks spend waiting for resources.
/// The "some" metrics track when at least one task is stalled, while "full" metrics
/// track when all tasks are stalled. Average values are computed over 10s, 60s, and 300s windows.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureStats<'a> {
    /// Identifies the pressure type: "cpu", "memory", or "io"
    pub metric_type: Cow<'a, str>,
    /// Percentage of time at least one task was stalled (10-second average)
    pub some_avg10: f64,
    /// Percentage of time at least one task was stalled (60-second average)
    pub some_avg60: f64,
    /// Percentage of time at least one task was stalled (300-second average)
    pub some_avg300: f64,
    /// Percentage of time all tasks were stalled (10-second average), None if not applicable
    pub full_avg10: Option<f64>,
    /// Percentage of time all tasks were stalled (60-second average), None if not applicable
    pub full_avg60: Option<f64>,
    /// Percentage of time all tasks were stalled (300-second average), None if not applicable
    pub full_avg300: Option<f64>,
}

/// A container for multiple pressure statistics. This buffer holds metrics from all
/// supported pressure types collected in a single read operation.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PressureBuffer<'a> {
    stats: Vec<PressureStats<'a>>,
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
impl<'a> PressureBuffer<'a> {
    /// Creates an empty pressure buffer with no pre-allocated capacity.
    pub fn new() -> Self {
        PressureBuffer { stats: Vec::new() }
    }

    /// Creates an empty pressure buffer with pre-allocated capacity to reduce allocations.
    /// Useful when the number of pressure types is known in advance.
    pub fn with_capacity(capacity: usize) -> Self {
        PressureBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Appends a pressure statistic to the buffer.
    pub fn push(&mut self, stat: PressureStats<'a>) {
        self.stats.push(stat);
    }

    /// Returns an iterator over all pressure statistics in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &PressureStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of pressure statistics currently in the buffer.
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer contains any statistics.
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Returns the statistics as a slice for direct array access.
    pub fn as_slice(&self) -> &[PressureStats<'a>] {
        &self.stats
    }
}

/// The main collector that reads pressure metrics from Linux /proc/pressure files.
/// This collector is responsible for gathering CPU, memory, and I/O pressure information
/// from the kernel's PSI interface.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
pub struct PressureCollector;

impl PressureCollector {
    pub fn new() -> Self {
        PressureCollector
    }
}

impl Default for PressureCollector {
    fn default() -> Self {
        PressureCollector::new()
    }
}

/// Implementation of the DataProducer trait for collecting pressure metrics.
/// Asynchronously reads pressure information from /proc/pressure/{cpu,memory,io} files.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for PressureCollector {
    type Output = PressureBuffer<'static>;

    /// Collects pressure metrics from the kernel's PSI interface.
    /// Reads three files: /proc/pressure/cpu, /proc/pressure/memory, and /proc/pressure/io.
    /// Non-existent files are silently skipped (some systems may not support all pressure types).
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let mut buffer = PressureBuffer::with_capacity(3);

        for pressure_type in &["cpu", "memory", "io"] {
            let path = format!("/proc/pressure/{}", pressure_type);
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    let (some_avg10, some_avg60, some_avg300, full_avg10, full_avg60, full_avg300) =
                        Self::parse_pressure(&content)?;

                    buffer.push(PressureStats {
                        metric_type: Cow::Owned(pressure_type.to_string()),
                        some_avg10,
                        some_avg60,
                        some_avg300,
                        full_avg10,
                        full_avg60,
                        full_avg300,
                    });
                }
                Err(_) => {
                    // Silently ignore missing or inaccessible files
                    // This allows graceful handling on systems without full PSI support
                }
            }
        }

        Ok(buffer)
    }
}
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
impl PressureCollector {
    /// Parses the content of a pressure file and extracts average values.
    ///
    /// The kernel format typically looks like this:
    ///
    /// some avg10=X.XX avg60=X.XX avg300=X.XX total=XXXXXX
    /// full avg10=X.XX avg60=X.XX avg300=X.XX total=XXXXXX
    ///
    /// The "full" line may not exist for all resource types (e.g., missing for CPU).
    fn parse_pressure(
        content: &str,
    ) -> CollectorResult<(f64, f64, f64, Option<f64>, Option<f64>, Option<f64>)> {
        let mut some_avg10 = 0.0;
        let mut some_avg60 = 0.0;
        let mut some_avg300 = 0.0;
        let mut full_avg10: Option<f64> = None;
        let mut full_avg60: Option<f64> = None;
        let mut full_avg300: Option<f64> = None;

        for line in content.lines() {
            if line.starts_with("some ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in parts {
                    if part.starts_with("avg10=") {
                        some_avg10 = part[6..].parse().map_err(|_| CollectorError::ParseError {
                            metric: "pressure_some_avg10".to_string(),
                            location: "/proc/pressure".to_string(),
                            reason: format!("invalid value: {}", part),
                        })?;
                    } else if part.starts_with("avg60=") {
                        some_avg60 = part[6..].parse().map_err(|_| CollectorError::ParseError {
                            metric: "pressure_some_avg60".to_string(),
                            location: "/proc/pressure".to_string(),
                            reason: format!("invalid value: {}", part),
                        })?;
                    } else if part.starts_with("avg300=") {
                        some_avg300 =
                            part[7..].parse().map_err(|_| CollectorError::ParseError {
                                metric: "pressure_some_avg300".to_string(),
                                location: "/proc/pressure".to_string(),
                                reason: format!("invalid value: {}", part),
                            })?;
                    }
                }
            } else if line.starts_with("full ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in parts {
                    if part.starts_with("avg10=") {
                        full_avg10 =
                            Some(part[6..].parse().map_err(|_| CollectorError::ParseError {
                                metric: "pressure_full_avg10".to_string(),
                                location: "/proc/pressure".to_string(),
                                reason: format!("invalid value: {}", part),
                            })?);
                    } else if part.starts_with("avg60=") {
                        full_avg60 =
                            Some(part[6..].parse().map_err(|_| CollectorError::ParseError {
                                metric: "pressure_full_avg60".to_string(),
                                location: "/proc/pressure".to_string(),
                                reason: format!("invalid value: {}", part),
                            })?);
                    } else if part.starts_with("avg300=") {
                        full_avg300 =
                            Some(part[7..].parse().map_err(|_| CollectorError::ParseError {
                                metric: "pressure_full_avg300".to_string(),
                                location: "/proc/pressure".to_string(),
                                reason: format!("invalid value: {}", part),
                            })?);
                    }
                }
            }
        }

        Ok((
            some_avg10,
            some_avg60,
            some_avg300,
            full_avg10,
            full_avg60,
            full_avg300,
        ))
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
register_collector!(PressureCollector, "pressure");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definitions for non-Linux platforms or when the collector is not enabled.
/// These are identical in structure to the Linux versions but the collector returns an error.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureStats<'a> {
    pub metric_type: Cow<'a, str>,
    pub some_avg10: f64,
    pub some_avg60: f64,
    pub some_avg300: f64,
    pub full_avg10: Option<f64>,
    pub full_avg60: Option<f64>,
    pub full_avg300: Option<f64>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PressureBuffer<'a> {
    stats: Vec<PressureStats<'a>>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
)))]
impl<'a> PressureBuffer<'a> {
    pub fn new() -> Self {
        PressureBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        PressureBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stat: PressureStats<'a>) {
        self.stats.push(stat);
    }

    pub fn iter(&self) -> impl Iterator<Item = &PressureStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    pub fn as_slice(&self) -> &[PressureStats<'a>] {
        &self.stats
    }
}

/// Fallback collector for unsupported platforms. Always returns an error indicating
/// that the pressure collector is not available on the current platform.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
)))]
pub struct PressureCollector;

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for PressureCollector {
    type Output = PressureBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Pressure collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pressure_with_full_metrics() {
        // Realistic output from /proc/pressure/memory on a busy system
        let content = r#"some avg10=5.25 avg60=6.35 avg300=4.20 total=12345
full avg10=2.10 avg60=3.50 avg300=1.80 total=56789"#;

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_ok());

        let (some_10, some_60, some_300, full_10, full_60, full_300) = result.unwrap();
        assert_eq!(some_10, 5.25);
        assert_eq!(some_60, 6.35);
        assert_eq!(some_300, 4.20);
        assert_eq!(full_10, Some(2.10));
        assert_eq!(full_60, Some(3.50));
        assert_eq!(full_300, Some(1.80));
    }

    #[test]
    fn test_parse_pressure_cpu_only() {
        // CPU pressure typically doesn't have a "full" line
        let content = "some avg10=1.25 avg60=2.35 avg300=3.40 total=9999";

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_ok());

        let (some_10, some_60, some_300, full_10, full_60, full_300) = result.unwrap();
        assert_eq!(some_10, 1.25);
        assert_eq!(some_60, 2.35);
        assert_eq!(some_300, 3.40);
        assert!(full_10.is_none());
        assert!(full_60.is_none());
        assert!(full_300.is_none());
    }

    #[test]
    fn test_parse_pressure_with_zero_values() {
        let content = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=0
full avg10=0.00 avg60=0.00 avg300=0.00 total=0"#;

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_ok());

        let (some_10, some_60, some_300, full_10, full_60, full_300) = result.unwrap();
        assert_eq!(some_10, 0.0);
        assert_eq!(some_60, 0.0);
        assert_eq!(some_300, 0.0);
        assert_eq!(full_10, Some(0.0));
        assert_eq!(full_60, Some(0.0));
        assert_eq!(full_300, Some(0.0));
    }

    #[test]
    fn test_parse_pressure_invalid_value() {
        let content = "some avg10=invalid avg60=2.35 avg300=3.40 total=9999";

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pressure_missing_some_line() {
        // File with only full line (unusual but should not crash)
        let content = "full avg10=1.5 avg60=2.0 avg300=2.5 total=1000";

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_ok());

        let (some_10, some_60, some_300, full_10, full_60, full_300) = result.unwrap();
        // When "some" line is missing, values default to 0.0
        assert_eq!(some_10, 0.0);
        assert_eq!(some_60, 0.0);
        assert_eq!(some_300, 0.0);
        assert_eq!(full_10, Some(1.5));
        assert_eq!(full_60, Some(2.0));
        assert_eq!(full_300, Some(2.5));
    }

    #[test]
    fn test_pressure_buffer_creation() {
        let buffer = PressureBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_pressure_buffer_with_capacity() {
        let buffer = PressureBuffer::with_capacity(10);
        assert_eq!(buffer.len(), 0);
        // The stats vector should have capacity 10 but length 0
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_pressure_buffer_push_and_iter() {
        let mut buffer = PressureBuffer::new();

        let stat1 = PressureStats {
            metric_type: Cow::Borrowed("cpu"),
            some_avg10: 1.0,
            some_avg60: 2.0,
            some_avg300: 3.0,
            full_avg10: None,
            full_avg60: None,
            full_avg300: None,
        };

        let stat2 = PressureStats {
            metric_type: Cow::Borrowed("memory"),
            some_avg10: 4.0,
            some_avg60: 5.0,
            some_avg300: 6.0,
            full_avg10: Some(1.5),
            full_avg60: Some(2.5),
            full_avg300: Some(3.5),
        };

        buffer.push(stat1);
        buffer.push(stat2);

        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());

        let stats: Vec<_> = buffer.iter().collect();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].metric_type, "cpu");
        assert_eq!(stats[1].metric_type, "memory");
    }

    #[test]
    fn test_pressure_buffer_as_slice() {
        let mut buffer = PressureBuffer::new();

        let stat = PressureStats {
            metric_type: Cow::Borrowed("io"),
            some_avg10: 7.5,
            some_avg60: 8.5,
            some_avg300: 9.5,
            full_avg10: Some(4.0),
            full_avg60: Some(5.0),
            full_avg300: Some(6.0),
        };

        buffer.push(stat);

        let slice = buffer.as_slice();
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].metric_type, "io");
        assert_eq!(slice[0].some_avg10, 7.5);
    }

    #[test]
    fn test_pressure_collector_creation() {
        let collector = PressureCollector::new();
        let default_collector = PressureCollector::default();
        // Both should create valid instances without panicking
        let _ = (collector, default_collector);
    }

    #[test]
    fn test_pressure_stats_serialization() {
        let stat = PressureStats {
            metric_type: Cow::Borrowed("cpu"),
            some_avg10: 1.5,
            some_avg60: 2.5,
            some_avg300: 3.5,
            full_avg10: Some(0.5),
            full_avg60: Some(1.0),
            full_avg300: Some(1.5),
        };

        // Test that serialization doesn't panic
        let json = serde_json::to_string(&stat);
        assert!(json.is_ok());

        // Test round-trip serialization
        let serialized = json.unwrap();
        let deserialized: Result<PressureStats, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());
    }

    #[test]
    fn test_parse_pressure_with_extra_whitespace() {
        let content = "some  avg10=1.5  avg60=2.5  avg300=3.5  total=5000";

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_ok());

        let (some_10, some_60, some_300, _, _, _) = result.unwrap();
        assert_eq!(some_10, 1.5);
        assert_eq!(some_60, 2.5);
        assert_eq!(some_300, 3.5);
    }

    #[test]
    fn test_parse_pressure_high_values() {
        // Test with high pressure values (close to 100%)
        let content = "some avg10=95.50 avg60=98.25 avg300=87.40 total=999999";

        let result = PressureCollector::parse_pressure(content);
        assert!(result.is_ok());

        let (some_10, some_60, some_300, _, _, _) = result.unwrap();
        assert_eq!(some_10, 95.50);
        assert_eq!(some_60, 98.25);
        assert_eq!(some_300, 87.40);
    }
}
