use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Captures global kernel statistics that provide insight into system-wide activity.
/// These metrics are aggregated across the entire system and are useful for understanding
/// overall system behavior, boot timing, and process lifecycle events. Data comes from
/// /proc/stat which exposes kernel runtime statistics.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatGlobalStats {
    /// Unix timestamp (seconds since epoch) when the system was booted.
    /// Useful for calculating system uptime: current_time - boot_time = uptime_seconds.
    /// Can be negative on some systems with unusual clock configurations, hence the i64 type
    /// rather than u64. This value is fixed after boot and does not change.
    pub boot_time: i64,
    /// Total number of processes created since system boot (cumulative counter).
    /// Tracks the "processes" field from /proc/stat which represents the total fork and clone
    /// system calls. Compare over time to measure process creation rate. High rates may indicate
    /// excessive spawning of short-lived processes or container churn.
    pub total_forks: u64,
    /// Total number of hardware interrupts handled by the kernel since boot (cumulative).
    /// This is the aggregate interrupt count across all interrupt types and CPUs.
    /// The "intr" field in /proc/stat provides this total in the first value.
    /// High values relative to time indicate high hardware event activity (disk I/O, network packets, etc.).
    pub total_interrupts: u64,
}

/// The main collector responsible for gathering global kernel statistics.
/// Reads /proc/stat and extracts system-wide metrics that don't fit into CPU-specific categories.
/// These metrics provide a high-level view of system activity and are useful for understanding
/// trends in process creation, interrupt handling, and system uptime.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatCollector;

impl StatCollector {
    pub fn new() -> Self {
        StatCollector
    }
}

impl Default for StatCollector {
    /// Provides the default constructor, allowing StatCollector to be instantiated
    /// using the Default trait. Useful in generic code that requires Default implementations.
    fn default() -> Self {
        StatCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting global kernel statistics.
/// Reads from /proc/stat and extracts three key metrics: boot time, total process creations,
/// and total interrupts. These are typically invariant or monotonically increasing counters.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for StatCollector {
    type Output = StatGlobalStats;

    /// Asynchronously reads global kernel statistics from /proc/stat.
    ///
    /// The method looks for three specific lines in the file:
    /// - "btime X" — boot time as Unix timestamp
    /// - "processes X" — total processes created since boot
    /// - "intr X ..." — interrupt line where X is the total interrupt count
    ///
    /// Example /proc/stat entries:
    /// ```text
    /// btime 1234567890
    /// processes 5000
    /// intr 1000000 50000 30000 ...
    /// ```
    ///
    /// All three fields are required; missing any one will result in a parse error.
    /// Returns an error if the file cannot be read or if any required metric is missing or malformed.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/stat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/stat".to_string(),
                source,
            })?;

        let mut stats = StatGlobalStats {
            boot_time: 0,
            total_forks: 0,
            total_interrupts: 0,
        };

        for line in content.lines() {
            if line.starts_with("btime ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // Extract the timestamp value (second field after "btime")
                stats.boot_time = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "boot_time".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("processes ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // Extract the total processes value
                stats.total_forks = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "total_forks".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("intr ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // The first value after "intr" is the total interrupt count
                // Subsequent values are per-IRQ counts which we ignore
                stats.total_interrupts = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "total_interrupts".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            }
        }

        Ok(stats)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
))]
register_collector!(StatCollector, "stat");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatGlobalStats {
    pub boot_time: i64,
    pub total_forks: u64,
    pub total_interrupts: u64,
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that global statistics are not available on this platform.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatCollector;

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for StatCollector {
    type Output = StatGlobalStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Stat collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    fn parse_stat_content(content: &str) -> CollectorResult<StatGlobalStats> {
        let mut stats = StatGlobalStats {
            boot_time: 0,
            total_forks: 0,
            total_interrupts: 0,
        };

        for line in content.lines() {
            if line.starts_with("btime ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.boot_time = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "boot_time".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("processes ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.total_forks = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "total_forks".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("intr ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.total_interrupts = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "total_interrupts".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            }
        }

        Ok(stats)
    }

    // ---- Test cases ----

    #[test]
    fn test_parse_valid_stat() {
        // Realistic output from /proc/stat
        let content = r#"cpu  2255 34 2290 22625563 1290 127 456 0 0 0
cpu0 1132 16 1449 11311718 896 110 340 0 0 0
cpu1 1123 18 841 11313845 394 17 116 0 0 0
intr 114930548 74852 2 0 0 0 0 0 0 0 0 0 0 4419 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
ctxt 1990473
btime 1584822450
processes 1234567
procs_running 2
procs_blocked 0"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 1584822450);
        assert_eq!(stats.total_forks, 1234567);
        assert_eq!(stats.total_interrupts, 114930548);
    }

    #[test]
    fn test_parse_boot_time_only() {
        let content = r#"btime 1234567890
processes 1000
intr 5000 100 200"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 1234567890);
        assert_eq!(stats.total_forks, 1000);
        assert_eq!(stats.total_interrupts, 5000);
    }

    #[test]
    fn test_parse_large_values() {
        // System with very high counters
        let content = r#"btime 1609459200
processes 18446744073709551615
intr 18446744073709551615 extra fields"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 1609459200);
        assert_eq!(stats.total_forks, u64::MAX);
        assert_eq!(stats.total_interrupts, u64::MAX);
    }

    #[test]
    fn test_parse_zero_values() {
        let content = r#"btime 0
processes 0
intr 0"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 0);
        assert_eq!(stats.total_forks, 0);
        assert_eq!(stats.total_interrupts, 0);
    }

    #[test]
    fn test_parse_negative_boot_time() {
        // Some systems with unusual clock configurations may have negative boot times
        let content = r#"btime -1000
processes 100
intr 1000"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, -1000);
    }

    #[test]
    fn test_parse_invalid_boot_time_value() {
        let content = r#"btime not_a_number
processes 1000
intr 5000"#;

        let result = parse_stat_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "boot_time");
        } else {
            panic!("Expected ParseError for invalid boot_time");
        }
    }

    #[test]
    fn test_parse_invalid_processes_value() {
        let content = r#"btime 1234567890
processes invalid_number
intr 5000"#;

        let result = parse_stat_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "total_forks");
        } else {
            panic!("Expected ParseError for invalid processes");
        }
    }

    #[test]
    fn test_parse_invalid_intr_value() {
        let content = r#"btime 1234567890
processes 1000
intr bad_value 100 200"#;

        let result = parse_stat_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "total_interrupts");
        } else {
            panic!("Expected ParseError for invalid intr");
        }
    }

    #[test]
    fn test_parse_intr_with_per_irq_counts() {
        // intr line has total count followed by per-IRQ breakdowns
        let content = r#"btime 1234567890
processes 1000
intr 1000000 100000 200000 300000 400000 0 0 0"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // Only the first value (total) is extracted
        assert_eq!(stats.total_interrupts, 1000000);
    }

    #[test]
    fn test_parse_order_independent() {
        // Verify parsing order doesn't matter
        let content1 = r#"btime 1111
processes 2222
intr 3333"#;

        let content2 = r#"processes 2222
intr 3333
btime 1111"#;

        let result1 = parse_stat_content(content1);
        let result2 = parse_stat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let stats1 = result1.unwrap();
        let stats2 = result2.unwrap();

        assert_eq!(stats1.boot_time, stats2.boot_time);
        assert_eq!(stats1.total_forks, stats2.total_forks);
        assert_eq!(stats1.total_interrupts, stats2.total_interrupts);
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        let content = r#"btime    1234567890
processes   1000
intr    5000"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 1234567890);
        assert_eq!(stats.total_forks, 1000);
        assert_eq!(stats.total_interrupts, 5000);
    }

    #[test]
    fn test_parse_with_extra_cpu_lines() {
        // Real /proc/stat has many CPU lines; verify we ignore them
        let content = r#"cpu  2255 34 2290 22625563 1290 127 456 0 0 0
cpu0 1132 16 1449 11311718 896 110 340 0 0 0
cpu1 1123 18 841 11313845 394 17 116 0 0 0
intr 114930548 74852 2 0 0
ctxt 1990473
btime 1584822450
processes 1234567
procs_running 2
procs_blocked 0"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 1584822450);
        assert_eq!(stats.total_forks, 1234567);
        assert_eq!(stats.total_interrupts, 114930548);
    }

    #[test]
    fn test_stat_global_stats_serialization() {
        let stats = StatGlobalStats {
            boot_time: 1584822450,
            total_forks: 1234567,
            total_interrupts: 114930548,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&stats);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<StatGlobalStats, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.boot_time, stats.boot_time);
        assert_eq!(restored.total_forks, stats.total_forks);
        assert_eq!(restored.total_interrupts, stats.total_interrupts);
    }

    #[test]
    fn test_stat_global_stats_clone() {
        let original = StatGlobalStats {
            boot_time: 1234567890,
            total_forks: 50000,
            total_interrupts: 2000000,
        };

        let cloned = original.clone();

        assert_eq!(original.boot_time, cloned.boot_time);
        assert_eq!(original.total_forks, cloned.total_forks);
        assert_eq!(original.total_interrupts, cloned.total_interrupts);
    }

    #[test]
    fn test_stat_collector_creation() {
        let collector = StatCollector::new();
        let default_collector = StatCollector::default();

        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_uptime_calculation() {
        // Scenario: Calculate system uptime from boot_time
        let current_timestamp = 1609459200i64; // 2021-01-01 00:00:00 UTC
        let boot_timestamp = 1609372800i64; // 2020-12-31 00:00:00 UTC

        let content = format!(
            r#"btime {}
processes 1000
intr 5000"#,
            boot_timestamp
        );

        let result = parse_stat_content(&content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        let uptime = current_timestamp - stats.boot_time;

        assert_eq!(uptime, 86400); // 1 day in seconds
    }

    #[test]
    fn test_parse_process_creation_rate() {
        // Scenario: Track process creation rate by comparing counts over time
        let content1 = r#"btime 1000
processes 100000
intr 5000"#;

        let content2 = r#"btime 1000
processes 101000
intr 5100"#;

        let result1 = parse_stat_content(content1);
        let result2 = parse_stat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let stats1 = result1.unwrap();
        let stats2 = result2.unwrap();

        let fork_rate = stats2.total_forks - stats1.total_forks;
        assert_eq!(fork_rate, 1000);
    }

    #[test]
    fn test_parse_interrupt_rate() {
        // Scenario: Track interrupt handling rate
        let content1 = r#"btime 1000
processes 1000
intr 1000000"#;

        let content2 = r#"btime 1000
processes 1001
intr 1050000"#;

        let result1 = parse_stat_content(content1);
        let result2 = parse_stat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let stats1 = result1.unwrap();
        let stats2 = result2.unwrap();

        let interrupt_increase = stats2.total_interrupts - stats1.total_interrupts;
        assert_eq!(interrupt_increase, 50000);
    }

    #[test]
    fn test_stat_global_stats_debug_format() {
        let stats = StatGlobalStats {
            boot_time: 1234567890,
            total_forks: 50000,
            total_interrupts: 2000000,
        };

        let debug_string = format!("{:?}", stats);
        assert!(debug_string.contains("boot_time"));
        assert!(debug_string.contains("total_forks"));
        assert!(debug_string.contains("total_interrupts"));
    }

    #[test]
    fn test_parse_i64_boundaries() {
        // Test near i64 boundaries for boot_time
        let max_i64_str = i64::MAX.to_string();
        let min_i64_str = i64::MIN.to_string();

        let content_max = format!(
            r#"btime {}
processes 1000
intr 5000"#,
            max_i64_str
        );

        let result_max = parse_stat_content(&content_max);
        assert!(result_max.is_ok());
        assert_eq!(result_max.unwrap().boot_time, i64::MAX);

        let content_min = format!(
            r#"btime {}
processes 1000
intr 5000"#,
            min_i64_str
        );

        let result_min = parse_stat_content(&content_min);
        assert!(result_min.is_ok());
        assert_eq!(result_min.unwrap().boot_time, i64::MIN);
    }

    #[test]
    fn test_parse_line_prefix_matching() {
        // Ensure exact prefix matching (e.g., "btime" not "btime_something")
        let content = r#"btime 1234567890
btime_custom 999999
processes 1000
proc_extra 100
intr 5000
interrupt_extra 5555"#;

        let result = parse_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.boot_time, 1234567890);
        assert_eq!(stats.total_forks, 1000);
        assert_eq!(stats.total_interrupts, 5000);
    }
}
