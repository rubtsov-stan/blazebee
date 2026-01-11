use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// IP Virtual Server (IPVS) statistics.
/// IPVS is a Linux kernel module that implements Layer 4 load balancing.
/// It maintains statistics about connections, packets, and bytes handled by the load balancer.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpvsStats {
    /// Total number of connections handled by IPVS load balancer
    pub connections: u64,
    /// Total number of incoming packets processed
    pub incoming_packets: u64,
    /// Total number of outgoing packets processed
    pub outgoing_packets: u64,
    /// Total number of incoming bytes processed
    pub incoming_bytes: u64,
    /// Total number of outgoing bytes processed
    pub outgoing_bytes: u64,
}

/// Collector for IPVS (IP Virtual Server) statistics from /proc/net/ip_vs_stats.
/// This collector monitors Linux kernel's load balancing subsystem which is used for
/// high-performance Layer 4 load balancing without requiring user-space daemons.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpvsCollector;

/// Constructor methods for IpvsCollector
impl IpvsCollector {
    /// Creates a new IpvsCollector instance
    pub fn new() -> Self {
        IpvsCollector
    }
}

/// Default trait implementation for IpvsCollector
impl Default for IpvsCollector {
    fn default() -> Self {
        IpvsCollector::new()
    }
}

/// Data producer implementation for IPVS collector.
/// Reads from /proc/net/ip_vs_stats and aggregates load balancing statistics.
/// The file contains header lines followed by a statistics line with connection and traffic data.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for IpvsCollector {
    type Output = IpvsStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the IPVS statistics file
        // Format: header lines followed by statistics line with aggregated counters
        let content = tokio::fs::read_to_string("/proc/net/ip_vs_stats")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/net/ip_vs_stats".to_string(),
                source,
            })?;

        // Initialize stats with zeros
        // These will be populated from the statistics line
        let mut stats = IpvsStats {
            connections: 0,
            incoming_packets: 0,
            outgoing_packets: 0,
            incoming_bytes: 0,
            outgoing_bytes: 0,
        };

        // Skip the first 2 header lines and process the statistics line
        // The file format is:
        // IP VS connections statistics
        //    0         10        100       1000      10000     100000    1000000
        // ... (actual statistics values follow)
        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();

            // The statistics line must have at least 5 fields
            if parts.len() >= 5 {
                // Parse connections count (field 0)
                stats.connections = parts[0].parse().map_err(|_| CollectorError::ParseError {
                    metric: "connections".to_string(),
                    location: "/proc/net/ip_vs_stats".to_string(),
                    reason: format!("invalid value: {}", parts[0]),
                })?;

                // Parse incoming packets count (field 1)
                stats.incoming_packets =
                    parts[1].parse().map_err(|_| CollectorError::ParseError {
                        metric: "incoming_packets".to_string(),
                        location: "/proc/net/ip_vs_stats".to_string(),
                        reason: format!("invalid value: {}", parts[1]),
                    })?;

                // Parse outgoing packets count (field 2)
                stats.outgoing_packets =
                    parts[2].parse().map_err(|_| CollectorError::ParseError {
                        metric: "outgoing_packets".to_string(),
                        location: "/proc/net/ip_vs_stats".to_string(),
                        reason: format!("invalid value: {}", parts[2]),
                    })?;

                // Parse incoming bytes count (field 3)
                stats.incoming_bytes =
                    parts[3].parse().map_err(|_| CollectorError::ParseError {
                        metric: "incoming_bytes".to_string(),
                        location: "/proc/net/ip_vs_stats".to_string(),
                        reason: format!("invalid value: {}", parts[3]),
                    })?;

                // Parse outgoing bytes count (field 4)
                stats.outgoing_bytes =
                    parts[4].parse().map_err(|_| CollectorError::ParseError {
                        metric: "outgoing_bytes".to_string(),
                        location: "/proc/net/ip_vs_stats".to_string(),
                        reason: format!("invalid value: {}", parts[4]),
                    })?;
            }
        }

        Ok(stats)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
))]
register_collector!(IpvsCollector, "ipvs");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpvsStats {
    pub connections: u64,
    pub incoming_packets: u64,
    pub outgoing_packets: u64,
    pub incoming_bytes: u64,
    pub outgoing_bytes: u64,
}

/// Fallback collector for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpvsCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for IpvsCollector {
    type Output = IpvsStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "IPVS collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipvs_stats_creation() {
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        assert_eq!(stats.connections, 1000);
        assert_eq!(stats.incoming_packets, 5000000);
        assert_eq!(stats.outgoing_packets, 4500000);
    }

    #[test]
    fn test_ipvs_stats_zero_traffic() {
        // Test with no IPVS activity (freshly started or disabled)
        let stats = IpvsStats {
            connections: 0,
            incoming_packets: 0,
            outgoing_packets: 0,
            incoming_bytes: 0,
            outgoing_bytes: 0,
        };

        assert_eq!(stats.connections, 0);
        assert_eq!(stats.incoming_bytes, 0);
    }

    #[test]
    fn test_ipvs_stats_high_traffic() {
        // Test with high load balancer activity
        let stats = IpvsStats {
            connections: 100000,
            incoming_packets: 1000000000,
            outgoing_packets: 900000000,
            incoming_bytes: 500000000000,
            outgoing_bytes: 400000000000,
        };

        assert!(stats.connections > 50000);
        assert!(stats.incoming_bytes > stats.outgoing_bytes);
    }

    #[test]
    fn test_ipvs_stats_typical_load_balancer() {
        // Typical values for a moderately loaded load balancer
        let stats = IpvsStats {
            connections: 5000,
            incoming_packets: 50000000,
            outgoing_packets: 45000000,
            incoming_bytes: 25000000000,
            outgoing_bytes: 20000000000,
        };

        assert!(stats.connections > 1000);
        assert!(stats.connections < 50000);
    }

    #[test]
    fn test_ipvs_collector_creation() {
        let _ = IpvsCollector::new();
        let _ = IpvsCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_ipvs_stats_serialize() {
        let stats = IpvsStats {
            connections: 2000,
            incoming_packets: 10000000,
            outgoing_packets: 9000000,
            incoming_bytes: 5000000000,
            outgoing_bytes: 4000000000,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        assert!(json.contains("2000"));
        assert!(json.contains("10000000"));
    }

    #[test]
    fn test_ipvs_stats_deserialize() {
        let json = r#"{
            "connections": 3000,
            "incoming_packets": 15000000,
            "outgoing_packets": 13500000,
            "incoming_bytes": 7500000000,
            "outgoing_bytes": 6000000000
        }"#;

        let stats: IpvsStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(stats.connections, 3000);
        assert_eq!(stats.incoming_packets, 15000000);
    }

    #[test]
    fn test_ipvs_stats_clone() {
        let stats1 = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let stats2 = stats1.clone();
        assert_eq!(stats1.connections, stats2.connections);
        assert_eq!(stats1.incoming_bytes, stats2.incoming_bytes);
    }

    #[test]
    fn test_ipvs_stats_debug_format() {
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("IpvsStats"));
        assert!(debug_str.contains("1000"));
    }

    #[test]
    fn test_ipvs_stats_total_packets() {
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let total_packets = stats.incoming_packets + stats.outgoing_packets;
        assert_eq!(total_packets, 9500000);
    }

    #[test]
    fn test_ipvs_stats_total_bytes() {
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let total_bytes = stats.incoming_bytes + stats.outgoing_bytes;
        assert_eq!(total_bytes, 4500000000);
    }

    #[test]
    fn test_ipvs_stats_average_packet_size() {
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 5000000000,
            outgoing_bytes: 4000000000,
        };

        let total_packets = stats.incoming_packets + stats.outgoing_packets;
        let total_bytes = stats.incoming_bytes + stats.outgoing_bytes;

        if total_packets > 0 {
            let avg_packet_size = total_bytes as f64 / total_packets as f64;
            assert!(avg_packet_size > 100.0); // Average packet size in bytes
            assert!(avg_packet_size < 10000.0);
        }
    }

    #[test]
    fn test_ipvs_stats_traffic_asymmetry() {
        // Test typical asymmetric traffic (more incoming than outgoing)
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 6000000,
            outgoing_packets: 4000000,
            incoming_bytes: 3000000000,
            outgoing_bytes: 2000000000,
        };

        assert!(stats.incoming_packets > stats.outgoing_packets);
        assert!(stats.incoming_bytes > stats.outgoing_bytes);
    }

    #[test]
    fn test_ipvs_stats_high_connections_low_traffic() {
        // Test scenario: many short connections with low data transfer
        let stats = IpvsStats {
            connections: 50000,
            incoming_packets: 100000,
            outgoing_packets: 100000,
            incoming_bytes: 10000000,
            outgoing_bytes: 10000000,
        };

        // Many connections but little traffic per connection
        let bytes_per_connection =
            (stats.incoming_bytes + stats.outgoing_bytes) / stats.connections;
        assert!(bytes_per_connection < 1000);
    }

    #[test]
    fn test_ipvs_stats_low_connections_high_traffic() {
        // Test scenario: few large connections with high data transfer
        let stats = IpvsStats {
            connections: 100,
            incoming_packets: 10000000,
            outgoing_packets: 10000000,
            incoming_bytes: 5000000000,
            outgoing_bytes: 5000000000,
        };

        // Few connections but lots of data per connection
        let bytes_per_connection =
            (stats.incoming_bytes + stats.outgoing_bytes) / stats.connections;
        assert!(bytes_per_connection > 50000000);
    }

    #[test]
    fn test_ipvs_stats_serialization_roundtrip() {
        let original = IpvsStats {
            connections: 4000,
            incoming_packets: 20000000,
            outgoing_packets: 18000000,
            incoming_bytes: 10000000000,
            outgoing_bytes: 8000000000,
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: IpvsStats = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(original.connections, deserialized.connections);
        assert_eq!(original.incoming_bytes, deserialized.incoming_bytes);
    }

    #[test]
    fn test_ipvs_stats_maximum_u64() {
        // Test with maximum u64 values (theoretical maximum)
        let stats = IpvsStats {
            connections: u64::MAX,
            incoming_packets: u64::MAX,
            outgoing_packets: u64::MAX,
            incoming_bytes: u64::MAX,
            outgoing_bytes: u64::MAX,
        };

        assert_eq!(stats.connections, u64::MAX);
    }

    #[test]
    fn test_ipvs_stats_comparison() {
        let low_load = IpvsStats {
            connections: 100,
            incoming_packets: 1000000,
            outgoing_packets: 900000,
            incoming_bytes: 500000000,
            outgoing_bytes: 400000000,
        };

        let high_load = IpvsStats {
            connections: 10000,
            incoming_packets: 100000000,
            outgoing_packets: 90000000,
            incoming_bytes: 50000000000,
            outgoing_bytes: 40000000000,
        };

        assert!(low_load.connections < high_load.connections);
        assert!(low_load.incoming_bytes < high_load.incoming_bytes);
    }

    #[test]
    fn test_ipvs_stats_stress_scenario() {
        // Scenario: IPVS under heavy DDoS or high load testing
        let before_stress = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let during_stress = IpvsStats {
            connections: 50000,
            incoming_packets: 500000000,
            outgoing_packets: 450000000,
            incoming_bytes: 250000000000,
            outgoing_bytes: 200000000000,
        };

        let connection_increase = during_stress.connections - before_stress.connections;
        assert_eq!(connection_increase, 49000);
    }

    #[test]
    fn test_ipvs_collector_default_impl() {
        let collector1 = IpvsCollector::new();
        let collector2 = IpvsCollector::default();

        let json1 = serde_json::to_string(&collector1).expect("serialization failed");
        let json2 = serde_json::to_string(&collector2).expect("serialization failed");
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_ipvs_stats_clone_independence() {
        let mut stats1 = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let stats2 = stats1.clone();

        stats1.connections = 2000;

        assert_eq!(stats1.connections, 2000);
        assert_eq!(stats2.connections, 1000);
    }

    #[test]
    fn test_ipvs_stats_packet_loss_detection() {
        // Detect potential packet loss by comparing packet counts
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000, // Some packets not responded to
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let packet_ratio = stats.outgoing_packets as f64 / stats.incoming_packets as f64;
        assert!(packet_ratio < 1.0); // More incoming than outgoing
        assert!(packet_ratio > 0.8); // But not too much loss
    }

    #[test]
    fn test_ipvs_stats_throughput_calculation() {
        // Calculate estimated throughput
        let stats = IpvsStats {
            connections: 1000,
            incoming_packets: 5000000,
            outgoing_packets: 4500000,
            incoming_bytes: 2500000000,
            outgoing_bytes: 2000000000,
        };

        let total_bytes = stats.incoming_bytes + stats.outgoing_bytes;
        let total_packets = stats.incoming_packets + stats.outgoing_packets;

        // Bytes per packet metric
        if total_packets > 0 {
            let efficiency = total_bytes as f64 / total_packets as f64;
            assert!(efficiency > 0.0);
        }
    }
}
