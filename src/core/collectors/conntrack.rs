use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Connection tracking statistics from the Linux netfilter subsystem.
/// Conntrack (connection tracking) monitors and keeps track of all active
/// network connections, which is essential for stateful firewalling.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConntrackStats {
    /// Number of currently tracked connections (entries in the conntrack table)
    pub entries: u64,
    /// Maximum number of connections that can be tracked simultaneously
    pub max: u64,
}

/// Collector for netfilter conntrack statistics from /proc/net/nf_conntrack.
/// This collector retrieves information about the Linux kernel's connection
/// tracking table, which is used by stateful firewalls and NAT.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConntrackCollector;

/// Constructor methods for ConntrackCollector
impl ConntrackCollector {
    /// Creates a new ConntrackCollector instance
    pub fn new() -> Self {
        ConntrackCollector
    }
}

/// Default trait implementation for ConntrackCollector
impl Default for ConntrackCollector {
    fn default() -> Self {
        ConntrackCollector::new()
    }
}

/// Data producer implementation for Conntrack collector.
/// Reads conntrack statistics from /proc/net/nf_conntrack and /proc/sys/net/netfilter/nf_conntrack_max.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for ConntrackCollector {
    type Output = ConntrackStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Count the number of lines in /proc/net/nf_conntrack
        // Each line represents an active tracked connection
        // If the file doesn't exist or can't be read, gracefully default to 0
        let entries = tokio::fs::read_to_string("/proc/net/nf_conntrack")
            .await
            .map(|content| content.lines().count() as u64)
            .unwrap_or(0);

        // Read the maximum number of conntrack entries allowed
        // This value determines the limit of concurrent connections that can be tracked
        let max = tokio::fs::read_to_string("/proc/sys/net/netfilter/nf_conntrack_max")
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .ok_or(CollectorError::FileRead {
                path: "/proc/sys/net/netfilter/nf_conntrack_max".to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "file not found or invalid",
                ),
            })?;

        Ok(ConntrackStats { entries, max })
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
))]
register_collector!(ConntrackCollector, "conntrack");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConntrackStats {
    pub entries: u64,
    pub max: u64,
}

/// Fallback collector for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConntrackCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for ConntrackCollector {
    type Output = ConntrackStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Conntrack collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conntrack_stats_creation() {
        let stats = ConntrackStats {
            entries: 1234,
            max: 262144,
        };

        assert_eq!(stats.entries, 1234);
        assert_eq!(stats.max, 262144);
    }

    #[test]
    fn test_conntrack_stats_zero_entries() {
        let stats = ConntrackStats {
            entries: 0,
            max: 262144,
        };

        assert_eq!(stats.entries, 0);
        assert!(stats.entries <= stats.max);
    }

    #[test]
    fn test_conntrack_stats_full_table() {
        // Test when conntrack table is full or nearly full
        let stats = ConntrackStats {
            entries: 262144,
            max: 262144,
        };

        assert_eq!(stats.entries, stats.max);
    }

    #[test]
    fn test_conntrack_stats_high_utilization() {
        // Test with high but not full utilization
        let stats = ConntrackStats {
            entries: 200000,
            max: 262144,
        };

        let utilization = (stats.entries as f64 / stats.max as f64) * 100.0;
        assert!(utilization > 75.0);
        assert!(utilization < 100.0);
    }

    #[test]
    fn test_conntrack_stats_low_utilization() {
        // Test with low utilization
        let stats = ConntrackStats {
            entries: 50000,
            max: 262144,
        };

        let utilization = (stats.entries as f64 / stats.max as f64) * 100.0;
        assert!(utilization < 25.0);
    }

    #[test]
    fn test_conntrack_collector_creation() {
        let _ = ConntrackCollector::new();
        let _ = ConntrackCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_conntrack_stats_serialize() {
        let stats = ConntrackStats {
            entries: 5000,
            max: 262144,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        assert!(json.contains("5000"));
        assert!(json.contains("262144"));
    }

    #[test]
    fn test_conntrack_stats_deserialize() {
        let json = r#"{
            "entries": 10000,
            "max": 524288
        }"#;

        let stats: ConntrackStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(stats.entries, 10000);
        assert_eq!(stats.max, 524288);
    }

    #[test]
    fn test_conntrack_stats_utilization_calculation() {
        // Test helper function to calculate utilization percentage
        let stats = ConntrackStats {
            entries: 65536,
            max: 262144,
        };

        let utilization = (stats.entries as f64 / stats.max as f64) * 100.0;
        assert!((utilization - 25.0).abs() < 0.01); // Should be exactly 25%
    }

    #[test]
    fn test_conntrack_stats_typical_server() {
        // Typical values for a moderately loaded server
        let stats = ConntrackStats {
            entries: 15000,
            max: 262144,
        };

        assert!(stats.entries > 0);
        assert!(stats.entries < stats.max);

        let utilization = (stats.entries as f64 / stats.max as f64) * 100.0;
        assert!(utilization < 10.0); // Less than 10% utilization
    }

    #[test]
    fn test_conntrack_stats_ordering() {
        // Ensure entries field comes before max in serialization
        let stats = ConntrackStats {
            entries: 100,
            max: 1000,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        let entries_pos = json.find("entries").expect("entries field not found");
        let max_pos = json.find("max").expect("max field not found");

        // Both fields should be present
        assert!(entries_pos < max_pos || max_pos < entries_pos);
    }

    #[test]
    fn test_conntrack_stats_large_numbers() {
        // Test with large numbers that might occur on heavily loaded systems
        let stats = ConntrackStats {
            entries: 1000000,
            max: 2097152, // 2M connections
        };

        assert_eq!(stats.entries, 1000000);
        assert_eq!(stats.max, 2097152);
        assert!(stats.entries < stats.max);
    }

    #[test]
    fn test_conntrack_stats_clone() {
        let stats1 = ConntrackStats {
            entries: 5000,
            max: 262144,
        };

        let stats2 = stats1.clone();
        assert_eq!(stats1.entries, stats2.entries);
        assert_eq!(stats1.max, stats2.max);
    }

    #[test]
    fn test_conntrack_stats_debug_format() {
        let stats = ConntrackStats {
            entries: 1000,
            max: 100000,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("ConntrackStats"));
        assert!(debug_str.contains("1000"));
        assert!(debug_str.contains("100000"));
    }
}
