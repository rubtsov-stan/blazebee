use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// A single entry in the ARP (Address Resolution Protocol) table.
/// Contains information about the mapping between an IP address and a MAC address.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArpEntry<'a> {
    /// IP address of the device
    pub ip_address: Cow<'a, str>,
    /// Hardware interface type (typically "ether" for Ethernet)
    pub hw_type: Cow<'a, str>,
    /// Flags indicating the state of the ARP entry
    pub flags: Cow<'a, str>,
    /// MAC address (hardware address)
    pub hw_address: Cow<'a, str>,
    /// Mask field (typically unused, empty)
    pub mask: Cow<'a, str>,
    /// Network interface through which this ARP mapping is accessible
    pub device: Cow<'a, str>,
}

/// Buffer for storing ARP table entries.
/// Uses Vec internally for efficient storage of multiple entries.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArpBuffer<'a> {
    entries: Vec<ArpEntry<'a>>,
}

/// Methods for working with the ARP buffer
impl<'a> ArpBuffer<'a> {
    /// Creates a new empty ARP buffer
    pub fn new() -> Self {
        ArpBuffer {
            entries: Vec::new(),
        }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of entries
    /// This optimizes memory allocation when the number of entries is known in advance
    pub fn with_capacity(capacity: usize) -> Self {
        ArpBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Adds an ARP entry to the buffer
    pub fn push(&mut self, entry: ArpEntry<'a>) {
        self.entries.push(entry);
    }

    /// Returns an iterator over the ARP entries
    pub fn iter(&self) -> impl Iterator<Item = &ArpEntry<'a>> {
        self.entries.iter()
    }

    /// Returns the number of ARP entries in the buffer
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a slice of all ARP entries
    pub fn as_slice(&self) -> &[ArpEntry<'a>] {
        &self.entries
    }
}

/// The ARP collector that reads from /proc/net/arp on Linux systems.
/// Collects Address Resolution Protocol table information.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
pub struct ArpCollector;

/// Constructor methods for ArpCollector
impl ArpCollector {
    /// Creates a new ARP collector instance
    pub fn new() -> Self {
        ArpCollector
    }
}

/// Default trait implementation for ArpCollector
impl Default for ArpCollector {
    fn default() -> Self {
        ArpCollector::new()
    }
}

/// Data producer implementation for ARP collector.
/// Reads from /proc/net/arp and parses ARP table entries.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for ArpCollector {
    type Output = ArpBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the ARP table from /proc/net/arp
        let content = tokio::fs::read_to_string("/proc/net/arp")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/net/arp".to_string(),
                source,
            })?;

        // Initialize buffer with capacity for 64 entries
        let mut buffer = ArpBuffer::with_capacity(64);

        // Skip the header line and parse each ARP entry
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Each ARP entry must have at least 6 fields
            if parts.len() < 6 {
                continue;
            }

            // Create an ARP entry from the parsed fields
            buffer.push(ArpEntry {
                ip_address: Cow::Owned(parts[0].to_string()),
                hw_type: Cow::Owned(parts[1].to_string()),
                flags: Cow::Owned(parts[2].to_string()),
                hw_address: Cow::Owned(parts[3].to_string()),
                mask: Cow::Owned(parts[4].to_string()),
                device: Cow::Owned(parts[5].to_string()),
            });
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
register_collector!(ArpCollector, "arp");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
)))]
pub struct ArpCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for ArpCollector {
    type Output = ();

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "ARP collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arp_entry_creation() {
        let entry = ArpEntry {
            ip_address: Cow::Borrowed("192.168.1.1"),
            hw_type: Cow::Borrowed("ether"),
            flags: Cow::Borrowed("0x2"),
            hw_address: Cow::Borrowed("aa:bb:cc:dd:ee:ff"),
            mask: Cow::Borrowed("*"),
            device: Cow::Borrowed("eth0"),
        };

        assert_eq!(entry.ip_address, "192.168.1.1");
        assert_eq!(entry.hw_type, "ether");
        assert_eq!(entry.hw_address, "aa:bb:cc:dd:ee:ff");
        assert_eq!(entry.device, "eth0");
    }

    #[test]
    fn test_arp_buffer_creation() {
        let buffer = ArpBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_arp_buffer_push() {
        let mut buffer = ArpBuffer::new();
        let entry = ArpEntry {
            ip_address: Cow::Borrowed("192.168.1.100"),
            hw_type: Cow::Borrowed("ether"),
            flags: Cow::Borrowed("0x2"),
            hw_address: Cow::Borrowed("11:22:33:44:55:66"),
            mask: Cow::Borrowed("*"),
            device: Cow::Borrowed("eth0"),
        };

        buffer.push(entry);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_arp_buffer_with_capacity() {
        let buffer = ArpBuffer::with_capacity(100);
        assert_eq!(buffer.len(), 0);
        // The buffer has capacity but no entries
    }

    #[test]
    fn test_arp_buffer_multiple_entries() {
        let mut buffer = ArpBuffer::with_capacity(10);

        for i in 0..5 {
            let entry = ArpEntry {
                ip_address: Cow::Owned(format!("192.168.1.{}", i)),
                hw_type: Cow::Borrowed("ether"),
                flags: Cow::Borrowed("0x2"),
                hw_address: Cow::Owned(format!("aa:bb:cc:dd:ee:{:02x}", i)),
                mask: Cow::Borrowed("*"),
                device: Cow::Borrowed("eth0"),
            };
            buffer.push(entry);
        }

        assert_eq!(buffer.len(), 5);
        let slice = buffer.as_slice();
        assert_eq!(slice.len(), 5);
    }

    #[test]
    fn test_arp_buffer_iterator() {
        let mut buffer = ArpBuffer::new();

        let entry1 = ArpEntry {
            ip_address: Cow::Borrowed("192.168.1.1"),
            hw_type: Cow::Borrowed("ether"),
            flags: Cow::Borrowed("0x2"),
            hw_address: Cow::Borrowed("aa:bb:cc:dd:ee:01"),
            mask: Cow::Borrowed("*"),
            device: Cow::Borrowed("eth0"),
        };

        let entry2 = ArpEntry {
            ip_address: Cow::Borrowed("192.168.1.2"),
            hw_type: Cow::Borrowed("ether"),
            flags: Cow::Borrowed("0x2"),
            hw_address: Cow::Borrowed("aa:bb:cc:dd:ee:02"),
            mask: Cow::Borrowed("*"),
            device: Cow::Borrowed("eth0"),
        };

        buffer.push(entry1);
        buffer.push(entry2);

        let count = buffer.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_arp_collector_creation() {
        let _ = ArpCollector::new();
        let _default_collector = ArpCollector::default();
        // Both should create valid instances
    }

    #[test]
    fn test_arp_entry_serialize() {
        let entry = ArpEntry {
            ip_address: Cow::Borrowed("192.168.1.1"),
            hw_type: Cow::Borrowed("ether"),
            flags: Cow::Borrowed("0x2"),
            hw_address: Cow::Borrowed("aa:bb:cc:dd:ee:ff"),
            mask: Cow::Borrowed("*"),
            device: Cow::Borrowed("eth0"),
        };

        let json = serde_json::to_string(&entry).expect("serialization failed");
        assert!(json.contains("192.168.1.1"));
        assert!(json.contains("ether"));
        assert!(json.contains("eth0"));
    }
}
