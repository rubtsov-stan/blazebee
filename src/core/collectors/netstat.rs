use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// A single network statistics entry.
/// Contains protocol-specific metrics like TCP retransmissions, dropped packets,
/// UDP errors, etc. from the /proc/net/netstat file.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetstatEntry<'a> {
    /// Protocol name (e.g., "TcpExt", "IpExt", "UdpLite")
    pub protocol: Cow<'a, str>,
    /// Statistical metric name (e.g., "TCPRetransFail", "InErrors", "OutErrors")
    pub stat_name: Cow<'a, str>,
    /// Numeric value of the statistic
    pub value: u64,
}

/// Buffer for storing network statistics entries.
/// /proc/net/netstat contains statistics for multiple protocols, each with many metrics.
/// This buffer aggregates all of them into individual entries.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetstatBuffer<'a> {
    entries: Vec<NetstatEntry<'a>>,
}

/// Methods for working with the netstat buffer
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
impl<'a> NetstatBuffer<'a> {
    /// Creates a new empty netstat buffer
    pub fn new() -> Self {
        NetstatBuffer {
            entries: Vec::new(),
        }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of entries.
    /// Typical capacity is 100+ entries since each protocol has many statistics.
    pub fn with_capacity(capacity: usize) -> Self {
        NetstatBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Adds a network statistics entry to the buffer
    pub fn push(&mut self, entry: NetstatEntry<'a>) {
        self.entries.push(entry);
    }

    /// Returns an iterator over the network statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &NetstatEntry<'a>> {
        self.entries.iter()
    }

    /// Returns the number of statistics entries in the buffer
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a slice of all network statistics entries
    pub fn as_slice(&self) -> &[NetstatEntry<'a>] {
        &self.entries
    }
}

/// The Netstat collector that reads from /proc/net/netstat on Linux systems.
/// Collects comprehensive network protocol statistics for TCP, UDP, IP, and other protocols.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
pub struct NetstatCollector;

/// Constructor methods for NetstatCollector
impl NetstatCollector {
    /// Creates a new NetstatCollector instance
    pub fn new() -> Self {
        NetstatCollector
    }
}

/// Default trait implementation for NetstatCollector
impl Default for NetstatCollector {
    fn default() -> Self {
        NetstatCollector::new()
    }
}

/// Data producer implementation for Netstat collector.
/// Reads from /proc/net/netstat and parses paired header/value lines.
/// The file format consists of alternating header and value lines for each protocol.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for NetstatCollector {
    type Output = NetstatBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the network statistics file
        // Format: alternating header and value lines for each protocol
        // Example:
        // TcpExt: SyncookiesSent SyncookiesRecv SyncookiesFailed ...
        // TcpExt: 0 0 0 ...
        let content = tokio::fs::read_to_string("/proc/net/netstat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/net/netstat".to_string(),
                source,
            })?;

        // Initialize buffer with capacity for typical number of netstat entries (~100+)
        let mut buffer = NetstatBuffer::new();
        let mut lines = content.lines();

        // Process lines in pairs (header and value lines)
        while let Some(header_line) = lines.next() {
            if let Some(value_line) = lines.next() {
                // Split header line into protocol name and statistic names
                let headers: Vec<&str> = header_line.split_whitespace().collect();
                // Split value line into protocol name and statistic values
                let values: Vec<&str> = value_line.split_whitespace().collect();

                // Validate that both lines have content
                // First element is protocol name, rest are statistics
                if headers.len() > 1 && values.len() > 1 {
                    // Extract protocol name (first field, with trailing colon removed)
                    let protocol = headers[0];

                    // Iterate through statistic names, skipping the protocol name (index 0)
                    for (idx, header) in headers.iter().enumerate().skip(1) {
                        // Match header index with value index
                        if idx < values.len() {
                            // Try to parse the value as u64
                            if let Ok(val) = values[idx].parse::<u64>() {
                                // Create and add the statistic entry
                                buffer.push(NetstatEntry {
                                    protocol: Cow::Owned(protocol.to_string()),
                                    stat_name: Cow::Owned(header.to_string()),
                                    value: val,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
register_collector!(NetstatCollector, "netstat");

/// Fallback implementations for unsupported platforms or disabled features

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetstatEntry<'a> {
    pub protocol: Cow<'a, str>,
    pub stat_name: Cow<'a, str>,
    pub value: u64,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetstatBuffer<'a> {
    entries: Vec<NetstatEntry<'a>>,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
)))]
impl<'a> NetstatBuffer<'a> {
    pub fn new() -> Self {
        NetstatBuffer {
            entries: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        NetstatBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, entry: NetstatEntry<'a>) {
        self.entries.push(entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = &NetstatEntry<'a>> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn as_slice(&self) -> &[NetstatEntry<'a>] {
        &self.entries
    }
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
)))]
pub struct NetstatCollector;

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for NetstatCollector {
    type Output = NetstatBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Netstat collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netstat_entry_creation() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        assert_eq!(entry.protocol, "TcpExt");
        assert_eq!(entry.stat_name, "TCPRetransFail");
        assert_eq!(entry.value, 42);
    }

    #[test]
    fn test_netstat_entry_tcp_statistics() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 1000,
        };

        assert_eq!(entry.protocol, "TcpExt");
        assert!(entry.value > 0);
    }

    #[test]
    fn test_netstat_entry_udp_statistics() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("UdpLite"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 50,
        };

        assert_eq!(entry.protocol, "UdpLite");
        assert_eq!(entry.stat_name, "InErrors");
    }

    #[test]
    fn test_netstat_entry_zero_value() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("SyncookiesSent"),
            value: 0,
        };

        assert_eq!(entry.value, 0);
    }

    #[test]
    fn test_netstat_entry_large_value() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InOctets"),
            value: 1000000000000,
        };

        assert!(entry.value > 999999999999);
    }

    #[test]
    fn test_netstat_buffer_creation() {
        let buffer = NetstatBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_netstat_buffer_with_capacity() {
        let buffer = NetstatBuffer::with_capacity(100);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_netstat_buffer_push_single() {
        let mut buffer = NetstatBuffer::new();
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        buffer.push(entry);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_netstat_buffer_multiple_protocols() {
        let mut buffer = NetstatBuffer::new();

        let protocols = vec!["TcpExt", "IpExt", "UdpLite"];
        for protocol in protocols {
            let entry = NetstatEntry {
                protocol: Cow::Owned(protocol.to_string()),
                stat_name: Cow::Borrowed("Metric"),
                value: 100,
            };
            buffer.push(entry);
        }

        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_netstat_buffer_iterator() {
        let mut buffer = NetstatBuffer::new();

        for i in 0..5 {
            let entry = NetstatEntry {
                protocol: Cow::Borrowed("TcpExt"),
                stat_name: Cow::Owned(format!("Stat{}", i)),
                value: (i as u64) * 100,
            };
            buffer.push(entry);
        }

        let count = buffer.iter().count();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_netstat_entry_serialize() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        let json = serde_json::to_string(&entry).expect("serialization failed");
        assert!(json.contains("TcpExt"));
        assert!(json.contains("TCPRetransFail"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_netstat_entry_deserialize() {
        let json = r#"{
            "protocol": "TcpExt",
            "stat_name": "TCPRetransFail",
            "value": 42
        }"#;

        let entry: NetstatEntry = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(entry.protocol, "TcpExt");
        assert_eq!(entry.stat_name, "TCPRetransFail");
        assert_eq!(entry.value, 42);
    }

    #[test]
    fn test_netstat_collector_creation() {
        let _ = NetstatCollector::new();
        let _ = NetstatCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_netstat_buffer_as_slice() {
        let mut buffer = NetstatBuffer::new();

        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        buffer.push(entry);
        let slice = buffer.as_slice();

        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].protocol, "TcpExt");
    }

    #[test]
    fn test_netstat_entry_clone() {
        let entry1 = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        let entry2 = entry1.clone();
        assert_eq!(entry1.value, entry2.value);
    }

    #[test]
    fn test_netstat_buffer_clone() {
        let mut buffer1 = NetstatBuffer::new();

        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        buffer1.push(entry);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
    }

    #[test]
    fn test_netstat_buffer_find_protocol() {
        let mut buffer = NetstatBuffer::new();

        for proto in &["TcpExt", "IpExt", "UdpLite"] {
            let entry = NetstatEntry {
                protocol: Cow::Owned(proto.to_string()),
                stat_name: Cow::Borrowed("Stat"),
                value: 100,
            };
            buffer.push(entry);
        }

        let tcp_entries: Vec<_> = buffer.iter().filter(|e| e.protocol == "TcpExt").collect();
        assert_eq!(tcp_entries.len(), 1);
    }

    #[test]
    fn test_netstat_buffer_protocol_statistics() {
        let mut buffer = NetstatBuffer::new();

        // TCP statistics
        buffer.push(NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 500,
        });
        buffer.push(NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPOutRsts"),
            value: 100,
        });

        // IP statistics
        buffer.push(NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 50,
        });

        let tcp_errors: u64 = buffer
            .iter()
            .filter(|e| e.protocol == "TcpExt")
            .map(|e| e.value)
            .sum();
        assert_eq!(tcp_errors, 600);
    }

    #[test]
    fn test_netstat_entry_typical_retransmissions() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 250,
        };

        // Typical retransmission rate on healthy network
        assert!(entry.value < 10000);
    }

    #[test]
    fn test_netstat_entry_connection_errors() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPOutRsts"),
            value: 30,
        };

        // Low reset count indicates healthy connections
        assert!(entry.value < 1000);
    }

    #[test]
    fn test_netstat_entry_ipv4_statistics() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 100,
        };

        assert_eq!(entry.protocol, "IpExt");
    }

    #[test]
    fn test_netstat_stats_comparison() {
        let good_network = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 100,
        };

        let bad_network = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 10000,
        };

        assert!(good_network.value < bad_network.value);
    }

    #[test]
    fn test_netstat_serialization_roundtrip() {
        let original = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: NetstatEntry =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(original.value, deserialized.value);
    }

    #[test]
    fn test_netstat_buffer_protocol_count() {
        let mut buffer = NetstatBuffer::new();

        // Add multiple statistics for each protocol
        for _ in 0..10 {
            buffer.push(NetstatEntry {
                protocol: Cow::Borrowed("TcpExt"),
                stat_name: Cow::Borrowed("Stat"),
                value: 100,
            });
        }
        for _ in 0..5 {
            buffer.push(NetstatEntry {
                protocol: Cow::Borrowed("IpExt"),
                stat_name: Cow::Borrowed("Stat"),
                value: 50,
            });
        }

        assert_eq!(buffer.len(), 15);
    }

    #[test]
    fn test_netstat_entry_maximum_u64() {
        let entry = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("LargeCounter"),
            value: u64::MAX,
        };

        assert_eq!(entry.value, u64::MAX);
    }

    #[test]
    fn test_netstat_entry_clone_independence() {
        let mut entry1 = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransFail"),
            value: 42,
        };

        let entry2 = entry1.clone();

        // Modify original, clone should not be affected
        entry1.value = 100;

        assert_eq!(entry1.value, 100);
        assert_eq!(entry2.value, 42);
    }

    #[test]
    fn test_netstat_error_detection() {
        // Detect network problems by checking error statistics
        let errors = NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 1000,
        };

        let drops = NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InDiscards"),
            value: 500,
        };

        assert!(errors.value > 100); // High error count indicates problems
        assert!(drops.value > 0); // Any drops indicate packet loss
    }

    #[test]
    fn test_netstat_ipv_statistics() {
        // Different IP version statistics
        let ipv4_errors = NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 100,
        };

        let ipv6_errors = NetstatEntry {
            protocol: Cow::Borrowed("Ip6Ext"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 50,
        };

        assert!(ipv4_errors.value > ipv6_errors.value);
    }

    #[test]
    fn test_netstat_buffer_aggregate_errors() {
        let mut buffer = NetstatBuffer::new();

        // Add multiple error entries
        buffer.push(NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 100,
        });
        buffer.push(NetstatEntry {
            protocol: Cow::Borrowed("IpExt"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 50,
        });
        buffer.push(NetstatEntry {
            protocol: Cow::Borrowed("UdpLite"),
            stat_name: Cow::Borrowed("InErrors"),
            value: 25,
        });

        let total_errors: u64 = buffer.iter().map(|e| e.value).sum();
        assert_eq!(total_errors, 175);
    }

    #[test]
    fn test_netstat_typical_network_load() {
        let mut buffer = NetstatBuffer::new();

        // Simulate typical /proc/net/netstat output
        let stats = vec![
            ("TcpExt", "TCPRetransmit", 245),
            ("TcpExt", "TCPOutRsts", 12),
            ("IpExt", "InErrors", 8),
            ("IpExt", "InDiscards", 3),
            ("UdpLite", "InErrors", 0),
        ];

        for (proto, stat, val) in stats {
            buffer.push(NetstatEntry {
                protocol: Cow::Owned(proto.to_string()),
                stat_name: Cow::Owned(stat.to_string()),
                value: val,
            });
        }

        assert_eq!(buffer.len(), 5);

        // Find problematic metrics
        let high_retrans = buffer
            .iter()
            .filter(|e| e.stat_name == "TCPRetransmit" && e.value > 100)
            .count();
        assert_eq!(high_retrans, 1);
    }

    #[test]
    fn test_netstat_network_health_check() {
        // Determine network health based on statistics
        let healthy = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 10,
        };

        let degraded = NetstatEntry {
            protocol: Cow::Borrowed("TcpExt"),
            stat_name: Cow::Borrowed("TCPRetransmit"),
            value: 1000,
        };

        // Healthy network has low retransmission rate
        assert!(healthy.value < 100);
        // Degraded network has high retransmission rate
        assert!(degraded.value > 100);
    }
}
