use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Represents a single virtual memory statistic entry from /proc/vmstat.
/// Each entry is a key-value pair where the key identifies a kernel VM counter
/// and the value is the cumulative count since system boot. These counters track
/// memory management activities like page reclamation, swapping, and page faults.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmstatEntry<'a> {
    /// The statistic name (e.g., "pgpgin", "pgpgout", "pgfault", "pswpin", "pswwout").
    /// Common categories include:
    /// - Page activity: pgpgin, pgpgout (pages read/written from/to disk)
    /// - Faults: pgfault, pgmajfault (page faults and major page faults)
    /// - Swapping: pswpin, pswwout (pages swapped in/out)
    /// - Reclamation: pgreclaim, pgsteal (pages reclaimed by kernel)
    /// - Allocation: pgalloc_normal, pgalloc_movable (pages allocated by type)
    pub key: Cow<'a, str>,
    /// The counter value—a cumulative count since the last system boot.
    /// These are monotonically increasing counters (never decrease under normal operation).
    /// To calculate rates, compute the delta between two measurements and divide by time.
    /// For example, (pgpgin_now - pgpgin_prev) / seconds_elapsed = pages/second read from disk.
    pub value: u64,
}

/// Container for virtual memory statistics from the kernel's VM subsystem.
/// This buffer collects all VM counters from /proc/vmstat and provides convenient
/// access through standard collection methods. Data comes from /proc/vmstat which
/// exposes counters for memory management, page reclamation, and swap activity.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VmstatBuffer<'a> {
    entries: Vec<VmstatEntry<'a>>,
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
impl<'a> VmstatBuffer<'a> {
    /// Creates an empty vmstat buffer with no pre-allocated capacity.
    pub fn new() -> Self {
        VmstatBuffer {
            entries: Vec::new(),
        }
    }

    /// Creates an empty vmstat buffer with pre-allocated capacity.
    /// Useful when the expected number of vmstat entries is known in advance.
    /// Typical systems have 100-200 vmstat entries depending on kernel version.
    pub fn with_capacity(capacity: usize) -> Self {
        VmstatBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Appends a vmstat entry to the buffer.
    pub fn push(&mut self, entry: VmstatEntry<'a>) {
        self.entries.push(entry);
    }

    /// Returns an iterator over all vmstat entries in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &VmstatEntry<'a>> {
        self.entries.iter()
    }

    /// Returns the number of vmstat entries currently in the buffer.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Checks if the buffer contains any vmstat entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the vmstat entries as a slice for direct array access.
    pub fn as_slice(&self) -> &[VmstatEntry<'a>] {
        &self.entries
    }
}

/// The main collector responsible for gathering virtual memory statistics from the kernel.
/// Reads /proc/vmstat which exposes cumulative counters for memory management operations
/// including page in/out, swapping, page faults, and memory reclamation. Essential for
/// diagnosing memory pressure, swap usage, and page fault rates.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
pub struct VmstatCollector;

impl VmstatCollector {
    pub fn new() -> Self {
        VmstatCollector
    }
}

impl Default for VmstatCollector {
    /// Provides the default constructor, allowing VmstatCollector to be instantiated
    /// using the Default trait. Useful in generic code that requires Default implementations.
    fn default() -> Self {
        VmstatCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting virtual memory statistics.
/// Reads from /proc/vmstat which contains kernel VM counters in a simple key-value format.
/// The parser is lenient, skipping malformed lines and silently ignoring non-parseable values.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for VmstatCollector {
    type Output = VmstatBuffer<'static>;

    /// Asynchronously reads virtual memory statistics from /proc/vmstat.
    ///
    /// The file format contains one entry per line with a key-value pair separated by whitespace:
    /// ```text
    /// pgpgin 12345678
    /// pgpgout 87654321
    /// pgfault 1234567
    /// pgmajfault 56789
    /// pswpin 12345
    /// pswwout 54321
    /// ```
    ///
    /// Each line should have exactly 2 fields: the statistic name and its numeric value.
    /// Lines with fewer or more than 2 fields are skipped. Values that fail to parse as u64
    /// are silently ignored, maintaining robustness across kernel versions with varying
    /// counter types or availability.
    ///
    /// Returns an error only if the file cannot be read. Parse errors on individual
    /// entries are silently ignored to allow partial data collection.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/vmstat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/vmstat".to_string(),
                source,
            })?;

        let mut buffer = VmstatBuffer::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Only process lines with exactly 2 fields: key and value
            if parts.len() == 2 {
                // Try to parse the value as u64, skipping on failure
                if let Ok(val) = parts[1].parse::<u64>() {
                    buffer.push(VmstatEntry {
                        key: Cow::Owned(parts[0].to_string()),
                        value: val,
                    });
                }
                // Unparseable values are silently skipped
            }
            // Lines with != 2 fields are skipped
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
register_collector!(VmstatCollector, "vmstat");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmstatEntry<'a> {
    pub key: Cow<'a, str>,
    pub value: u64,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VmstatBuffer<'a> {
    entries: Vec<VmstatEntry<'a>>,
}

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
)))]
impl<'a> VmstatBuffer<'a> {
    pub fn new() -> Self {
        VmstatBuffer {
            entries: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        VmstatBuffer {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, entry: VmstatEntry<'a>) {
        self.entries.push(entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = &VmstatEntry<'a>> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn as_slice(&self) -> &[VmstatEntry<'a>] {
        &self.entries
    }
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that vmstat metrics are not available on this platform.
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
)))]
pub struct VmstatCollector;

#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for VmstatCollector {
    type Output = VmstatBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Vmstat collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    fn parse_vmstat_content(content: &str) -> CollectorResult<VmstatBuffer<'static>> {
        let mut buffer = VmstatBuffer::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                if let Ok(val) = parts[1].parse::<u64>() {
                    buffer.push(VmstatEntry {
                        key: Cow::Owned(parts[0].to_string()),
                        value: val,
                    });
                }
            }
        }

        Ok(buffer)
    }

    // ---- Test cases ----

    #[test]
    fn test_parse_valid_vmstat() {
        // Realistic output from /proc/vmstat
        let content = r#"pgpgin 12345678
pgpgout 87654321
pgfault 1234567
pgmajfault 56789
pswpin 12345
pswwout 54321"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 6);

        let entries = buffer.as_slice();
        assert_eq!(entries[0].key, "pgpgin");
        assert_eq!(entries[0].value, 12345678);
        assert_eq!(entries[1].key, "pgpgout");
        assert_eq!(entries[1].value, 87654321);
    }

    #[test]
    fn test_parse_single_entry() {
        let content = "pgfault 1000000";

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.as_slice()[0].key, "pgfault");
        assert_eq!(buffer.as_slice()[0].value, 1000000);
    }

    #[test]
    fn test_parse_many_entries() {
        // System with extensive vmstat output (typical modern kernel)
        let mut content = String::new();
        for i in 0..150 {
            content.push_str(&format!("stat_{} {}\n", i, i * 1000));
        }

        let result = parse_vmstat_content(&content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 150);
    }

    #[test]
    fn test_parse_zero_values() {
        let content = r#"pgpgin 0
pgpgout 0
pgfault 0"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        for entry in buffer.iter() {
            assert_eq!(entry.value, 0);
        }
    }

    #[test]
    fn test_parse_large_values() {
        // Test with very large counter values
        let content = "pgpgin 18446744073709551615\npgpgout 10000000000000";

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.as_slice()[0].value, u64::MAX);
        assert_eq!(buffer.as_slice()[1].value, 10000000000000);
    }

    #[test]
    fn test_parse_invalid_value_skipped() {
        // Non-numeric value is skipped, but valid entries are processed
        let content = r#"pgpgin 12345678
pgpgout invalid_number
pgfault 1234567"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // pgpgout is skipped due to parse error
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.as_slice()[0].key, "pgpgin");
        assert_eq!(buffer.as_slice()[1].key, "pgfault");
    }

    #[test]
    fn test_parse_malformed_lines_skipped() {
        // Lines with wrong field count are skipped
        let content = r#"pgpgin
pgpgout 87654321
pgfault 1234567 extra_field
pswpin 12345"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // Only pgpgout and pswpin have exactly 2 fields
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        let content = "pgpgin     12345678\npgpgout\t\t87654321";

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.as_slice()[0].value, 12345678);
        assert_eq!(buffer.as_slice()[1].value, 87654321);
    }

    #[test]
    fn test_parse_memory_pressure_indicators() {
        // Entries indicating memory pressure
        let content = r#"pgmajfault 567890
pgreclaim 123456
pgscan_direct 654321
pgscan_kswapd 987654"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 4);

        // Verify we can find specific entries
        let majfault = buffer
            .iter()
            .find(|e| e.key == "pgmajfault")
            .map(|e| e.value);
        assert_eq!(majfault, Some(567890));
    }

    #[test]
    fn test_parse_swap_activity() {
        // Entries indicating swap usage
        let content = r#"pswpin 12345
pswwout 54321
pgsteal_normal 111111
pgsteal_movable 222222"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let pswpin = buffer.iter().find(|e| e.key == "pswpin").map(|e| e.value);

        assert_eq!(pswpin, Some(12345));
    }

    #[test]
    fn test_parse_allocation_statistics() {
        // Memory allocation-related entries
        let content = r#"pgalloc_normal 1000000
pgalloc_movable 2000000
pgalloc_dma 500000"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_vmstat_buffer_operations() {
        let mut buffer = VmstatBuffer::new();

        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgpgin"),
            value: 100,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgpgout"),
            value: 200,
        });

        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());

        let entries: Vec<_> = buffer.iter().collect();
        assert_eq!(entries[0].key, "pgpgin");
        assert_eq!(entries[1].value, 200);
    }

    #[test]
    fn test_vmstat_buffer_with_capacity() {
        let buffer = VmstatBuffer::with_capacity(200);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_vmstat_entry_serialization() {
        let entry = VmstatEntry {
            key: Cow::Borrowed("pgfault"),
            value: 1234567,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&entry);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<VmstatEntry, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.key, "pgfault");
        assert_eq!(restored.value, 1234567);
    }

    #[test]
    fn test_vmstat_entry_clone() {
        let original = VmstatEntry {
            key: Cow::Owned("pswwout".to_string()),
            value: 54321,
        };

        let cloned = original.clone();

        assert_eq!(original.key, cloned.key);
        assert_eq!(original.value, cloned.value);
    }

    #[test]
    fn test_vmstat_collector_creation() {
        let collector = VmstatCollector::new();
        let default_collector = VmstatCollector::default();

        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_parse_page_io_rate_calculation() {
        // Scenario: Calculate page I/O rate from consecutive readings
        let content1 = r#"pgpgin 1000000
pgpgout 2000000"#;

        let content2 = r#"pgpgin 1001000
pgpgout 2000100"#;

        let result1 = parse_vmstat_content(content1);
        let result2 = parse_vmstat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let buffer1 = result1.unwrap();
        let buffer2 = result2.unwrap();

        let pgpgin_delta = buffer2.as_slice()[0].value - buffer1.as_slice()[0].value;
        let pgpgout_delta = buffer2.as_slice()[1].value - buffer1.as_slice()[1].value;

        assert_eq!(pgpgin_delta, 1000);
        assert_eq!(pgpgout_delta, 100);
    }

    #[test]
    fn test_parse_memory_pressure_detection() {
        // Scenario: Detecting memory pressure from high major faults
        let content1 = r#"pgmajfault 10000"#;
        let content2 = r#"pgmajfault 15000"#;

        let result1 = parse_vmstat_content(content1);
        let result2 = parse_vmstat_content(content2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let val1 = result1.unwrap().as_slice()[0].value;
        let val2 = result2.unwrap().as_slice()[0].value;

        let majfault_increase = val2 - val1;
        assert_eq!(majfault_increase, 5000);
    }

    #[test]
    fn test_vmstat_buffer_find_by_key() {
        // Scenario: Finding a specific statistic
        let mut buffer = VmstatBuffer::new();
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgpgin"),
            value: 1000000,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgfault"),
            value: 5000000,
        });

        let pgfault_entry = buffer.iter().find(|e| e.key == "pgfault");
        assert_eq!(pgfault_entry.map(|e| e.value), Some(5000000));
    }

    #[test]
    fn test_vmstat_buffer_sum_statistics() {
        // Scenario: Summing related statistics
        let mut buffer = VmstatBuffer::new();
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgalloc_normal"),
            value: 1000000,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgalloc_movable"),
            value: 500000,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgalloc_dma"),
            value: 100000,
        });

        let total_alloc: u64 = buffer.iter().map(|e| e.value).sum();
        assert_eq!(total_alloc, 1600000);
    }

    #[test]
    fn test_vmstat_buffer_filter_by_pattern() {
        // Scenario: Filter entries by pattern (e.g., all "pgalloc_*" entries)
        let mut buffer = VmstatBuffer::new();
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgalloc_normal"),
            value: 100,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgfault"),
            value: 200,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgalloc_movable"),
            value: 300,
        });

        let alloc_entries: Vec<_> = buffer
            .iter()
            .filter(|e| e.key.starts_with("pgalloc"))
            .collect();

        assert_eq!(alloc_entries.len(), 2);
    }

    #[test]
    fn test_parse_realistic_modern_system() {
        // Realistic output from a modern Linux system with many counters
        let content = r#"nr_dirty 12345
nr_writeback 500
nr_writeback_temp 0
nr_swapcache 1000
pgpgin 10000000
pgpgout 9000000
pgfault 50000000
pgmajfault 100000
pswpin 5000
pswwout 6000
pgalloc_normal 20000000
pgalloc_movable 5000000
pgsteal_normal 1000000
pgsteal_movable 500000"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert!(buffer.len() >= 14);
    }

    #[test]
    fn test_vmstat_entry_debug_format() {
        let entry = VmstatEntry {
            key: Cow::Borrowed("pgmajfault"),
            value: 567890,
        };

        let debug_string = format!("{:?}", entry);
        assert!(debug_string.contains("key"));
        assert!(debug_string.contains("value"));
    }

    #[test]
    fn test_parse_swap_in_out_ratio() {
        // Scenario: Calculate swap activity ratio
        let content = r#"pswpin 10000
pswwout 20000"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        let entries = buffer.as_slice();

        let pswpin = entries[0].value;
        let pswwout = entries[1].value;
        let ratio = pswwout as f64 / pswpin as f64;

        assert!((ratio - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_vmstat_buffer_serialization() {
        let mut buffer = VmstatBuffer::new();
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgpgin"),
            value: 100,
        });
        buffer.push(VmstatEntry {
            key: Cow::Borrowed("pgfault"),
            value: 500,
        });

        let json = serde_json::to_string(&buffer);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<VmstatBuffer, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.len(), 2);
    }

    #[test]
    fn test_vmstat_buffer_clone() {
        let mut original = VmstatBuffer::new();
        original.push(VmstatEntry {
            key: Cow::Borrowed("pgpgin"),
            value: 1000,
        });

        let cloned = original.clone();

        assert_eq!(original.len(), cloned.len());
        assert_eq!(cloned.as_slice()[0].value, 1000);
    }

    #[test]
    fn test_parse_negative_values_unsigned() {
        // u64 cannot parse negative numbers
        let content = "pgpgin -100\npgpgout 200";

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        // Only pgpgout should be parsed
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.as_slice()[0].key, "pgpgout");
    }

    #[test]
    fn test_parse_key_naming_variations() {
        // Various vmstat key naming patterns
        let content = r#"pgpgin 1000
pgpg_out 2000
pg_fault 3000
NR_DIRTY 4000
nr_writeback 5000"#;

        let result = parse_vmstat_content(content);
        assert!(result.is_ok());

        let buffer = result.unwrap();
        assert_eq!(buffer.len(), 5);

        let keys: Vec<_> = buffer.iter().map(|e| e.key.as_ref()).collect();
        assert!(keys.contains(&"pgpgin"));
        assert!(keys.contains(&"NR_DIRTY"));
    }
}
