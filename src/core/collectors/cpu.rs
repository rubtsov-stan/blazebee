use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// CPU time statistics for a specific CPU core or aggregate.
/// Contains information about how much time the CPU spent in different states.
/// All values are in jiffies (usually 1/100th of a second on x86).
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStats<'a> {
    /// CPU identifier (e.g., "cpu0", "cpu1", or "cpu" for aggregate)
    pub cpu: Cow<'a, str>,
    /// Time spent in user mode (running user applications)
    pub user: u64,
    /// Time spent in user mode with low priority (nice)
    pub nice: u64,
    /// Time spent in kernel (system) mode
    pub system: u64,
    /// Time spent idle (not executing anything)
    pub idle: u64,
    /// Time spent waiting for I/O to complete
    pub iowait: u64,
    /// Time spent handling hardware interrupts
    pub irq: u64,
    /// Time spent handling software interrupts
    pub softirq: u64,
}

/// Buffer for storing CPU statistics from multiple CPU cores.
/// Uses Vec internally for efficient storage and iteration.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CpuBuffer<'a> {
    stats: Vec<CpuStats<'a>>,
}

/// Methods for working with the CPU buffer
impl<'a> CpuBuffer<'a> {
    /// Creates a new empty CPU buffer
    pub fn new() -> Self {
        CpuBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of CPU cores.
    /// Typical capacity would be the number of CPU cores available on the system.
    pub fn with_capacity(capacity: usize) -> Self {
        CpuBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds a CPU statistics entry to the buffer
    pub fn push(&mut self, stats: CpuStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the CPU statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &CpuStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of CPU entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Returns a slice of all CPU statistics entries
    pub fn as_slice(&self) -> &[CpuStats<'a>] {
        &self.stats
    }
}

/// The CPU collector that reads from /proc/stat on Linux systems.
/// Collects detailed CPU time statistics for both individual cores and system aggregate.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
pub struct CpuCollector;

/// Constructor methods for CpuCollector
impl CpuCollector {
    /// Creates a new CPU collector instance
    pub fn new() -> Self {
        CpuCollector
    }
}

/// Default trait implementation for CpuCollector
impl Default for CpuCollector {
    fn default() -> Self {
        CpuCollector::new()
    }
}

/// Data producer implementation for CPU collector.
/// Reads from /proc/stat and parses CPU time statistics for all available cores.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for CpuCollector {
    type Output = CpuBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the entire /proc/stat file which contains CPU statistics
        let content = tokio::fs::read_to_string("/proc/stat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/stat".to_string(),
                source,
            })?;

        // Initialize buffer with capacity for up to 16 CPU cores (can be adjusted)
        let mut buffer = CpuBuffer::with_capacity(16);

        // Process each line in /proc/stat
        for line in content.lines() {
            // Only process lines that start with "cpu" (skip other metrics)
            if line.starts_with("cpu") {
                let parts: Vec<&str> = line.split_whitespace().collect();

                // Each CPU line must have at least 8 fields (cpu label + 7 counters)
                if parts.len() < 8 {
                    continue;
                }

                // Extract the CPU label (e.g., "cpu0", "cpu1", or "cpu")
                let cpu_label = parts[0];

                // Parse user time (field index 1)
                let user = parts[1]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "user".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[1]),
                    })?;

                // Parse nice time (field index 2)
                let nice = parts[2]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "nice".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[2]),
                    })?;

                // Parse system time (field index 3)
                let system = parts[3]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "system".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[3]),
                    })?;

                // Parse idle time (field index 4)
                let idle = parts[4]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "idle".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[4]),
                    })?;

                // Parse I/O wait time (field index 5)
                let iowait = parts[5]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "iowait".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[5]),
                    })?;

                // Parse hardware interrupt time (field index 6)
                let irq = parts[6]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "irq".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[6]),
                    })?;

                // Parse software interrupt time (field index 7)
                let softirq = parts[7]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "softirq".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: format!("invalid value: {}", parts[7]),
                    })?;

                // Create a CpuStats entry with the parsed values
                buffer.push(CpuStats {
                    cpu: Cow::Owned(cpu_label.to_string()),
                    user,
                    nice,
                    system,
                    idle,
                    iowait,
                    irq,
                    softirq,
                });
            }
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
register_collector!(CpuCollector, "cpu");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
)))]
pub struct CpuCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for CpuCollector {
    type Output = ();

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "CPU collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_stats_creation() {
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1000,
            nice: 100,
            system: 500,
            idle: 10000,
            iowait: 200,
            irq: 50,
            softirq: 75,
        };

        assert_eq!(cpu_stats.cpu, "cpu0");
        assert_eq!(cpu_stats.user, 1000);
        assert_eq!(cpu_stats.system, 500);
        assert_eq!(cpu_stats.idle, 10000);
    }

    #[test]
    fn test_cpu_stats_aggregate() {
        // Test for aggregate CPU stats (no core number)
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu"),
            user: 8000,
            nice: 800,
            system: 4000,
            idle: 80000,
            iowait: 1600,
            irq: 400,
            softirq: 600,
        };

        assert_eq!(cpu_stats.cpu, "cpu");
        // Aggregate should have higher values than individual cores
        assert!(cpu_stats.user > 0);
    }

    #[test]
    fn test_cpu_buffer_creation() {
        let buffer = CpuBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_cpu_buffer_with_capacity() {
        let buffer = CpuBuffer::with_capacity(8);
        assert_eq!(buffer.len(), 0);
        // Buffer has capacity but no entries yet
    }

    #[test]
    fn test_cpu_buffer_push_single() {
        let mut buffer = CpuBuffer::new();
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1000,
            nice: 100,
            system: 500,
            idle: 10000,
            iowait: 200,
            irq: 50,
            softirq: 75,
        };

        buffer.push(cpu_stats);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_cpu_buffer_multiple_cores() {
        let mut buffer = CpuBuffer::with_capacity(4);

        for i in 0..4 {
            let cpu_stats = CpuStats {
                cpu: Cow::Owned(format!("cpu{}", i)),
                user: 1000 * (i as u64 + 1),
                nice: 100,
                system: 500,
                idle: 10000,
                iowait: 200,
                irq: 50,
                softirq: 75,
            };
            buffer.push(cpu_stats);
        }

        assert_eq!(buffer.len(), 4);
        let slice = buffer.as_slice();
        assert_eq!(slice.len(), 4);
    }

    #[test]
    fn test_cpu_buffer_iterator() {
        let mut buffer = CpuBuffer::new();

        for i in 0..3 {
            let cpu_stats = CpuStats {
                cpu: Cow::Owned(format!("cpu{}", i)),
                user: 1000,
                nice: 100,
                system: 500,
                idle: 10000,
                iowait: 200,
                irq: 50,
                softirq: 75,
            };
            buffer.push(cpu_stats);
        }

        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_cpu_stats_serialize() {
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1000,
            nice: 100,
            system: 500,
            idle: 10000,
            iowait: 200,
            irq: 50,
            softirq: 75,
        };

        let json = serde_json::to_string(&cpu_stats).expect("serialization failed");
        assert!(json.contains("cpu0"));
        assert!(json.contains("1000"));
        assert!(json.contains("10000"));
    }

    #[test]
    fn test_cpu_stats_deserialize() {
        let json = r#"{
            "cpu": "cpu0",
            "user": 2000,
            "nice": 200,
            "system": 1000,
            "idle": 20000,
            "iowait": 400,
            "irq": 100,
            "softirq": 150
        }"#;

        let cpu_stats: CpuStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(cpu_stats.cpu, "cpu0");
        assert_eq!(cpu_stats.user, 2000);
        assert_eq!(cpu_stats.idle, 20000);
    }

    #[test]
    fn test_cpu_collector_creation() {
        let _ = CpuCollector::new();
        let _ = CpuCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_cpu_buffer_as_slice() {
        let mut buffer = CpuBuffer::new();

        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1000,
            nice: 100,
            system: 500,
            idle: 10000,
            iowait: 200,
            irq: 50,
            softirq: 75,
        };

        buffer.push(cpu_stats);
        let slice = buffer.as_slice();

        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].cpu, "cpu0");
    }

    #[test]
    fn test_cpu_stats_total_time() {
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1000,
            nice: 100,
            system: 500,
            idle: 10000,
            iowait: 200,
            irq: 50,
            softirq: 75,
        };

        // Calculate total CPU time
        let total = cpu_stats.user
            + cpu_stats.nice
            + cpu_stats.system
            + cpu_stats.idle
            + cpu_stats.iowait
            + cpu_stats.irq
            + cpu_stats.softirq;

        assert_eq!(total, 11925);
    }

    #[test]
    fn test_cpu_buffer_clone() {
        let mut buffer1 = CpuBuffer::new();

        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1000,
            nice: 100,
            system: 500,
            idle: 10000,
            iowait: 200,
            irq: 50,
            softirq: 75,
        };

        buffer1.push(cpu_stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
        assert_eq!(buffer1.len(), 1);
    }

    #[test]
    fn test_cpu_stats_zero_times() {
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 0,
            nice: 0,
            system: 0,
            idle: 0,
            iowait: 0,
            irq: 0,
            softirq: 0,
        };

        assert_eq!(cpu_stats.user, 0);
        // All times can theoretically be zero (unusual case)
    }

    #[test]
    fn test_cpu_stats_large_numbers() {
        // Test with large numbers that occur on systems running for a long time
        let cpu_stats = CpuStats {
            cpu: Cow::Borrowed("cpu0"),
            user: 1_000_000_000,
            nice: 100_000_000,
            system: 500_000_000,
            idle: 10_000_000_000,
            iowait: 200_000_000,
            irq: 50_000_000,
            softirq: 75_000_000,
        };

        assert_eq!(cpu_stats.user, 1_000_000_000);
        assert_eq!(cpu_stats.idle, 10_000_000_000);
    }
}
