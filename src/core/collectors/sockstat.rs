use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use tracing::trace;

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Represents a single network socket statistic entry for a specific protocol.
/// Each entry captures a named metric (like "inuse", "orphan", "timewait") for a protocol
/// (like "TCP", "UDP", "RAW"). The data comes from the kernel's network stack statistics.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SockstatEntry<'a> {
    /// The protocol name (e.g., "TCP", "UDP", "RAW", "UNIX").
    /// Identifies which network protocol or socket family this metric applies to.
    pub protocol: Cow<'a, str>,
    /// The metric name for the protocol (e.g., "inuse", "orphan", "tw", "sockets").
    /// Different protocols support different metrics:
    /// - TCP: inuse, orphan, tw (timewait), alloc, mem
    /// - UDP: inuse, mem
    /// - RAW: inuse
    /// - UNIX: inuse, orphan, sockets
    pub metric: Cow<'a, str>,
    /// The numeric value of the metric. Typically represents counts of sockets
    /// in various states, or memory usage in pages.
    pub value: u64,
}

/// Container for network socket statistics from all protocols.
/// This buffer collects sockstat data entries and provides convenient access
/// through standard collection methods. Data comes from /proc/net/sockstat.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SockstatBuffer<'a> {
    entries: Vec<SockstatEntry<'a>>,
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
impl<'a> SockstatBuffer<'a> {
    /// Creates an empty sockstat buffer with no pre-allocated capacity.
    pub fn new() -> Self {
        SockstatBuffer {
            entries: Vec::new(),
        }
    }

    /// Creates an empty sockstat buffer with pre-allocated capacity.
    /// Useful when the expected number of entries is known in advance.
    pub fn with_capacity(capacity: usize) -> Self {
        SockstatBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Appends a socket statistic entry to the buffer.
    pub fn push(&mut self, entry: SockstatEntry<'a>) {
        self.entries.push(entry);
    }

    /// Returns an iterator over all socket statistic entries in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &SockstatEntry<'a>> {
        self.entries.iter()
    }

    /// Returns the number of entries currently in the buffer.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Checks if the buffer contains any entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the entries as a slice for direct array access.
    pub fn as_slice(&self) -> &[SockstatEntry<'a>] {
        &self.entries
    }
}

/// The main collector responsible for gathering network socket statistics from the kernel.
/// Reads /proc/net/sockstat which exposes kernel statistics about sockets for various
/// network protocols. Useful for monitoring network resource usage and detecting socket leaks.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
pub struct SockstatCollector;

impl SockstatCollector {
    pub fn new() -> Self {
        SockstatCollector
    }
}

impl Default for SockstatCollector {
    fn default() -> Self {
        SockstatCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting network socket statistics.
/// Reads from /proc/net/sockstat and parses key-value pairs for each protocol. The parser
/// handles variable-length metrics per protocol and gracefully ignores unparseable values.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for SockstatCollector {
    type Output = SockstatBuffer<'static>;

    /// Asynchronously reads network socket statistics from /proc/net/sockstat.
    ///
    /// The file format contains lines like:
    /// ```text
    /// TCP: inuse 42 orphan 0 tw 5 alloc 123 mem 456
    /// UDP: inuse 10 mem 20
    /// RAW: inuse 0
    /// UNIX: inuse 100 orphan 2 sockets 200
    /// ```
    ///
    /// Each line starts with a protocol name followed by colon, then alternating
    /// metric names and values. The parser processes metrics as key-value pairs:
    /// odd positions (1, 3, 5, ...) are metric names, even positions (2, 4, 6, ...)
    /// are numeric values.
    ///
    /// Returns an error if the file cannot be read. Parse errors on individual
    /// metrics are silently ignored to maintain robustness across kernel versions.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/net/sockstat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/net/sockstat".to_string(),
                source,
            })?;

        let mut buffer = SockstatBuffer::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<String> = trimmed.split_whitespace().map(|s| s.to_string()).collect();
            if parts.len() < 3 {
                trace!("Skipping short line in sockstat: {}", trimmed);
                continue;
            }

            let protocol = parts[0].trim_end_matches(':').to_string();

            for chunk in parts[1..].chunks(2) {
                if chunk.len() != 2 {
                    trace!("Incomplete pair in sockstat line: {:?}", chunk);
                    continue;
                }

                let metric = chunk[0].clone();
                let value_str = &chunk[1];

                match value_str.parse::<u64>() {
                    Ok(value) => {
                        buffer.push(SockstatEntry {
                            protocol: Cow::Owned(protocol.clone()),
                            metric: Cow::Owned(metric),
                            value,
                        });
                    }
                    Err(e) => {
                        trace!(
                            "Failed to parse value '{}' for metric '{}' in protocol '{}': {}",
                            value_str,
                            metric,
                            protocol,
                            e
                        );
                    }
                }
            }
        }

        trace!("Collected {} sockstat entries", buffer.len());
        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
register_collector!(SockstatCollector, "sockstat");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SockstatEntry<'a> {
    pub protocol: Cow<'a, str>,
    pub metric: Cow<'a, str>,
    pub value: u64,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SockstatBuffer<'a> {
    entries: Vec<SockstatEntry<'a>>,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
)))]
impl<'a> SockstatBuffer<'a> {
    pub fn new() -> Self {
        SockstatBuffer {
            entries: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        SockstatBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, entry: SockstatEntry<'a>) {
        self.entries.push(entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = &SockstatEntry<'a>> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn as_slice(&self) -> &[SockstatEntry<'a>] {
        &self.entries
    }
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that socket statistics are not available on this platform.
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
)))]
pub struct SockstatCollector;

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for SockstatCollector {
    type Output = SockstatBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Sockstat collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    fn parse_sockstat_content(content: &str) -> CollectorResult<SockstatBuffer<'static>> {
        let mut buffer = SockstatBuffer::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let protocol = parts[0].trim_end_matches(':');
                for chunk in parts[1..].chunks(2) {
                    if chunk.len() == 2 {
                        if let Ok(val) = chunk[1].parse::<u64>() {
                            buffer.push(SockstatEntry {
                                protocol: Cow::Owned(protocol.to_string()),
                                metric: Cow::Owned(chunk[0].to_string()),
                                value: val,
                            });
                        }
                    }
                }
            }
        }

        Ok(buffer)
    }

    #[test]
    fn test_parse_single_protocol() {
        let content = "TCP: inuse 100 orphan 5 tw 10";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 3);

        let entries = buffer.as_slice();
        assert_eq!(entries[0].protocol, "TCP");
        assert_eq!(entries[0].metric, "inuse");
        assert_eq!(entries[0].value, 100);

        assert_eq!(entries[1].protocol, "TCP");
        assert_eq!(entries[1].metric, "orphan");
        assert_eq!(entries[1].value, 5);

        assert_eq!(entries[2].protocol, "TCP");
        assert_eq!(entries[2].metric, "tw");
        assert_eq!(entries[2].value, 10);
    }

    #[test]
    fn test_parse_zero_values() {
        let content = "TCP: inuse 0 orphan 0 tw 0 alloc 0 mem 0";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let entries = buffer.as_slice();

        for entry in entries {
            if entry.protocol == "TCP" {
                assert_eq!(entry.value, 0);
            }
        }
    }

    #[test]
    fn test_parse_high_values() {
        // Test with very large socket counts
        let content = "TCP: inuse 18446744073709551615 orphan 1000000000";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let entries = buffer.as_slice();
        assert_eq!(entries[0].value, u64::MAX);
    }

    #[test]
    fn test_parse_odd_number_of_metrics() {
        // Line with odd number of items after protocol (incomplete pair at end)
        let content = "TCP: inuse 42 orphan 5 tw";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // Only 2 entries should be parsed (inuse 42, orphan 5)
        // The incomplete pair (tw) is skipped
        assert_eq!(buffer.len(), 2);

        let entries = buffer.as_slice();
        assert_eq!(entries[0].metric, "inuse");
        assert_eq!(entries[0].value, 42);
        assert_eq!(entries[1].metric, "orphan");
        assert_eq!(entries[1].value, 5);
    }

    #[test]
    fn test_parse_invalid_value_skipped() {
        // Non-numeric value is skipped, but parsing continues with valid entries
        let content = "TCP: inuse 42 orphan invalid_value tw 10";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // Only 2 entries: inuse 42 and tw 10
        // The invalid orphan entry is skipped
        assert_eq!(buffer.len(), 2);

        let entries = buffer.as_slice();
        assert_eq!(entries[0].metric, "inuse");
        assert_eq!(entries[1].metric, "tw");
    }

    #[test]
    fn test_parse_protocol_name_extraction() {
        // Verify protocol names are correctly extracted without the colon
        let content = r#"TCP: inuse 1
UDP: inuse 2
RAW: inuse 3
UNIX: inuse 4"#;

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let entries = buffer.as_slice();

        assert_eq!(entries[0].protocol, "TCP");
        assert_eq!(entries[1].protocol, "UDP");
        assert_eq!(entries[2].protocol, "RAW");
        assert_eq!(entries[3].protocol, "UNIX");
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        // Test robustness with variable whitespace
        let content = "TCP:  inuse  42  orphan  5   tw   10";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 3);

        let entries = buffer.as_slice();
        assert_eq!(entries[0].value, 42);
        assert_eq!(entries[1].value, 5);
        assert_eq!(entries[2].value, 10);
    }

    #[test]
    fn test_parse_minimal_line() {
        // Minimal valid line (protocol + one metric-value pair)
        let content = "TCP: inuse 1";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 1);

        let entry = &buffer.as_slice()[0];
        assert_eq!(entry.protocol, "TCP");
        assert_eq!(entry.metric, "inuse");
        assert_eq!(entry.value, 1);
    }

    #[test]
    fn test_parse_line_with_only_protocol() {
        // Line with only protocol, no metrics (should be skipped)
        let content = r#"TCP:
UDP: inuse 5"#;

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // Only UDP entry should be parsed
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.as_slice()[0].protocol, "UDP");
    }

    #[test]
    fn test_parse_realistic_system_load() {
        // Realistic output from a loaded system
        let content = r#"TCP: inuse 1234 orphan 45 tw 567 alloc 2000 mem 8912
UDP: inuse 89 mem 150
RAW: inuse 2
UNIX: inuse 456 orphan 12 sockets 1200"#;

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let entries = buffer.as_slice();

        // TCP should have 5 metrics
        let tcp_count = entries.iter().filter(|e| e.protocol == "TCP").count();
        assert_eq!(tcp_count, 5);

        // Verify total TCP sockets
        let tcp_inuse = entries
            .iter()
            .find(|e| e.protocol == "TCP" && e.metric == "inuse")
            .map(|e| e.value);
        assert_eq!(tcp_inuse, Some(1234));
    }

    #[test]
    fn test_sockstat_buffer_operations() {
        let mut buffer = SockstatBuffer::new();

        let entry1 = SockstatEntry {
            protocol: Cow::Borrowed("TCP"),
            metric: Cow::Borrowed("inuse"),
            value: 100,
        };

        let entry2 = SockstatEntry {
            protocol: Cow::Borrowed("UDP"),
            metric: Cow::Borrowed("inuse"),
            value: 50,
        };

        buffer.push(entry1);
        buffer.push(entry2);

        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());

        let entries: Vec<_> = buffer.iter().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].protocol, "TCP");
        assert_eq!(entries[1].protocol, "UDP");

        let slice = buffer.as_slice();
        assert_eq!(slice[0].value, 100);
        assert_eq!(slice[1].value, 50);
    }

    #[test]
    fn test_sockstat_buffer_with_capacity() {
        let buffer = SockstatBuffer::with_capacity(100);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_sockstat_entry_serialization() {
        let entry = SockstatEntry {
            protocol: Cow::Borrowed("TCP"),
            metric: Cow::Borrowed("inuse"),
            value: 42,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&entry);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<SockstatEntry, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.protocol, "TCP");
        assert_eq!(restored.metric, "inuse");
        assert_eq!(restored.value, 42);
    }

    #[test]
    fn test_sockstat_entry_clone() {
        let original = SockstatEntry {
            protocol: Cow::Owned("UNIX".to_string()),
            metric: Cow::Owned("orphan".to_string()),
            value: 12345,
        };

        let cloned = original.clone();

        assert_eq!(original.protocol, cloned.protocol);
        assert_eq!(original.metric, cloned.metric);
        assert_eq!(original.value, cloned.value);
    }

    #[test]
    fn test_sockstat_collector_creation() {
        let collector = SockstatCollector::new();
        let default_collector = SockstatCollector::default();

        // Both should create valid instances without panicking
        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_parse_many_metrics_per_protocol() {
        // Protocol with many metrics
        let content = "NETLINK: inuse 5 orphan 0 sockets 10 mem 20 groups 15";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 5);

        let entries = buffer.as_slice();
        let metrics: Vec<_> = entries.iter().map(|e| e.metric.as_ref()).collect();
        assert_eq!(metrics, vec!["inuse", "orphan", "sockets", "mem", "groups"]);
    }

    #[test]
    fn test_parse_negative_values_unsigned() {
        // u64 cannot parse negative numbers
        let content = "TCP: inuse -100 orphan 5";

        let result = parse_sockstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // Only orphan 5 should be parsed; inuse -100 fails to parse
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.as_slice()[0].metric, "orphan");
    }

    #[test]
    fn test_sockstat_buffer_filtering() {
        let mut buffer = SockstatBuffer::new();

        buffer.push(SockstatEntry {
            protocol: Cow::Borrowed("TCP"),
            metric: Cow::Borrowed("inuse"),
            value: 100,
        });

        buffer.push(SockstatEntry {
            protocol: Cow::Borrowed("TCP"),
            metric: Cow::Borrowed("orphan"),
            value: 5,
        });

        buffer.push(SockstatEntry {
            protocol: Cow::Borrowed("UDP"),
            metric: Cow::Borrowed("inuse"),
            value: 50,
        });

        // Filter TCP entries
        let tcp_entries: Vec<_> = buffer.iter().filter(|e| e.protocol == "TCP").collect();
        assert_eq!(tcp_entries.len(), 2);

        // Filter by metric
        let inuse_entries: Vec<_> = buffer.iter().filter(|e| e.metric == "inuse").collect();
        assert_eq!(inuse_entries.len(), 2);
    }

    #[test]
    fn test_sockstat_entry_debug_format() {
        let entry = SockstatEntry {
            protocol: Cow::Borrowed("TCP"),
            metric: Cow::Borrowed("inuse"),
            value: 42,
        };

        let debug_string = format!("{:?}", entry);
        assert!(debug_string.contains("protocol"));
        assert!(debug_string.contains("metric"));
        assert!(debug_string.contains("TCP"));
        assert!(debug_string.contains("inuse"));
    }

    #[test]
    fn test_parse_socket_leak_detection() {
        // Scenario: Detecting socket leaks by checking orphan sockets growth
        let content1 = "TCP: inuse 100 orphan 5 tw 10";
        let content2 = "TCP: inuse 100 orphan 50 tw 10"; // orphan count increased

        let result1 = parse_sockstat_content(content1);
        let result2 = parse_sockstat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let buffer1 = result1.unwrap();
        let buffer2 = result2.unwrap();

        let orphan1 = buffer1
            .iter()
            .find(|e| e.metric == "orphan")
            .map(|e| e.value);
        let orphan2 = buffer2
            .iter()
            .find(|e| e.metric == "orphan")
            .map(|e| e.value);

        assert_eq!(orphan1, Some(5));
        assert_eq!(orphan2, Some(50));
        assert!(orphan2.unwrap() > orphan1.unwrap());
    }
}
