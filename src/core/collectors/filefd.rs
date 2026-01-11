use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// File descriptor statistics for the entire system.
/// File descriptors are used by processes to reference open files, sockets,
/// pipes, and other I/O resources. The kernel maintains a pool of file descriptors
/// that can be allocated to processes as needed.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFdStats {
    /// Total number of file descriptors currently allocated across all processes
    pub allocated: u64,
    /// Maximum number of file descriptors that can be allocated system-wide.
    /// If allocated reaches this limit, no new files can be opened.
    pub maximum: u64,
}

/// Collector for system-wide file descriptor statistics from /proc/sys/fs/file-nr.
/// This collector monitors how many file descriptors are in use versus the maximum
/// available, which is important for detecting file descriptor exhaustion.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilefdCollector;

/// Constructor methods for FilefdCollector
impl FilefdCollector {
    /// Creates a new FilefdCollector instance
    pub fn new() -> Self {
        FilefdCollector
    }
}

/// Default trait implementation for FilefdCollector
impl Default for FilefdCollector {
    fn default() -> Self {
        FilefdCollector::new()
    }
}

/// Data producer implementation for Filefd collector.
/// Reads from /proc/sys/fs/file-nr to get system-wide file descriptor statistics.
/// The file contains three values: allocated, reserved, and maximum.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for FilefdCollector {
    type Output = FileFdStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the file-nr file which contains file descriptor statistics
        // Format: "allocated reserved maximum"
        // Example: "576 0 3289152"
        let content = tokio::fs::read_to_string("/proc/sys/fs/file-nr")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/sys/fs/file-nr".to_string(),
                source,
            })?;

        // Split the file content into whitespace-separated fields
        let parts: Vec<&str> = content.split_whitespace().collect();

        // Parse the allocated count (first field, index 0)
        // This is the total number of open file descriptors in the system
        let allocated =
            parts
                .get(0)
                .and_then(|s| s.parse().ok())
                .ok_or(CollectorError::MissingField {
                    field: "allocated".to_string(),
                    location: "/proc/sys/fs/file-nr".to_string(),
                })?;

        // Parse the maximum count (third field, index 2)
        // The second field (index 1) is "reserved" and is not used in this collector
        // This is the system-wide limit of file descriptors
        let maximum =
            parts
                .get(2)
                .and_then(|s| s.parse().ok())
                .ok_or(CollectorError::MissingField {
                    field: "maximum".to_string(),
                    location: "/proc/sys/fs/file-nr".to_string(),
                })?;

        Ok(FileFdStats { allocated, maximum })
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
))]
register_collector!(FilefdCollector, "filefd");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFdStats {
    pub allocated: u64,
    pub maximum: u64,
}

/// Fallback collector for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilefdCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for FilefdCollector {
    type Output = FileFdStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Filefd collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filefd_stats_creation() {
        let stats = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };

        assert_eq!(stats.allocated, 512);
        assert_eq!(stats.maximum, 3289152);
    }

    #[test]
    fn test_filefd_stats_zero_allocated() {
        // Test with zero allocated file descriptors (unlikely but valid)
        let stats = FileFdStats {
            allocated: 0,
            maximum: 3289152,
        };

        assert_eq!(stats.allocated, 0);
        assert!(stats.allocated < stats.maximum);
    }

    #[test]
    fn test_filefd_stats_nearly_exhausted() {
        // Test when file descriptor limit is nearly reached
        let stats = FileFdStats {
            allocated: 3200000,
            maximum: 3289152,
        };

        assert!(stats.allocated < stats.maximum);

        let utilization = (stats.allocated as f64 / stats.maximum as f64) * 100.0;
        assert!(utilization > 97.0); // More than 97% utilized
    }

    #[test]
    fn test_filefd_stats_exhausted() {
        // Test when file descriptor limit is reached (critical state)
        let stats = FileFdStats {
            allocated: 3289152,
            maximum: 3289152,
        };

        assert_eq!(stats.allocated, stats.maximum);
    }

    #[test]
    fn test_filefd_stats_typical_usage() {
        // Typical values on a moderately loaded system
        let stats = FileFdStats {
            allocated: 1024,
            maximum: 3289152,
        };

        let utilization = (stats.allocated as f64 / stats.maximum as f64) * 100.0;
        assert!(utilization < 1.0); // Less than 1% utilization
    }

    #[test]
    fn test_filefd_stats_high_usage() {
        // High usage on a heavily loaded system
        let stats = FileFdStats {
            allocated: 100000,
            maximum: 3289152,
        };

        let utilization = (stats.allocated as f64 / stats.maximum as f64) * 100.0;
        assert!(utilization > 3.0);
        assert!(utilization < 10.0);
    }

    #[test]
    fn test_filefd_collector_creation() {
        let _ = FilefdCollector::new();
        let _ = FilefdCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_filefd_stats_serialize() {
        let stats = FileFdStats {
            allocated: 768,
            maximum: 3289152,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        assert!(json.contains("768"));
        assert!(json.contains("3289152"));
    }

    #[test]
    fn test_filefd_stats_deserialize() {
        let json = r#"{
            "allocated": 1024,
            "maximum": 4194304
        }"#;

        let stats: FileFdStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(stats.allocated, 1024);
        assert_eq!(stats.maximum, 4194304);
    }

    #[test]
    fn test_filefd_stats_clone() {
        let stats1 = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };

        let stats2 = stats1.clone();
        assert_eq!(stats1.allocated, stats2.allocated);
        assert_eq!(stats1.maximum, stats2.maximum);
    }

    #[test]
    fn test_filefd_stats_debug_format() {
        let stats = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("FileFdStats"));
        assert!(debug_str.contains("512"));
        assert!(debug_str.contains("3289152"));
    }

    #[test]
    fn test_filefd_stats_utilization_calculation() {
        let stats = FileFdStats {
            allocated: 1645760,
            maximum: 3289152,
        };

        let utilization = (stats.allocated as f64 / stats.maximum as f64) * 100.0;
        assert!((utilization - 50.0).abs() < 0.1); // Should be approximately 50%
    }

    #[test]
    fn test_filefd_stats_small_system() {
        // File descriptor limits on older or resource-constrained systems
        let stats = FileFdStats {
            allocated: 256,
            maximum: 65536,
        };

        assert_eq!(stats.allocated, 256);
        assert_eq!(stats.maximum, 65536);
    }

    #[test]
    fn test_filefd_stats_large_system() {
        // File descriptor limits on large production servers
        let stats = FileFdStats {
            allocated: 1000000,
            maximum: 10000000,
        };

        assert_eq!(stats.allocated, 1000000);
        assert_eq!(stats.maximum, 10000000);
    }

    #[test]
    fn test_filefd_stats_remaining_capacity() {
        // Calculate remaining available file descriptors
        let stats = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };

        let remaining = stats.maximum - stats.allocated;
        assert_eq!(remaining, 3288640);
    }

    #[test]
    fn test_filefd_stats_serialization_roundtrip() {
        let original = FileFdStats {
            allocated: 2048,
            maximum: 4194304,
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: FileFdStats =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(original.allocated, deserialized.allocated);
        assert_eq!(original.maximum, deserialized.maximum);
    }

    #[test]
    fn test_filefd_stats_comparison() {
        let low_usage = FileFdStats {
            allocated: 256,
            maximum: 3289152,
        };
        let high_usage = FileFdStats {
            allocated: 2000000,
            maximum: 3289152,
        };

        assert!(low_usage.allocated < high_usage.allocated);
        assert_eq!(low_usage.maximum, high_usage.maximum);
    }

    #[test]
    fn test_filefd_stats_stress_scenario() {
        // Scenario where system is under stress and opening many connections
        let before_stress = FileFdStats {
            allocated: 5000,
            maximum: 3289152,
        };
        let during_stress = FileFdStats {
            allocated: 150000,
            maximum: 3289152,
        };

        let increase = during_stress.allocated - before_stress.allocated;
        assert_eq!(increase, 145000);
    }

    #[test]
    fn test_filefd_collector_default_impl() {
        let collector1 = FilefdCollector::new();
        let collector2 = FilefdCollector::default();

        let json1 = serde_json::to_string(&collector1).expect("serialization failed");
        let json2 = serde_json::to_string(&collector2).expect("serialization failed");
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_filefd_stats_clone_independence() {
        let mut stats1 = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };
        let stats2 = stats1.clone();

        stats1.allocated = 1024;

        assert_eq!(stats1.allocated, 1024);
        assert_eq!(stats2.allocated, 512);
    }

    #[test]
    fn test_filefd_stats_maximum_u64() {
        // Test with maximum u64 values (theoretical maximum)
        let stats = FileFdStats {
            allocated: u64::MAX,
            maximum: u64::MAX,
        };

        assert_eq!(stats.allocated, u64::MAX);
        assert_eq!(stats.maximum, u64::MAX);
    }

    #[test]
    fn test_filefd_typical_default_limit() {
        // Typical default limit on modern Linux systems
        let typical_modern = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };

        // This is a common default limit
        assert_eq!(typical_modern.maximum, 3289152);
    }

    #[test]
    fn test_filefd_per_process_vs_system() {
        // System-wide file descriptors vs typical per-process limit
        let system_stats = FileFdStats {
            allocated: 512,
            maximum: 3289152,
        };

        // Typical per-process limit is much lower (often 1024)
        // System-wide limit includes all processes
        assert!(system_stats.maximum > 1024);
    }
}
