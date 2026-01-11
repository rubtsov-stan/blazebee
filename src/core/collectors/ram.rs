use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Comprehensive snapshot of system memory usage across different categories.
/// All values are in kilobytes as reported by the kernel's /proc/meminfo interface.
/// This structure provides a detailed breakdown of physical RAM and swap memory usage,
/// useful for monitoring memory pressure and available free memory.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total amount of physical RAM available to the system in kilobytes.
    /// This is a fixed value that doesn't change unless hardware is added/removed.
    pub mem_total: u64,
    /// Amount of unallocated physical RAM not in use by the kernel or any process.
    /// Warning: this value can be misleading as it doesn't account for memory freed by
    /// processes but not yet reclaimed by the kernel.
    pub mem_free: u64,
    /// Better indicator of truly available memory than mem_free. Includes memory that
    /// is currently free plus memory that can be easily reclaimed (from caches and buffers).
    /// This is the recommended value to check for available system memory.
    pub mem_available: u64,
    /// Memory allocated to filesystem buffers for disk I/O operations.
    /// This memory is reclaimed by the kernel if needed by processes.
    pub buffers: u64,
    /// Memory allocated to page cache from filesystem reads. Includes memory used by
    /// applications for caching and memory-mapped files. Can be reclaimed when needed.
    pub cached: u64,
    /// Total amount of swap space configured in the system in kilobytes.
    pub swap_total: u64,
    /// Amount of unused swap space available in kilobytes. High usage indicates the
    /// system is swapping memory to disk, which significantly impacts performance.
    pub swap_free: u64,
}

/// The main collector responsible for gathering detailed memory statistics from the kernel.
/// Parses /proc/meminfo which provides a comprehensive view of physical and virtual memory
/// usage. The collector uses a HashMap to flexibly handle the key-value format while being
/// robust to variations or missing fields across different Linux kernel versions.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryCollector;

impl MemoryCollector {
    /// Creates a new memory collector instance.
    pub fn new() -> Self {
        MemoryCollector
    }
}

impl Default for MemoryCollector {
    /// Provides the default constructor, allowing MemoryCollector to be instantiated
    /// using the Default trait. Useful in generic code that requires Default implementations.
    fn default() -> Self {
        MemoryCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting memory statistics.
/// Reads from /proc/meminfo which contains detailed memory information formatted as key-value pairs.
/// The implementation parses this file into a HashMap for flexible field extraction.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for MemoryCollector {
    type Output = MemoryStats;

    /// Asynchronously reads memory statistics from /proc/meminfo.
    ///
    /// The method parses the file which has a "key: value unit" format, e.g.:
    /// ```text
    /// MemTotal:       16384000 kB
    /// MemFree:         8192000 kB
    /// MemAvailable:    12288000 kB
    /// Buffers:         1024000 kB
    /// Cached:          4096000 kB
    /// SwapTotal:       2097152 kB
    /// SwapFree:        2097152 kB
    /// ```
    ///
    /// MemTotal is required and will cause an error if missing. Other fields default to 0
    /// if not present, allowing graceful handling of kernel version differences.
    ///
    /// Returns an error if the file cannot be read or if MemTotal is missing/malformed.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/meminfo".to_string(),
                source,
            })?;

        // Parse the file into a HashMap for flexible field lookup.
        // Pre-allocate capacity to avoid reallocations during insertion.
        let mut mem_map: HashMap<&str, u64> = HashMap::with_capacity(32);

        for line in content.lines() {
            // Split on colon to separate key from value
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() != 2 {
                // Skip malformed lines that don't have exactly one colon
                continue;
            }

            let key = parts[0].trim();
            let value_parts: Vec<&str> = parts[1].trim().split_whitespace().collect();

            if !value_parts.is_empty() {
                // Extract the numeric value (first part after the colon)
                // The unit (kB) comes second and is ignored
                if let Ok(val) = value_parts[0].parse::<u64>() {
                    mem_map.insert(key, val);
                }
            }
        }

        // Extract values from the map, using MemTotal as a required field to detect
        // if the file was successfully parsed. Other fields default to 0 for compatibility.
        Ok(MemoryStats {
            mem_total: mem_map
                .get("MemTotal")
                .copied()
                .ok_or(CollectorError::MissingField {
                    field: "MemTotal".to_string(),
                    location: "/proc/meminfo".to_string(),
                })?,
            mem_free: mem_map.get("MemFree").copied().unwrap_or(0),
            mem_available: mem_map.get("MemAvailable").copied().unwrap_or(0),
            buffers: mem_map.get("Buffers").copied().unwrap_or(0),
            cached: mem_map.get("Cached").copied().unwrap_or(0),
            swap_total: mem_map.get("SwapTotal").copied().unwrap_or(0),
            swap_free: mem_map.get("SwapFree").copied().unwrap_or(0),
        })
    }
}

#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
))]
register_collector!(MemoryCollector, "ram");

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub mem_total: u64,
    pub mem_free: u64,
    pub mem_available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that memory metrics are not available on this platform.
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryCollector;

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for MemoryCollector {
    type Output = MemoryStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Memory collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without requiring actual file I/O.
    /// This function extracts the core parsing logic for testability.
    fn parse_meminfo_content(content: &str) -> CollectorResult<MemoryStats> {
        let mut mem_map: HashMap<&str, u64> = HashMap::with_capacity(32);

        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0].trim();
            let value_parts: Vec<&str> = parts[1].trim().split_whitespace().collect();

            if !value_parts.is_empty() {
                if let Ok(val) = value_parts[0].parse::<u64>() {
                    mem_map.insert(key, val);
                }
            }
        }

        Ok(MemoryStats {
            mem_total: mem_map
                .get("MemTotal")
                .copied()
                .ok_or(CollectorError::MissingField {
                    field: "MemTotal".to_string(),
                    location: "/proc/meminfo".to_string(),
                })?,
            mem_free: mem_map.get("MemFree").copied().unwrap_or(0),
            mem_available: mem_map.get("MemAvailable").copied().unwrap_or(0),
            buffers: mem_map.get("Buffers").copied().unwrap_or(0),
            cached: mem_map.get("Cached").copied().unwrap_or(0),
            swap_total: mem_map.get("SwapTotal").copied().unwrap_or(0),
            swap_free: mem_map.get("SwapFree").copied().unwrap_or(0),
        })
    }

    // ---- Test cases ----

    #[test]
    fn test_parse_valid_meminfo() {
        // Realistic output from /proc/meminfo on a system with 16GB RAM and 2GB swap
        let content = r#"MemTotal:       16384000 kB
MemFree:         8192000 kB
MemAvailable:   12288000 kB
Buffers:        1024000 kB
Cached:         4096000 kB
SwapTotal:      2097152 kB
SwapFree:       2097152 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 8192000);
        assert_eq!(stats.mem_available, 12288000);
        assert_eq!(stats.buffers, 1024000);
        assert_eq!(stats.cached, 4096000);
        assert_eq!(stats.swap_total, 2097152);
        assert_eq!(stats.swap_free, 2097152);
    }

    #[test]
    fn test_parse_with_extra_fields() {
        // Real /proc/meminfo has many more fields; verify we ignore irrelevant ones
        let content = r#"MemTotal:       16384000 kB
MemFree:         8192000 kB
MemAvailable:   12288000 kB
Buffers:        1024000 kB
Cached:         4096000 kB
SReclaimable:    512000 kB
SUnreclaim:      256000 kB
SwapTotal:      2097152 kB
SwapFree:       2097152 kB
Dirty:           102400 kB
Writeback:            0 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.cached, 4096000);
        assert_eq!(stats.swap_free, 2097152);
    }

    #[test]
    fn test_parse_missing_memtotal() {
        // MemTotal is required; without it the parser should fail
        let content = r#"MemFree:         8192000 kB
MemAvailable:   12288000 kB
SwapTotal:      2097152 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::MissingField { field, .. }) = result {
            assert_eq!(field, "MemTotal");
        } else {
            panic!("Expected MissingField error for MemTotal");
        }
    }

    #[test]
    fn test_parse_missing_optional_fields() {
        // Optional fields should default to 0 if missing
        let content = "MemTotal:       16384000 kB";

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 0);
        assert_eq!(stats.mem_available, 0);
        assert_eq!(stats.buffers, 0);
        assert_eq!(stats.cached, 0);
        assert_eq!(stats.swap_total, 0);
        assert_eq!(stats.swap_free, 0);
    }

    #[test]
    fn test_parse_zero_values() {
        // System with no swap and zero free memory (stress test condition)
        let content = r#"MemTotal:       16384000 kB
MemFree:              0 kB
MemAvailable:         0 kB
Buffers:              0 kB
Cached:               0 kB
SwapTotal:            0 kB
SwapFree:            0 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 0);
        assert_eq!(stats.swap_free, 0);
    }

    #[test]
    fn test_parse_high_values() {
        // System with very large memory (e.g., 1TB RAM)
        let content = r#"MemTotal:    1099511627776 kB
MemFree:     549755813888 kB
MemAvailable: 824633720832 kB
SwapTotal:      0 kB
SwapFree:       0 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 1099511627776);
        assert_eq!(stats.mem_free, 549755813888);
    }

    #[test]
    fn test_parse_invalid_memtotal_value() {
        let content = r#"MemTotal:       not_a_number kB
MemFree:         8192000 kB
SwapTotal:      2097152 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_err());

        if let Err(CollectorError::MissingField { .. }) = result {
            // Non-numeric values fail to parse, so MemTotal is treated as missing
        } else {
            panic!("Expected MissingField error for invalid MemTotal");
        }
    }

    #[test]
    fn test_parse_invalid_optional_field_value() {
        // Invalid values in optional fields should be silently ignored (treated as 0)
        let content = r#"MemTotal:       16384000 kB
MemFree:         not_a_number kB
MemAvailable:   12288000 kB
Cached:         broken_value kB
SwapTotal:      2097152 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        // Invalid values default to 0
        assert_eq!(stats.mem_free, 0);
        assert_eq!(stats.cached, 0);
    }

    #[test]
    fn test_parse_malformed_lines() {
        // Lines without colons should be skipped
        let content = r#"MemTotal:       16384000 kB
This is not a valid line
Another invalid line with no colon
MemFree:         8192000 kB
Line: with: multiple: colons: ignored
SwapTotal:      2097152 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 8192000);
    }

    #[test]
    fn test_parse_extra_whitespace() {
        // Verify robustness with extra whitespace
        let content = r#"MemTotal:    16384000   kB
MemFree:       8192000    kB
MemAvailable:   12288000  kB
Buffers:        1024000   kB
Cached:         4096000   kB
SwapTotal:      2097152   kB
SwapFree:       2097152   kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.buffers, 1024000);
    }

    #[test]
    fn test_parse_unit_ignored() {
        // Different unit formats should be parsed correctly (only the number matters)
        let content = r#"MemTotal:       16384000 kB
MemFree:         8192000 MB
MemAvailable:   12288000 KB
SwapTotal:      2097152 kilobytes"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // Only the numeric part is parsed
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 8192000);
    }

    #[test]
    fn test_memory_stats_serialization() {
        let stats = MemoryStats {
            mem_total: 16384000,
            mem_free: 8192000,
            mem_available: 12288000,
            buffers: 1024000,
            cached: 4096000,
            swap_total: 2097152,
            swap_free: 2097152,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&stats);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        // Verify we can deserialize it back
        let deserialized: Result<MemoryStats, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.mem_total, stats.mem_total);
        assert_eq!(restored.mem_free, stats.mem_free);
        assert_eq!(restored.swap_free, stats.swap_free);
    }

    #[test]
    fn test_memory_stats_clone() {
        let original = MemoryStats {
            mem_total: 16384000,
            mem_free: 8192000,
            mem_available: 12288000,
            buffers: 1024000,
            cached: 4096000,
            swap_total: 2097152,
            swap_free: 2097152,
        };

        let cloned = original.clone();

        assert_eq!(original.mem_total, cloned.mem_total);
        assert_eq!(original.mem_free, cloned.mem_free);
        assert_eq!(original.cached, cloned.cached);
        assert_eq!(original.swap_total, cloned.swap_total);
    }

    #[test]
    fn test_memory_collector_creation() {
        let collector = MemoryCollector::new();
        let default_collector = MemoryCollector::default();

        // Both should create valid instances without panicking
        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_realistic_modern_linux() {
        // Output from a modern Linux system with many fields
        let content = r#"MemTotal:       16384000 kB
MemFree:         2048000 kB
MemAvailable:    8192000 kB
Buffers:         512000 kB
Cached:          6144000 kB
SwapCached:      262144 kB
Active:          8388608 kB
Inactive:        4194304 kB
SwapTotal:       4194304 kB
SwapFree:        3932160 kB
Dirty:           102400 kB
Writeback:            0 kB
AnonPages:       6291456 kB
Mapped:          1048576 kB
Shmem:           524288 kB"#;

        let result = parse_meminfo_content(content);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 2048000);
        assert_eq!(stats.mem_available, 8192000);
        assert_eq!(stats.cached, 6144000);
        assert_eq!(stats.swap_total, 4194304);
        assert_eq!(stats.swap_free, 3932160);
    }

    #[test]
    fn test_memory_stats_debug_format() {
        let stats = MemoryStats {
            mem_total: 16384000,
            mem_free: 8192000,
            mem_available: 12288000,
            buffers: 1024000,
            cached: 4096000,
            swap_total: 2097152,
            swap_free: 2097152,
        };

        let debug_string = format!("{:?}", stats);
        assert!(debug_string.contains("mem_total"));
        assert!(debug_string.contains("mem_free"));
        assert!(debug_string.contains("16384000"));
    }

    #[test]
    fn test_parse_case_sensitivity() {
        // Keys should match exactly (case-sensitive)
        let content = r#"MemTotal:       16384000 kB
memfree:         8192000 kB
SWAPTOTAL:      2097152 kB
memAvailable:   12288000 kB"#;

        let result = parse_meminfo_content(content);
        // Should fail because "memfree", "SWAPTOTAL", "memAvailable" don't match expected keys
        // Only MemTotal is found, but that's enough; others default to 0
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.mem_total, 16384000);
        // These won't match, so they default to 0
        assert_eq!(stats.mem_free, 0);
        assert_eq!(stats.swap_total, 0);
    }
}
