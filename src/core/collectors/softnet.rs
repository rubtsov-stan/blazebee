use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Represents software-based network packet processing statistics for a single CPU core.
/// These metrics describe how the kernel's softnet (software-based packet processing)
/// subsystem is performing on each logical CPU. The data comes from /proc/net/softnet_stat
/// and is expressed as cumulative counters since system boot, stored in hexadecimal format.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftnetStats<'a> {
    /// Identifier for the CPU core (e.g., "0", "1", "2"). Maps to the line number in
    /// /proc/net/softnet_stat where line 0 is cpu0, line 1 is cpu1, etc.
    pub cpu: Cow<'a, str>,
    /// Total number of network packets processed by softnet on this CPU (cumulative).
    /// This is the primary metric indicating network activity level. Compare with
    /// dropped to understand packet loss. Stored as hexadecimal in the kernel file.
    pub processed: u64,
    /// Total number of network packets dropped by softnet on this CPU due to processing
    /// limits (cumulative). Non-zero values indicate the CPU could not keep up with
    /// packet arrival rate. High values suggest network congestion or packet loss on that core.
    pub dropped: u64,
    /// Number of times the softnet backlog processor reached its time limit (cumulative).
    /// The kernel bounds softnet processing to prevent it from completely starving other
    /// work. High values indicate the CPU is spending significant time on network processing.
    /// Can be a sign of network congestion or inadequate packet processing capacity.
    pub time_squeeze: u64,
}

/// Container for software network statistics from all logical CPU cores.
/// This buffer collects softnet_stat data for each CPU and provides convenient access
/// through standard collection methods. Data comes from /proc/net/softnet_stat.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SoftnetBuffer<'a> {
    stats: Vec<SoftnetStats<'a>>,
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
impl<'a> SoftnetBuffer<'a> {
    /// Creates an empty softnet buffer with no pre-allocated capacity.
    pub fn new() -> Self {
        SoftnetBuffer { stats: Vec::new() }
    }

    /// Creates an empty softnet buffer with pre-allocated capacity.
    /// Useful when the number of CPUs is known in advance.
    pub fn with_capacity(capacity: usize) -> Self {
        SoftnetBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Appends network packet processing statistics to the buffer.
    pub fn push(&mut self, stat: SoftnetStats<'a>) {
        self.stats.push(stat);
    }

    /// Returns an iterator over all CPU softnet statistics in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &SoftnetStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of CPUs whose statistics are stored in the buffer.
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer contains any CPU statistics.
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Returns the statistics as a slice for direct array access.
    pub fn as_slice(&self) -> &[SoftnetStats<'a>] {
        &self.stats
    }
}

/// The main collector responsible for gathering software network statistics from the kernel.
/// Reads /proc/net/softnet_stat which exposes per-CPU counters for the kernel's software-based
/// network packet processing (NET_RX_SOFTIRQ). Essential for diagnosing network performance
/// issues and understanding packet handling load distribution across CPUs.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
pub struct SoftnetCollector;

impl SoftnetCollector {
    pub fn new() -> Self {
        SoftnetCollector
    }
}

impl Default for SoftnetCollector {
    fn default() -> Self {
        SoftnetCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting softnet statistics.
/// Reads from /proc/net/softnet_stat which contains per-CPU network packet processing counters
/// in hexadecimal format. The parser is lenient with parse errors, defaulting to 0 for values
/// that fail to convert from hex, ensuring robustness across different system states.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for SoftnetCollector {
    type Output = SoftnetBuffer<'static>;

    /// Asynchronously reads network packet processing statistics from /proc/net/softnet_stat.
    ///
    /// The file format contains one line per CPU core with hexadecimal values:
    /// ```text
    /// 0000a1b2 00000000 00012345
    /// 0000c3d4 00000001 00054321
    /// 0000e5f6 00000002 00087654
    /// ```
    /// Each line contains (in order):
    /// - processed: packets processed (hex)
    /// - dropped: packets dropped (hex)
    /// - time_squeeze: times backlog limit reached (hex)
    ///
    /// The CPU ID is derived from the line number. Lines with fewer than 3 fields are skipped.
    /// Hexadecimal parsing errors are handled gracefully by defaulting to 0, allowing the
    /// collector to continue even if some values are malformed.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/net/softnet_stat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/net/softnet_stat".to_string(),
                source,
            })?;

        let mut buffer = SoftnetBuffer::new();

        for (cpu_id, line) in content.lines().enumerate() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                // Parse hexadecimal values. Default to 0 if parsing fails to maintain
                // robustness—some fields may be malformed or unavailable on certain systems.
                let processed = u64::from_str_radix(parts[0], 16).unwrap_or(0);
                let dropped = u64::from_str_radix(parts[1], 16).unwrap_or(0);
                let time_squeeze = u64::from_str_radix(parts[2], 16).unwrap_or(0);

                buffer.push(SoftnetStats {
                    cpu: Cow::Owned(cpu_id.to_string()),
                    processed,
                    dropped,
                    time_squeeze,
                });
            }
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
register_collector!(SoftnetCollector, "softnet");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftnetStats<'a> {
    pub cpu: Cow<'a, str>,
    pub processed: u64,
    pub dropped: u64,
    pub time_squeeze: u64,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SoftnetBuffer<'a> {
    stats: Vec<SoftnetStats<'a>>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
)))]
impl<'a> SoftnetBuffer<'a> {
    pub fn new() -> Self {
        SoftnetBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        SoftnetBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stat: SoftnetStats<'a>) {
        self.stats.push(stat);
    }

    pub fn iter(&self) -> impl Iterator<Item = &SoftnetStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    pub fn as_slice(&self) -> &[SoftnetStats<'a>] {
        &self.stats
    }
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that softnet statistics are not available on this platform.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
)))]
pub struct SoftnetCollector;

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for SoftnetCollector {
    type Output = SoftnetBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Softnet collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    fn parse_softnet_content(content: &str) -> CollectorResult<SoftnetBuffer<'static>> {
        let mut buffer = SoftnetBuffer::new();

        for (cpu_id, line) in content.lines().enumerate() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let processed = u64::from_str_radix(parts[0], 16).unwrap_or(0);
                let dropped = u64::from_str_radix(parts[1], 16).unwrap_or(0);
                let time_squeeze = u64::from_str_radix(parts[2], 16).unwrap_or(0);

                buffer.push(SoftnetStats {
                    cpu: Cow::Owned(cpu_id.to_string()),
                    processed,
                    dropped,
                    time_squeeze,
                });
            }
        }

        Ok(buffer)
    }

    // ---- Test cases ----

    #[test]
    fn test_parse_valid_softnet_hex() {
        // Realistic output from /proc/net/softnet_stat with hexadecimal values
        let content = r#"0000a1b2 00000000 00012345
0000c3d4 00000001 00054321
0000e5f6 00000002 00087654"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 3);

        let stats = buffer.as_slice();
        // Verify hex conversion: 0x0000a1b2 = 41394
        assert_eq!(stats[0].cpu, "0");
        assert_eq!(stats[0].processed, 0x0000a1b2);
        assert_eq!(stats[0].dropped, 0x00000000);
        assert_eq!(stats[0].time_squeeze, 0x00012345);

        // Verify second CPU
        assert_eq!(stats[1].cpu, "1");
        assert_eq!(stats[1].processed, 0x0000c3d4);
        assert_eq!(stats[1].dropped, 0x00000001);
        assert_eq!(stats[1].time_squeeze, 0x00054321);
    }

    #[test]
    fn test_parse_single_cpu() {
        let content = "00123456 00000000 00000001";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 1);

        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.cpu, "0");
        assert_eq!(stat.processed, 0x00123456);
        assert_eq!(stat.dropped, 0x00000000);
        assert_eq!(stat.time_squeeze, 0x00000001);
    }

    #[test]
    fn test_parse_many_cpus() {
        // System with 8 logical CPUs
        let content = r#"00100000 00000000 00000001
00200000 00000000 00000002
00300000 00000000 00000003
00400000 00000000 00000004
00500000 00000000 00000005
00600000 00000000 00000006
00700000 00000000 00000007
00800000 00000000 00000008"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 8);

        let stats = buffer.as_slice();
        assert_eq!(stats[0].cpu, "0");
        assert_eq!(stats[7].cpu, "7");
        assert_eq!(stats[7].processed, 0x00800000);
    }

    #[test]
    fn test_parse_zero_values() {
        let content = "00000000 00000000 00000000";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 0);
        assert_eq!(stat.dropped, 0);
        assert_eq!(stat.time_squeeze, 0);
    }

    #[test]
    fn test_parse_max_hex_values() {
        // Test with maximum u64 hex value
        let content = "ffffffffffffffff ffffffffffffffff ffffffffffffffff";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, u64::MAX);
        assert_eq!(stat.dropped, u64::MAX);
        assert_eq!(stat.time_squeeze, u64::MAX);
    }

    #[test]
    fn test_parse_lowercase_hex() {
        // Kernel may output lowercase hex digits
        let content = "0000abcd 00000000 0000ef01";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 0x0000abcd);
        assert_eq!(stat.time_squeeze, 0x0000ef01);
    }

    #[test]
    fn test_parse_mixed_case_hex() {
        // Verify hex parsing handles mixed case
        let content = "0000AbCd 00000000 0000EF01";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 0x0000abcd);
        assert_eq!(stat.time_squeeze, 0x0000ef01);
    }

    #[test]
    fn test_parse_invalid_hex_defaults_to_zero() {
        // Invalid hex values default to 0
        let content = "0000zzzz 00000000 00000001";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 0); // Invalid hex defaults to 0
        assert_eq!(stat.dropped, 0);
        assert_eq!(stat.time_squeeze, 1);
    }

    #[test]
    fn test_parse_partial_invalid_hex() {
        // Some values invalid, some valid
        let content = "0000aaaa invalid 00000002";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 0x0000aaaa);
        assert_eq!(stat.dropped, 0); // Invalid defaults to 0
        assert_eq!(stat.time_squeeze, 2);
    }

    #[test]
    fn test_parse_extra_fields_ignored() {
        // Extra fields beyond the first 3 are ignored
        let content = "00000001 00000002 00000003 extra field extra";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 1);
        assert_eq!(stat.dropped, 2);
        assert_eq!(stat.time_squeeze, 3);
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        // Test robustness with variable whitespace
        let content = "00000001    00000002    00000003";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stat = &buffer.as_slice()[0];
        assert_eq!(stat.processed, 1);
        assert_eq!(stat.dropped, 2);
        assert_eq!(stat.time_squeeze, 3);
    }

    #[test]
    fn test_parse_realistic_high_load() {
        // Scenario: CPU under heavy network load
        let content = r#"00fff000 00000001 00001234
00ffff00 00000002 00005678"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stats = buffer.as_slice();

        // CPU 0 has processed many packets
        assert!(stats[0].processed > 1000000);
        // And has some drops and time squeezes
        assert!(stats[0].dropped > 0);
        assert!(stats[0].time_squeeze > 0);
    }

    #[test]
    fn test_parse_realistic_idle() {
        // Scenario: Minimal network activity
        let content = r#"00000001 00000000 00000000
00000002 00000000 00000000
00000001 00000000 00000000"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 3);

        let stats = buffer.as_slice();
        for stat in stats {
            // Very low packet counts
            assert!(stat.processed <= 0x10);
            // No drops or time squeezes
            assert_eq!(stat.dropped, 0);
            assert_eq!(stat.time_squeeze, 0);
        }
    }

    #[test]
    fn test_softnet_buffer_operations() {
        let mut buffer = SoftnetBuffer::new();

        let stat1 = SoftnetStats {
            cpu: Cow::Borrowed("0"),
            processed: 1000,
            dropped: 5,
            time_squeeze: 10,
        };

        let stat2 = SoftnetStats {
            cpu: Cow::Borrowed("1"),
            processed: 2000,
            dropped: 10,
            time_squeeze: 20,
        };

        buffer.push(stat1);
        buffer.push(stat2);

        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());

        let stats: Vec<_> = buffer.iter().collect();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].cpu, "0");
        assert_eq!(stats[1].cpu, "1");

        let slice = buffer.as_slice();
        assert_eq!(slice[0].processed, 1000);
        assert_eq!(slice[1].processed, 2000);
    }

    #[test]
    fn test_softnet_buffer_with_capacity() {
        let buffer = SoftnetBuffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_softnet_stats_serialization() {
        let stat = SoftnetStats {
            cpu: Cow::Borrowed("3"),
            processed: 0x12345678,
            dropped: 0x00000005,
            time_squeeze: 0x00000010,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&stat);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<SoftnetStats, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.cpu, "3");
        assert_eq!(restored.processed, stat.processed);
    }

    #[test]
    fn test_softnet_stats_clone() {
        let original = SoftnetStats {
            cpu: Cow::Owned("5".to_string()),
            processed: 0x11111111,
            dropped: 0x22222222,
            time_squeeze: 0x33333333,
        };

        let cloned = original.clone();

        assert_eq!(original.cpu, cloned.cpu);
        assert_eq!(original.processed, cloned.processed);
        assert_eq!(original.dropped, cloned.dropped);
        assert_eq!(original.time_squeeze, cloned.time_squeeze);
    }

    #[test]
    fn test_softnet_collector_creation() {
        let collector = SoftnetCollector::new();
        let default_collector = SoftnetCollector::default();

        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_parse_packet_drop_detection() {
        // Scenario: Detecting when packets are being dropped
        let content1 = "00000100 00000000 00000001";
        let content2 = "00000100 00000005 00000001"; // Drops increased

        let result1 = parse_softnet_content(content1);
        let result2 = parse_softnet_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let buffer1 = result1.unwrap();
        let buffer2 = result2.unwrap();

        let drops1 = buffer1.as_slice()[0].dropped;
        let drops2 = buffer2.as_slice()[0].dropped;

        assert_eq!(drops1, 0);
        assert_eq!(drops2, 5);
        assert!(drops2 > drops1);
    }

    #[test]
    fn test_parse_cpu_load_comparison() {
        // Scenario: Comparing load across CPUs
        let content = r#"00001000 00000000 00000010
00002000 00000000 00000020
00000500 00000000 00000005"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stats = buffer.as_slice();

        // Find the most loaded CPU
        let most_loaded = stats
            .iter()
            .max_by_key(|s| s.time_squeeze)
            .map(|s| s.cpu.as_ref());

        assert_eq!(most_loaded, Some("1"));
    }

    #[test]
    fn test_softnet_stats_debug_format() {
        let stat = SoftnetStats {
            cpu: Cow::Borrowed("2"),
            processed: 0xabcd1234,
            dropped: 0x00000001,
            time_squeeze: 0x00000100,
        };

        let debug_string = format!("{:?}", stat);
        assert!(debug_string.contains("processed"));
        assert!(debug_string.contains("dropped"));
        assert!(debug_string.contains("time_squeeze"));
    }

    #[test]
    fn test_parse_cpu_id_sequential() {
        // Verify CPU IDs are assigned sequentially based on line number
        let content = r#"00000001 00000000 00000000
00000002 00000000 00000000
00000003 00000000 00000000"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stats = buffer.as_slice();

        assert_eq!(stats[0].cpu, "0");
        assert_eq!(stats[1].cpu, "1");
        assert_eq!(stats[2].cpu, "2");
    }

    #[test]
    fn test_parse_sum_all_cpus() {
        // Scenario: Summing metrics across all CPUs for total system load
        let content = r#"00000100 00000001 00000010
00000200 00000002 00000020
00000300 00000003 00000030"#;

        let result = parse_softnet_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let stats = buffer.as_slice();

        let total_processed: u64 = stats.iter().map(|s| s.processed).sum();
        let total_dropped: u64 = stats.iter().map(|s| s.dropped).sum();
        let total_time_squeeze: u64 = stats.iter().map(|s| s.time_squeeze).sum();

        assert_eq!(total_processed, 0x100 + 0x200 + 0x300);
        assert_eq!(total_dropped, 6);
        assert_eq!(total_time_squeeze, 0x60);
    }
}
