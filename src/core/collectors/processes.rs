use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Captures snapshot metrics about the current state of processes on the system.
/// These metrics are collected from the /proc/stat kernel interface and provide
/// system-wide process information at a point in time.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    /// Total number of processes created since system boot. This is a monotonically
    /// increasing counter and useful for detecting process creation rate over time intervals.
    pub total_processes: u64,
    /// Number of processes currently in the running state. These are processes scheduled
    /// to run on the CPU. Note that this is not the same as "number of processes that are
    /// executing on a CPU right now" — it includes processes waiting in the run queue.
    pub running_processes: u64,
    /// Cumulative number of context switches that have occurred since system boot.
    /// A context switch happens when the kernel switches execution from one process to another.
    /// High values indicate significant CPU scheduling activity and contention.
    pub context_switches: u64,
}

/// The main collector responsible for gathering process-related metrics from the kernel.
/// Reads data from /proc/stat which contains system-wide statistics including process counts
/// and context switch information. This collector provides insights into process scheduling
/// and system load characteristics.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessCollector;

impl ProcessCollector {
    /// Creates a new process collector instance.
    pub fn new() -> Self {
        ProcessCollector
    }
}

impl Default for ProcessCollector {
    /// Provides the default constructor, allowing ProcessCollector to be instantiated
    /// using the Default trait. Useful in generic code that requires Default implementations.
    fn default() -> Self {
        ProcessCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting process metrics.
/// Reads from /proc/stat and parses three key metrics: total processes, running processes,
/// and context switches. The implementation handles potential file read errors and parse errors
/// with descriptive error messages.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for ProcessCollector {
    type Output = ProcessStats;

    /// Asynchronously reads process statistics from /proc/stat and returns them in a ProcessStats struct.
    /// The method looks for three specific lines in the file:
    /// - "processes X" — total processes created since boot
    /// - "procs_running X" — currently running processes
    /// - "ctxt X" — total context switches
    ///
    /// Returns an error if the file cannot be read or if required metrics are malformed.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/stat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/stat".to_string(),
                source,
            })?;

        let mut stats = ProcessStats {
            total_processes: 0,
            running_processes: 0,
            context_switches: 0,
        };

        // Parse the /proc/stat file line by line, extracting the metrics we care about
        for line in content.lines() {
            if line.starts_with("processes ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.total_processes = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "total_processes".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("procs_running ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.running_processes = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "running_processes".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("ctxt ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.context_switches = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "context_switches".to_string(),
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
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
))]
register_collector!(ProcessCollector, "processes");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub total_processes: u64,
    pub running_processes: u64,
    pub context_switches: u64,
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that process metrics are not available on this platform.
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessCollector;

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for ProcessCollector {
    type Output = ProcessStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Processes collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    /// This function extracts the core parsing logic for testability.
    fn parse_proc_stat_content(content: &str) -> CollectorResult<ProcessStats> {
        let mut stats = ProcessStats {
            total_processes: 0,
            running_processes: 0,
            context_switches: 0,
        };

        for line in content.lines() {
            if line.starts_with("processes ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.total_processes = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "total_processes".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("procs_running ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.running_processes = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "running_processes".to_string(),
                        location: "/proc/stat".to_string(),
                        reason: "invalid value".to_string(),
                    },
                )?;
            } else if line.starts_with("ctxt ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                stats.context_switches = parts.get(1).and_then(|s| s.parse().ok()).ok_or(
                    CollectorError::ParseError {
                        metric: "context_switches".to_string(),
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
    fn test_parse_valid_proc_stat() {
        // Realistic output from /proc/stat on a running Linux system
        let content = r#"cpu  2255 34 2290 22625563 1290 127 456 0 0 0
cpu0 1132 16 1449 11311718 896 110 340 0 0 0
cpu1 1123 18 841 11313845 394 17 116 0 0 0
intr 114930548 74852 2 0 0 0 0 0 0 0 0 0 0 4419 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
ctxt 1990473
btime 1584822450
processes 1234567
procs_running 2
procs_blocked 0"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_processes, 1234567);
        assert_eq!(stats.running_processes, 2);
        assert_eq!(stats.context_switches, 1990473);
    }

    #[test]
    fn test_parse_zero_values() {
        let content = r#"ctxt 0
processes 0
procs_running 0"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_processes, 0);
        assert_eq!(stats.running_processes, 0);
        assert_eq!(stats.context_switches, 0);
    }

    #[test]
    fn test_parse_high_values() {
        // Test with very large values that fit in u64
        let content = r#"processes 18446744073709551615
ctxt 18446744073709551615
procs_running 1000"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_processes, u64::MAX);
        assert_eq!(stats.context_switches, u64::MAX);
        assert_eq!(stats.running_processes, 1000);
    }

    #[test]
    fn test_parse_invalid_total_processes() {
        let content = r#"processes notanumber
ctxt 1000
procs_running 5"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "total_processes");
        } else {
            panic!("Expected ParseError for total_processes");
        }
    }

    #[test]
    fn test_parse_invalid_running_processes() {
        let content = r#"processes 1000
ctxt 2000
procs_running badvalue"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "running_processes");
        } else {
            panic!("Expected ParseError for running_processes");
        }
    }

    #[test]
    fn test_parse_invalid_context_switches() {
        let content = r#"processes 5000
ctxt invalid_number
procs_running 3"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "context_switches");
        } else {
            panic!("Expected ParseError for context_switches");
        }
    }

    #[test]
    fn test_parse_missing_fields_defaults_to_zero() {
        // Only one field present, others should default to 0
        let content = "ctxt 5000";

        let result = parse_proc_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_processes, 0);
        assert_eq!(stats.running_processes, 0);
        assert_eq!(stats.context_switches, 5000);
    }

    #[test]
    fn test_parse_extra_whitespace() {
        // Test that the parser handles extra whitespace correctly
        let content = "processes   987654\nctxt   999999\nprocs_running   10";

        let result = parse_proc_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_processes, 987654);
        assert_eq!(stats.context_switches, 999999);
        assert_eq!(stats.running_processes, 10);
    }

    #[test]
    fn test_parse_order_independent() {
        // Verify that the order of lines doesn't matter
        let content1 = r#"processes 100
ctxt 200
procs_running 3"#;

        let content2 = r#"ctxt 200
procs_running 3
processes 100"#;

        let result1 = parse_proc_stat_content(content1);
        let result2 = parse_proc_stat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let stats1 = result1.unwrap();
        let stats2 = result2.unwrap();

        assert_eq!(stats1.total_processes, stats2.total_processes);
        assert_eq!(stats1.running_processes, stats2.running_processes);
        assert_eq!(stats1.context_switches, stats2.context_switches);
    }

    #[test]
    fn test_process_collector_creation() {
        let collector = ProcessCollector::new();
        let default_collector = ProcessCollector::default();

        // Both should create valid instances without panicking
        let _ = (collector, default_collector);
    }

    #[test]
    fn test_process_stats_serialization() {
        let stats = ProcessStats {
            total_processes: 50000,
            running_processes: 5,
            context_switches: 2000000,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&stats);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        // Verify we can deserialize it back
        let deserialized: Result<ProcessStats, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.total_processes, stats.total_processes);
        assert_eq!(restored.running_processes, stats.running_processes);
        assert_eq!(restored.context_switches, stats.context_switches);
    }

    #[test]
    fn test_process_stats_clone() {
        let original = ProcessStats {
            total_processes: 12345,
            running_processes: 4,
            context_switches: 3000000,
        };

        let cloned = original.clone();

        assert_eq!(original.total_processes, cloned.total_processes);
        assert_eq!(original.running_processes, cloned.running_processes);
        assert_eq!(original.context_switches, cloned.context_switches);
    }

    #[test]
    fn test_parse_with_extra_lines() {
        // Real /proc/stat has many more lines, verify we ignore irrelevant ones
        let content = r#"cpu  2255 34 2290 22625563 1290 127 456 0 0 0
cpu0 1132 16 1449 11311718 896 110 340 0 0 0
cpu1 1123 18 841 11313845 394 17 116 0 0 0
intr 114930548 74852 2 0 0
softirq 97234 0 17675 8 42 0 0 0 0 0 77509
ctxt 2500000
btime 1584822450
processes 555555
procs_running 8
procs_blocked 1"#;

        let result = parse_proc_stat_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_processes, 555555);
        assert_eq!(stats.running_processes, 8);
        assert_eq!(stats.context_switches, 2500000);
    }

    #[test]
    fn test_parse_negative_values_as_unsigned() {
        // u64 cannot represent negative numbers, so this should fail to parse
        let content = "processes -100\nctxt 1000\nprocs_running 2";

        let result = parse_proc_stat_content(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_stats_debug_format() {
        let stats = ProcessStats {
            total_processes: 1000,
            running_processes: 5,
            context_switches: 100000,
        };

        let debug_string = format!("{:?}", stats);
        assert!(debug_string.contains("total_processes"));
        assert!(debug_string.contains("running_processes"));
        assert!(debug_string.contains("context_switches"));
    }
}
