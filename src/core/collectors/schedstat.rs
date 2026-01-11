use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Represents CPU scheduling statistics for a single logical CPU core.
/// These metrics are derived from the kernel's process scheduler and provide
/// detailed insights into CPU scheduling behavior and contention. All values
/// are in nanoseconds since the scheduler was initialized.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedstatCpu<'a> {
    /// Identifier for the CPU core (e.g., "0", "1", "2"). This is the logical CPU
    /// number as presented by the kernel, not necessarily the physical processor number.
    pub cpu: Cow<'a, str>,
    /// Total time this CPU core has spent actively executing task code (in nanoseconds).
    /// This is the actual CPU time spent in user and kernel code, not including idle time
    /// or time spent servicing interrupts.
    pub running_ns: u64,
    /// Total time tasks have spent in the runqueue waiting to be scheduled on this CPU
    /// (in nanoseconds). High values indicate the CPU is oversubscribed with runnable tasks.
    /// Compare with running_ns to determine if the system is CPU-bound.
    pub runnable_ns: u64,
    /// Total time the scheduler has delayed running tasks on this CPU (in nanoseconds).
    /// This includes involuntary context switches and time spent in the scheduler itself.
    /// High values suggest scheduling overhead or lock contention in the kernel scheduler.
    pub run_delay_ns: u64,
}

/// Container for CPU scheduling statistics from all logical cores.
/// This buffer collects schedstat data for each CPU and provides convenient
/// access through standard collection methods. Data comes from /proc/schedstat.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SchedstatBuffer<'a> {
    stats: Vec<SchedstatCpu<'a>>,
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
impl<'a> SchedstatBuffer<'a> {
    /// Creates an empty schedstat buffer with no pre-allocated capacity.
    pub fn new() -> Self {
        SchedstatBuffer { stats: Vec::new() }
    }

    /// Creates an empty schedstat buffer with pre-allocated capacity.
    /// Useful when the number of CPUs is known in advance (typical on fixed hardware).
    pub fn with_capacity(capacity: usize) -> Self {
        SchedstatBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Appends CPU scheduling statistics to the buffer.
    pub fn push(&mut self, stat: SchedstatCpu<'a>) {
        self.stats.push(stat);
    }

    /// Returns an iterator over all CPU scheduling statistics in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &SchedstatCpu<'a>> {
        self.stats.iter()
    }

    /// Returns the number of CPUs whose statistics are stored in the buffer.
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer contains any CPU statistics.
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Returns the statistics as a slice for direct array access.
    pub fn as_slice(&self) -> &[SchedstatCpu<'a>] {
        &self.stats
    }
}

/// The main collector responsible for gathering CPU scheduling statistics from the kernel.
/// Reads /proc/schedstat which exposes the kernel scheduler's performance counters for each
/// CPU. These metrics are essential for performance analysis and detecting CPU contention issues.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
pub struct SchedstatCollector;

impl SchedstatCollector {
    pub fn new() -> Self {
        SchedstatCollector
    }
}

impl Default for SchedstatCollector {
    fn default() -> Self {
        SchedstatCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting scheduler statistics.
/// Reads from /proc/schedstat and extracts per-CPU scheduling metrics that describe how the
/// kernel's process scheduler is performing. The parser is lenient with extra fields, extracting
/// only the first three numeric values (running, runnable, run_delay) from each CPU line.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for SchedstatCollector {
    type Output = SchedstatBuffer<'static>;

    /// Asynchronously reads CPU scheduling statistics from /proc/schedstat.
    ///
    /// The file format contains lines like:
    /// ```text
    /// cpu0 12345678 87654321 1234567890
    /// cpu1 11111111 22222222 3333333333
    /// ```
    /// Each line starting with "cpu" is parsed to extract:
    /// - CPU identifier (0, 1, 2, etc.)
    /// - running_ns: nanoseconds spent running
    /// - runnable_ns: nanoseconds spent runnable (waiting for CPU)
    /// - run_delay_ns: nanoseconds of scheduler delay
    ///
    /// Lines with fewer than 9 fields (including the cpu label and other kernel data)
    /// are skipped. Parse errors in the three required metrics cause the entire
    /// collection to fail with a descriptive error message.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/schedstat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/schedstat".to_string(),
                source,
            })?;

        let mut buffer = SchedstatBuffer::new();

        for line in content.lines() {
            if line.starts_with("cpu") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // Ensure we have enough fields: cpu label + at least 8 more fields (running, runnable, delay, + extra kernel fields)
                if parts.len() >= 9 {
                    let cpu_id = parts[0].trim_start_matches("cpu");

                    let running_ns = parts[1].parse().map_err(|_| CollectorError::ParseError {
                        metric: "running_ns".to_string(),
                        location: format!("/proc/schedstat cpu={}", cpu_id),
                        reason: format!("invalid value: {}", parts[1]),
                    })?;

                    let runnable_ns = parts[2].parse().map_err(|_| CollectorError::ParseError {
                        metric: "runnable_ns".to_string(),
                        location: format!("/proc/schedstat cpu={}", cpu_id),
                        reason: format!("invalid value: {}", parts[2]),
                    })?;

                    let run_delay_ns =
                        parts[3].parse().map_err(|_| CollectorError::ParseError {
                            metric: "run_delay_ns".to_string(),
                            location: format!("/proc/schedstat cpu={}", cpu_id),
                            reason: format!("invalid value: {}", parts[3]),
                        })?;

                    buffer.push(SchedstatCpu {
                        cpu: Cow::Owned(cpu_id.to_string()),
                        running_ns,
                        runnable_ns,
                        run_delay_ns,
                    });
                }
            }
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
register_collector!(SchedstatCollector, "schedstat");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedstatCpu<'a> {
    pub cpu: Cow<'a, str>,
    pub running_ns: u64,
    pub runnable_ns: u64,
    pub run_delay_ns: u64,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SchedstatBuffer<'a> {
    stats: Vec<SchedstatCpu<'a>>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
)))]
impl<'a> SchedstatBuffer<'a> {
    pub fn new() -> Self {
        SchedstatBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        SchedstatBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stat: SchedstatCpu<'a>) {
        self.stats.push(stat);
    }

    pub fn iter(&self) -> impl Iterator<Item = &SchedstatCpu<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    pub fn as_slice(&self) -> &[SchedstatCpu<'a>] {
        &self.stats
    }
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that scheduling statistics are not available on this platform.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
)))]
pub struct SchedstatCollector;

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for SchedstatCollector {
    type Output = SchedstatBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Schedstat collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    fn parse_schedstat_content(content: &str) -> CollectorResult<SchedstatBuffer<'static>> {
        let mut buffer = SchedstatBuffer::new();

        for line in content.lines() {
            if line.starts_with("cpu") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 9 {
                    let cpu_id = parts[0].trim_start_matches("cpu");

                    let running_ns = parts[1].parse().map_err(|_| CollectorError::ParseError {
                        metric: "running_ns".to_string(),
                        location: format!("/proc/schedstat cpu={}", cpu_id),
                        reason: format!("invalid value: {}", parts[1]),
                    })?;

                    let runnable_ns = parts[2].parse().map_err(|_| CollectorError::ParseError {
                        metric: "runnable_ns".to_string(),
                        location: format!("/proc/schedstat cpu={}", cpu_id),
                        reason: format!("invalid value: {}", parts[2]),
                    })?;

                    let run_delay_ns =
                        parts[3].parse().map_err(|_| CollectorError::ParseError {
                            metric: "run_delay_ns".to_string(),
                            location: format!("/proc/schedstat cpu={}", cpu_id),
                            reason: format!("invalid value: {}", parts[3]),
                        })?;

                    buffer.push(SchedstatCpu {
                        cpu: Cow::Owned(cpu_id.to_string()),
                        running_ns,
                        runnable_ns,
                        run_delay_ns,
                    });
                }
            }
        }

        Ok(buffer)
    }

    // ---- Test cases ----

    #[test]
    fn test_schedstat_buffer_operations() {
        let mut buffer = SchedstatBuffer::new();

        let stat1 = SchedstatCpu {
            cpu: Cow::Borrowed("0"),
            running_ns: 1000,
            runnable_ns: 2000,
            run_delay_ns: 3000,
        };

        let stat2 = SchedstatCpu {
            cpu: Cow::Borrowed("1"),
            running_ns: 1100,
            runnable_ns: 2100,
            run_delay_ns: 3100,
        };

        buffer.push(stat1);
        buffer.push(stat2);

        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());

        let stats: Vec<_> = buffer.iter().collect();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].cpu, "0");
        assert_eq!(stats[1].cpu, "1");

        let slice = buffer.as_slice();
        assert_eq!(slice[0].running_ns, 1000);
        assert_eq!(slice[1].running_ns, 1100);
    }

    #[test]
    fn test_schedstat_buffer_with_capacity() {
        let buffer = SchedstatBuffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_schedstat_cpu_serialization() {
        let stat = SchedstatCpu {
            cpu: Cow::Borrowed("2"),
            running_ns: 5000000000,
            runnable_ns: 10000000000,
            run_delay_ns: 1000000000,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&stat);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<SchedstatCpu, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.cpu, "2");
        assert_eq!(restored.running_ns, stat.running_ns);
    }

    #[test]
    fn test_schedstat_cpu_clone() {
        let original = SchedstatCpu {
            cpu: Cow::Owned("5".to_string()),
            running_ns: 5555,
            runnable_ns: 6666,
            run_delay_ns: 7777,
        };

        let cloned = original.clone();

        assert_eq!(original.cpu, cloned.cpu);
        assert_eq!(original.running_ns, cloned.running_ns);
        assert_eq!(original.runnable_ns, cloned.runnable_ns);
        assert_eq!(original.run_delay_ns, cloned.run_delay_ns);
    }

    #[test]
    fn test_schedstat_collector_creation() {
        let collector = SchedstatCollector::new();
        let default_collector = SchedstatCollector::default();

        // Both should create valid instances without panicking
        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";

        let result = parse_schedstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_schedstat_cpu_debug_format() {
        let stat = SchedstatCpu {
            cpu: Cow::Borrowed("3"),
            running_ns: 1111111,
            runnable_ns: 2222222,
            run_delay_ns: 3333333,
        };

        let debug_string = format!("{:?}", stat);
        assert!(debug_string.contains("running_ns"));
        assert!(debug_string.contains("runnable_ns"));
        assert!(debug_string.contains("1111111"));
    }
}
