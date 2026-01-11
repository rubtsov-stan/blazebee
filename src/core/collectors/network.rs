use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Network interface statistics.
/// Contains comprehensive information about traffic (bytes/packets) and quality metrics
/// (errors/drops) for both receive and transmit directions on a single network interface.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats<'a> {
    /// Network interface name (e.g., "eth0", "wlan0", "lo", "docker0")
    pub interface: Cow<'a, str>,
    /// Total bytes received on this interface
    pub receive_bytes: u64,
    /// Total packets received on this interface
    pub receive_packets: u64,
    /// Total receive errors (CRC, frame errors, etc.)
    pub receive_errors: u64,
    /// Total dropped receive packets (buffer overflow, etc.)
    pub receive_drops: u64,
    /// Total bytes transmitted on this interface
    pub transmit_bytes: u64,
    /// Total packets transmitted on this interface
    pub transmit_packets: u64,
    /// Total transmit errors (collision, carrier, etc.)
    pub transmit_errors: u64,
    /// Total dropped transmit packets (buffer overflow, etc.)
    pub transmit_drops: u64,
}

/// Buffer for storing network statistics from multiple interfaces.
/// /proc/net/dev contains one entry per network interface, making a buffer useful
/// for collecting and processing statistics across all or selected interfaces.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkBuffer<'a> {
    stats: Vec<NetworkStats<'a>>,
}

/// Methods for working with the network buffer
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
impl<'a> NetworkBuffer<'a> {
    /// Creates a new empty network buffer
    pub fn new() -> Self {
        NetworkBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of interfaces.
    /// Typical systems have 5-20 interfaces, so capacity of 16 is reasonable.
    pub fn with_capacity(capacity: usize) -> Self {
        NetworkBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds network statistics for an interface to the buffer
    pub fn push(&mut self, stats: NetworkStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the network statistics for all interfaces
    pub fn iter(&self) -> impl Iterator<Item = &NetworkStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of interfaces in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Returns a slice of all network statistics
    pub fn as_slice(&self) -> &[NetworkStats<'a>] {
        &self.stats
    }
}

/// The Network collector that reads from /proc/net/dev on Linux systems.
/// Collects interface-level network traffic and error statistics.
/// Supports optional filtering to monitor only specific interfaces.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
pub struct NetworkCollector {
    /// Optional list of interface names to include (if None, collects all interfaces)
    pub include_interfaces: Option<Vec<String>>,
}

/// Default trait implementation for NetworkCollector
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
impl Default for NetworkCollector {
    fn default() -> Self {
        NetworkCollector {
            include_interfaces: None,
        }
    }
}

/// Data producer implementation for Network collector.
/// Reads from /proc/net/dev and parses colon-delimited interface statistics.
/// File format: interface_name: RX_bytes RX_packets RX_errors RX_drops TX_bytes TX_packets TX_errors TX_drops
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for NetworkCollector {
    type Output = NetworkBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the /proc/net/dev file which contains per-interface statistics
        // Format: Inter-|   Receive                                                |  Transmit
        //         face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
        let content = tokio::fs::read_to_string("/proc/net/dev")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/net/dev".to_string(),
                source,
            })?;

        // Initialize buffer with typical capacity for network interfaces
        let mut buffer = NetworkBuffer::with_capacity(16);

        // Skip header lines (first 2 lines contain column headers)
        // Process each interface line starting from line 3
        for line in content.lines().skip(2) {
            // Split by colon to separate interface name from statistics
            let mut parts = line.split(':');

            // Extract interface name (before the colon)
            let iface_name = match parts.next() {
                Some(name) => name.trim(),
                None => continue,
            };

            // Check if this interface should be included based on filter
            if let Some(ref include) = self.include_interfaces {
                if !include.iter().any(|i| i == iface_name) {
                    continue;
                }
            }

            // Extract statistics values (after the colon)
            let values_str = match parts.next() {
                Some(vals) => vals,
                None => continue,
            };

            // Parse the 8 statistics values: RX_bytes, RX_packets, RX_errors, RX_drops,
            //                               TX_bytes, TX_packets, TX_errors, TX_drops
            let values: Result<Vec<u64>, _> = values_str
                .split_whitespace()
                .take(8)
                .map(|s| s.parse::<u64>())
                .collect();

            // Handle parsing errors with detailed information
            let values = values.map_err(|_| CollectorError::ParseError {
                metric: "network_stats".to_string(),
                location: format!("/proc/net/dev interface={}", iface_name),
                reason: "failed to parse network counters".to_string(),
            })?;

            // Ensure we have all 8 required values
            if values.len() >= 8 {
                // Create NetworkStats entry and add to buffer
                buffer.push(NetworkStats {
                    interface: Cow::Owned(iface_name.to_string()),
                    receive_bytes: values[0],
                    receive_packets: values[1],
                    receive_errors: values[2],
                    receive_drops: values[3],
                    transmit_bytes: values[4],
                    transmit_packets: values[5],
                    transmit_errors: values[6],
                    transmit_drops: values[7],
                });
            }
        }

        Ok(buffer)
    }
}

/// Constructor methods for NetworkCollector
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
impl NetworkCollector {
    /// Creates a new NetworkCollector with optional interface filter
    pub fn new(include_interfaces: Option<Vec<String>>) -> Self {
        NetworkCollector { include_interfaces }
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
register_collector!(NetworkCollector, "network");

/// Fallback implementations for unsupported platforms or disabled features

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats<'a> {
    pub interface: Cow<'a, str>,
    pub receive_bytes: u64,
    pub receive_packets: u64,
    pub receive_errors: u64,
    pub receive_drops: u64,
    pub transmit_bytes: u64,
    pub transmit_packets: u64,
    pub transmit_errors: u64,
    pub transmit_drops: u64,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkBuffer<'a> {
    stats: Vec<NetworkStats<'a>>,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
impl<'a> NetworkBuffer<'a> {
    pub fn new() -> Self {
        NetworkBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        NetworkBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stats: NetworkStats<'a>) {
        self.stats.push(stats);
    }

    pub fn iter(&self) -> impl Iterator<Item = &NetworkStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    pub fn as_slice(&self) -> &[NetworkStats<'a>] {
        &self.stats
    }
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
pub struct NetworkCollector {
    pub include_interfaces: Option<Vec<String>>,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
impl Default for NetworkCollector {
    fn default() -> Self {
        NetworkCollector {
            include_interfaces: None,
        }
    }
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
impl NetworkCollector {
    pub fn new(include_interfaces: Option<Vec<String>>) -> Self {
        NetworkCollector { include_interfaces }
    }
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for NetworkCollector {
    type Output = NetworkBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Network collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_stats_creation() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        assert_eq!(stats.interface, "eth0");
        assert_eq!(stats.receive_bytes, 1000000000);
        assert_eq!(stats.transmit_bytes, 800000000);
    }

    #[test]
    fn test_network_stats_ethernet_interface() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 5000000,
            receive_packets: 50000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 4000000,
            transmit_packets: 40000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert_eq!(stats.interface, "eth0");
    }

    #[test]
    fn test_network_stats_loopback_interface() {
        // Loopback typically has symmetric traffic
        let stats = NetworkStats {
            interface: Cow::Borrowed("lo"),
            receive_bytes: 100000000,
            receive_packets: 1000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 100000000,
            transmit_packets: 1000000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert_eq!(stats.interface, "lo");
        // Loopback should have equal RX and TX
        assert_eq!(stats.receive_bytes, stats.transmit_bytes);
    }

    #[test]
    fn test_network_stats_wifi_interface() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("wlan0"),
            receive_bytes: 2000000000,
            receive_packets: 20000000,
            receive_errors: 500,
            receive_drops: 200,
            transmit_bytes: 1500000000,
            transmit_packets: 15000000,
            transmit_errors: 300,
            transmit_drops: 100,
        };

        assert_eq!(stats.interface, "wlan0");
        // WiFi typically has higher error rates
        assert!(stats.receive_errors > 0);
    }

    #[test]
    fn test_network_stats_virtual_interface() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("veth12345"),
            receive_bytes: 50000000,
            receive_packets: 500000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 50000000,
            transmit_packets: 500000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert_eq!(stats.interface, "veth12345");
    }

    #[test]
    fn test_network_stats_docker_interface() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("docker0"),
            receive_bytes: 3000000000,
            receive_packets: 30000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 3000000000,
            transmit_packets: 30000000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert_eq!(stats.interface, "docker0");
    }

    #[test]
    fn test_network_stats_zero_traffic() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 0,
            receive_packets: 0,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 0,
            transmit_packets: 0,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert_eq!(stats.receive_bytes, 0);
        assert_eq!(stats.transmit_packets, 0);
    }

    #[test]
    fn test_network_stats_high_traffic() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000000,
            receive_packets: 10000000000,
            receive_errors: 1000,
            receive_drops: 500,
            transmit_bytes: 800000000000,
            transmit_packets: 8000000000,
            transmit_errors: 750,
            transmit_drops: 250,
        };

        assert!(stats.receive_bytes > 999999999999);
    }

    #[test]
    fn test_network_stats_with_errors() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 5000,
            receive_drops: 2000,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 3000,
            transmit_drops: 1000,
        };

        assert!(stats.receive_errors > 0);
        assert!(stats.transmit_errors > 0);
    }

    #[test]
    fn test_network_buffer_creation() {
        let buffer = NetworkBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_network_buffer_with_capacity() {
        let buffer = NetworkBuffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_network_buffer_push_single() {
        let mut buffer = NetworkBuffer::new();
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        buffer.push(stats);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_network_buffer_multiple_interfaces() {
        let mut buffer = NetworkBuffer::new();

        let interfaces = vec!["eth0", "eth1", "wlan0", "lo", "docker0"];
        for iface in interfaces {
            let stats = NetworkStats {
                interface: Cow::Owned(iface.to_string()),
                receive_bytes: 1000000,
                receive_packets: 10000,
                receive_errors: 0,
                receive_drops: 0,
                transmit_bytes: 800000,
                transmit_packets: 8000,
                transmit_errors: 0,
                transmit_drops: 0,
            };
            buffer.push(stats);
        }

        assert_eq!(buffer.len(), 5);
    }

    #[test]
    fn test_network_buffer_iterator() {
        let mut buffer = NetworkBuffer::new();

        for i in 0..3 {
            let stats = NetworkStats {
                interface: Cow::Owned(format!("eth{}", i)),
                receive_bytes: 1000000,
                receive_packets: 10000,
                receive_errors: 0,
                receive_drops: 0,
                transmit_bytes: 800000,
                transmit_packets: 8000,
                transmit_errors: 0,
                transmit_drops: 0,
            };
            buffer.push(stats);
        }

        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_network_stats_serialize() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        assert!(json.contains("eth0"));
        assert!(json.contains("1000000000"));
    }

    #[test]
    fn test_network_stats_deserialize() {
        let json = r#"{
            "interface": "eth0",
            "receive_bytes": 1000000000,
            "receive_packets": 10000000,
            "receive_errors": 100,
            "receive_drops": 50,
            "transmit_bytes": 800000000,
            "transmit_packets": 8000000,
            "transmit_errors": 75,
            "transmit_drops": 25
        }"#;

        let stats: NetworkStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(stats.interface, "eth0");
        assert_eq!(stats.receive_bytes, 1000000000);
    }

    #[test]
    fn test_network_collector_creation() {
        let collector = NetworkCollector::new(None);
        assert!(collector.include_interfaces.is_none());
    }

    #[test]
    fn test_network_collector_with_filters() {
        let interfaces = vec!["eth0".to_string(), "eth1".to_string()];
        let collector = NetworkCollector::new(Some(interfaces.clone()));

        assert!(collector.include_interfaces.is_some());
        assert_eq!(collector.include_interfaces.unwrap().len(), 2);
    }

    #[test]
    fn test_network_collector_default() {
        let collector = NetworkCollector::default();
        assert!(collector.include_interfaces.is_none());
    }

    #[test]
    fn test_network_buffer_as_slice() {
        let mut buffer = NetworkBuffer::new();

        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        buffer.push(stats);
        let slice = buffer.as_slice();

        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].interface, "eth0");
    }

    #[test]
    fn test_network_stats_clone() {
        let stats1 = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        let stats2 = stats1.clone();
        assert_eq!(stats1.receive_bytes, stats2.receive_bytes);
    }

    #[test]
    fn test_network_buffer_clone() {
        let mut buffer1 = NetworkBuffer::new();

        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        buffer1.push(stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
    }

    #[test]
    fn test_network_buffer_find_interface() {
        let mut buffer = NetworkBuffer::new();

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000,
            receive_packets: 10000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 800000,
            transmit_packets: 8000,
            transmit_errors: 0,
            transmit_drops: 0,
        });
        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 2000000,
            receive_packets: 20000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 1600000,
            transmit_packets: 16000,
            transmit_errors: 0,
            transmit_drops: 0,
        });

        let eth0 = buffer.iter().find(|s| s.interface == "eth0");
        assert!(eth0.is_some());
        assert_eq!(eth0.unwrap().receive_bytes, 1000000);
    }

    #[test]
    fn test_network_stats_asymmetric_traffic() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 500000000,
            transmit_packets: 5000000,
            transmit_errors: 50,
            transmit_drops: 25,
        };

        assert!(stats.receive_bytes > stats.transmit_bytes);
    }

    #[test]
    fn test_network_stats_interface_health() {
        let healthy = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 10,
            receive_drops: 5,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 5,
            transmit_drops: 2,
        };

        let unhealthy = NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 500000,
            receive_drops: 100000,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 300000,
            transmit_drops: 50000,
        };

        let healthy_error_rate =
            (healthy.receive_errors as f64 / healthy.receive_packets as f64) * 100.0;
        let unhealthy_error_rate =
            (unhealthy.receive_errors as f64 / unhealthy.receive_packets as f64) * 100.0;

        assert!(healthy_error_rate < unhealthy_error_rate);
    }

    #[test]
    fn test_network_stats_high_error_rate() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 500000,
            receive_drops: 100000,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 300000,
            transmit_drops: 50000,
        };

        let error_rate = ((stats.receive_errors + stats.receive_drops) as f64
            / stats.receive_packets as f64)
            * 100.0;
        assert!(error_rate > 5.0); // More than 5% error rate is unhealthy
    }

    #[test]
    fn test_network_stats_serialization_roundtrip() {
        let original = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: NetworkStats =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(original.receive_bytes, deserialized.receive_bytes);
    }

    #[test]
    fn test_network_buffer_total_traffic() {
        let mut buffer = NetworkBuffer::new();

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 0,
            transmit_drops: 0,
        });

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 500000000,
            receive_packets: 5000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 400000000,
            transmit_packets: 4000000,
            transmit_errors: 0,
            transmit_drops: 0,
        });

        let total_rx: u64 = buffer.iter().map(|s| s.receive_bytes).sum();
        assert_eq!(total_rx, 1500000000);
    }

    #[test]
    fn test_network_stats_maximum_u64() {
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: u64::MAX,
            receive_packets: u64::MAX,
            receive_errors: u64::MAX,
            receive_drops: u64::MAX,
            transmit_bytes: u64::MAX,
            transmit_packets: u64::MAX,
            transmit_errors: u64::MAX,
            transmit_drops: u64::MAX,
        };

        assert_eq!(stats.receive_bytes, u64::MAX);
    }

    #[test]
    fn test_network_stats_clone_independence() {
        let mut stats1 = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        };

        let stats2 = stats1.clone();

        stats1.receive_bytes = 2000000000;

        assert_eq!(stats1.receive_bytes, 2000000000);
        assert_eq!(stats2.receive_bytes, 1000000000);
    }

    #[test]
    fn test_network_buffer_aggregate_errors() {
        let mut buffer = NetworkBuffer::new();

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 75,
            transmit_drops: 25,
        });

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 500000000,
            receive_packets: 5000000,
            receive_errors: 50,
            receive_drops: 25,
            transmit_bytes: 400000000,
            transmit_packets: 4000000,
            transmit_errors: 30,
            transmit_drops: 10,
        });

        let total_errors: u64 = buffer
            .iter()
            .map(|s| s.receive_errors + s.transmit_errors)
            .sum();
        assert_eq!(total_errors, 255);
    }

    #[test]
    fn test_network_stats_loopback_traffic() {
        // Loopback should have symmetric and zero-error traffic
        let stats = NetworkStats {
            interface: Cow::Borrowed("lo"),
            receive_bytes: 100000000,
            receive_packets: 1000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 100000000,
            transmit_packets: 1000000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert_eq!(stats.receive_bytes, stats.transmit_bytes);
        assert_eq!(stats.receive_packets, stats.transmit_packets);
        assert_eq!(stats.receive_errors, 0);
    }

    #[test]
    fn test_network_stats_typical_server_traffic() {
        // Typical server: more incoming than outgoing
        let stats = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000000,
            receive_packets: 10000000000,
            receive_errors: 100,
            receive_drops: 50,
            transmit_bytes: 500000000000,
            transmit_packets: 5000000000,
            transmit_errors: 50,
            transmit_drops: 25,
        };

        assert!(stats.receive_bytes > stats.transmit_bytes);
        assert!(stats.receive_packets > stats.transmit_packets);
    }

    #[test]
    fn test_network_stats_interface_comparison() {
        let busy_eth0 = NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 10000000000,
            receive_packets: 100000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 8000000000,
            transmit_packets: 80000000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        let idle_eth1 = NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 100000000,
            receive_packets: 1000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 80000000,
            transmit_packets: 800000,
            transmit_errors: 0,
            transmit_drops: 0,
        };

        assert!(busy_eth0.receive_bytes > idle_eth1.receive_bytes);
    }

    #[test]
    fn test_network_buffer_find_busiest_interface() {
        let mut buffer = NetworkBuffer::new();

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth0"),
            receive_bytes: 1000000000,
            receive_packets: 10000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 800000000,
            transmit_packets: 8000000,
            transmit_errors: 0,
            transmit_drops: 0,
        });

        buffer.push(NetworkStats {
            interface: Cow::Borrowed("eth1"),
            receive_bytes: 10000000000,
            receive_packets: 100000000,
            receive_errors: 0,
            receive_drops: 0,
            transmit_bytes: 8000000000,
            transmit_packets: 80000000,
            transmit_errors: 0,
            transmit_drops: 0,
        });

        let busiest = buffer
            .iter()
            .max_by_key(|s| s.receive_bytes + s.transmit_bytes)
            .unwrap();
        assert_eq!(busiest.interface, "eth1");
    }
}
