use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Comprehensive system uptime and idle time information collected from /proc/uptime.
/// These metrics describe how long the system has been running since the last boot,
/// and how much CPU idle time has accumulated. All time values are in seconds with
/// fractional precision for uptime and idle time (f64) and integer seconds for boot time (i64).
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeInfo {
    /// Total time in seconds the system has been running since the last boot.
    /// Measured with high precision (floating-point). This is the primary uptime metric
    /// and is directly read from /proc/uptime. Does not include time spent in suspend/hibernation.
    pub uptime_seconds: f64,
    /// Total accumulated CPU idle time in seconds since boot, summed across all CPU cores.
    /// If the system has 4 cores, this counter increments 4 seconds per real second when
    /// all cores are idle. Useful for calculating average CPU utilization percentage:
    /// (idle_time_seconds / (uptime_seconds * num_cpus)) * 100 = utilization %.
    /// May be 0 if the system doesn't provide idle time data.
    pub idle_time_seconds: f64,
    /// Unix timestamp (seconds since epoch, January 1, 1970 UTC) when the system last booted.
    /// Calculated by subtracting uptime from the current system time. Useful for determining
    /// the exact boot moment and calculating uptime duration in wall-clock terms.
    pub boot_time_seconds: i64,
}

/// The main collector responsible for gathering system uptime and idle time metrics.
/// Reads from /proc/uptime which exposes kernel uptime counters in a simple two-field format.
/// Also derives the boot timestamp by combining uptime with the current system time.
/// Essential for system health monitoring, detecting unexpected reboots, and capacity planning.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UptimeCollector;

impl UptimeCollector {
    pub fn new() -> Self {
        UptimeCollector
    }
}

impl Default for UptimeCollector {
    /// Provides the default constructor, allowing UptimeCollector to be instantiated
    /// using the Default trait. Useful in generic code that requires Default implementations.
    fn default() -> Self {
        UptimeCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting system uptime information.
/// Reads from /proc/uptime which contains two fields: uptime and idle time, both in seconds.
/// Calculates boot timestamp by subtracting uptime from current time. Handles missing idle time
/// gracefully by defaulting to 0.0.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for UptimeCollector {
    type Output = UptimeInfo;

    /// Asynchronously reads system uptime and idle time information from /proc/uptime.
    ///
    /// The /proc/uptime file contains two whitespace-separated floating-point values:
    /// ```text
    /// 123456.78 987654.32
    /// ```
    /// - First field: total uptime in seconds (with fractional part)
    /// - Second field: total idle time in seconds accumulated across all CPUs (with fractional part)
    ///
    /// The method also derives the boot_time_seconds by calculating:
    /// boot_time = current_unix_timestamp - uptime_seconds
    ///
    /// The uptime_seconds field is required. If it cannot be parsed, the entire collection fails.
    /// The idle_time_seconds field is optional and defaults to 0.0 if missing.
    ///
    /// Returns an error if:
    /// - The file cannot be read (filesystem error)
    /// - The file is empty (no fields present)
    /// - The first field (uptime) cannot be parsed as f64
    /// - The system time cannot be retrieved
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let content = tokio::fs::read_to_string("/proc/uptime")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/uptime".to_string(),
                source,
            })?;

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.is_empty() {
            return Err(CollectorError::InvalidFormat {
                location: "/proc/uptime".to_string(),
                reason: "Expected at least 1 field".to_string(),
            });
        }

        // Parse the required uptime field
        let uptime_seconds = parts[0]
            .parse::<f64>()
            .map_err(|_| CollectorError::ParseError {
                metric: "uptime_seconds".to_string(),
                location: "/proc/uptime".to_string(),
                reason: format!("invalid value: {}", parts[0]),
            })?;

        // Parse the optional idle time field, defaulting to 0.0 if missing
        let idle_time_seconds = parts
            .get(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        // Calculate boot time by subtracting uptime from current timestamp
        let boot_time = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| CollectorError::SystemCall {
                syscall: "UNIX_EPOCH".to_string(),
                reason: "failed to get current time".to_string(),
            })?
            .as_secs() as i64)
            - (uptime_seconds as i64);

        Ok(UptimeInfo {
            uptime_seconds,
            idle_time_seconds,
            boot_time_seconds: boot_time,
        })
    }
}

#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
))]
register_collector!(UptimeCollector, "uptime");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeInfo {
    pub uptime_seconds: f64,
    pub idle_time_seconds: f64,
    pub boot_time_seconds: i64,
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that uptime metrics are not available on this platform.
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UptimeCollector;

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for UptimeCollector {
    type Output = UptimeInfo;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Uptime collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helper function for parsing tests ----

    /// Simulates the produce() method's parsing logic without system time.
    /// For testing purposes, uses fixed boot_time instead of calculating from system time.
    fn parse_uptime_content(
        content: &str,
        fixed_boot_time: Option<i64>,
    ) -> CollectorResult<UptimeInfo> {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.is_empty() {
            return Err(CollectorError::InvalidFormat {
                location: "/proc/uptime".to_string(),
                reason: "Expected at least 1 field".to_string(),
            });
        }

        let uptime_seconds = parts[0]
            .parse::<f64>()
            .map_err(|_| CollectorError::ParseError {
                metric: "uptime_seconds".to_string(),
                location: "/proc/uptime".to_string(),
                reason: format!("invalid value: {}", parts[0]),
            })?;

        let idle_time_seconds = parts
            .get(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let boot_time_seconds = match fixed_boot_time {
            Some(bt) => bt,
            None => {
                // Real calculation for boot time
                (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| CollectorError::SystemCall {
                        syscall: "UNIX_EPOCH".to_string(),
                        reason: "failed to get current time".to_string(),
                    })?
                    .as_secs() as i64)
                    - (uptime_seconds as i64)
            }
        };

        Ok(UptimeInfo {
            uptime_seconds,
            idle_time_seconds,
            boot_time_seconds,
        })
    }

    // ---- Test cases ----

    #[test]
    fn test_parse_valid_uptime() {
        let content = "123456.78 987654.32";
        let result = parse_uptime_content(content, Some(1000000000));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!((info.uptime_seconds - 123456.78).abs() < 0.01);
        assert!((info.idle_time_seconds - 987654.32).abs() < 0.01);
        assert_eq!(info.boot_time_seconds, 1000000000);
    }

    #[test]
    fn test_parse_uptime_only() {
        let content = "3600.5";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!((info.uptime_seconds - 3600.5).abs() < 0.01);
        assert_eq!(info.idle_time_seconds, 0.0); // Missing idle time defaults to 0
    }

    #[test]
    fn test_parse_zero_uptime() {
        let content = "0.0 0.0";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.uptime_seconds, 0.0);
        assert_eq!(info.idle_time_seconds, 0.0);
    }

    #[test]
    fn test_parse_large_uptime() {
        // System with 365+ days of uptime
        let content = "31536000.5 126144000.0"; // ~1 year

        let result = parse_uptime_content(content, Some(1000000000));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.uptime_seconds > 31536000.0);
    }

    #[test]
    fn test_parse_fractional_values() {
        let content = "12345.6789 98765.4321";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        // Verify fractional precision is maintained
        assert!((info.uptime_seconds - 12345.6789).abs() < 0.0001);
        assert!((info.idle_time_seconds - 98765.4321).abs() < 0.0001);
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_err());
        if let Err(CollectorError::InvalidFormat { reason, .. }) = result {
            assert!(reason.contains("at least 1 field"));
        }
    }

    #[test]
    fn test_parse_invalid_uptime_value() {
        let content = "not_a_number 100.0";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_err());
        if let Err(CollectorError::ParseError { metric, .. }) = result {
            assert_eq!(metric, "uptime_seconds");
        }
    }

    #[test]
    fn test_parse_invalid_idle_time_value() {
        // Invalid idle time should not cause parse error, it defaults to 0
        let content = "3600.0 not_a_number";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.uptime_seconds, 3600.0);
        assert_eq!(info.idle_time_seconds, 0.0); // Defaults to 0
    }

    #[test]
    fn test_parse_extra_whitespace() {
        let content = "  3600.5   86400.0  ";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!((info.uptime_seconds - 3600.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_tabs_as_separator() {
        let content = "3600.5\t86400.0";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!((info.uptime_seconds - 3600.5).abs() < 0.01);
    }

    #[test]
    fn test_uptime_to_days_conversion() {
        // Scenario: Convert uptime to days
        let content = "86400.0 0.0"; // 1 day in seconds
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        let days = info.uptime_seconds / 86400.0;
        assert!((days - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_uptime_to_hours_minutes_conversion() {
        // Scenario: Convert uptime to hours and minutes
        let content = "3661.0 0.0"; // 1 hour, 1 minute, 1 second
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        let hours = info.uptime_seconds as i64 / 3600;
        let minutes = (info.uptime_seconds as i64 % 3600) / 60;
        let seconds = info.uptime_seconds as i64 % 60;

        assert_eq!(hours, 1);
        assert_eq!(minutes, 1);
        assert_eq!(seconds, 1);
    }

    #[test]
    fn test_cpu_utilization_calculation() {
        // Scenario: Calculate average CPU utilization from idle time
        let num_cpus = 4;
        let content = "3600.0 7200.0"; // 1 hour uptime, 2 hours cumulative idle
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();

        // Average CPU utilization = (idle_time / (uptime * num_cpus)) * 100
        let avg_util = (info.idle_time_seconds / (info.uptime_seconds * num_cpus as f64)) * 100.0;

        assert!((avg_util - 50.0).abs() < 0.1); // 50% utilization
    }

    #[test]
    fn test_uptime_info_serialization() {
        let info = UptimeInfo {
            uptime_seconds: 3600.5,
            idle_time_seconds: 7200.25,
            boot_time_seconds: 1609459200,
        };

        // Verify JSON serialization works
        let json = serde_json::to_string(&info);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<UptimeInfo, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert!((restored.uptime_seconds - info.uptime_seconds).abs() < 0.01);
        assert_eq!(restored.boot_time_seconds, info.boot_time_seconds);
    }

    #[test]
    fn test_uptime_info_clone() {
        let original = UptimeInfo {
            uptime_seconds: 12345.67,
            idle_time_seconds: 98765.43,
            boot_time_seconds: 1609459200,
        };

        let cloned = original.clone();

        assert!((original.uptime_seconds - cloned.uptime_seconds).abs() < 0.01);
        assert!((original.idle_time_seconds - cloned.idle_time_seconds).abs() < 0.01);
        assert_eq!(original.boot_time_seconds, cloned.boot_time_seconds);
    }

    #[test]
    fn test_uptime_collector_creation() {
        let collector = UptimeCollector::new();
        let default_collector = UptimeCollector::default();

        let _ = (collector, default_collector);
    }

    #[test]
    fn test_parse_realistic_modern_system() {
        // Realistic output from a modern Linux system
        let content = "7893456.78 31573824.45"; // ~91.4 days uptime
        let result = parse_uptime_content(content, Some(1600000000));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.uptime_seconds > 7800000.0); // > 90 days
    }

    #[test]
    fn test_parse_freshly_booted_system() {
        // System that just booted (small uptime)
        let content = "30.5 0.1"; // 30.5 seconds
        let result = parse_uptime_content(content, Some(1609459170));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.uptime_seconds < 100.0);
        assert!(info.idle_time_seconds < 10.0);
    }

    #[test]
    fn test_uptime_idle_time_ratio() {
        // Scenario: Check if idle_time > uptime * num_cpus (impossible in normal operation)
        let num_cpus = 4;
        let content = "3600.0 14400.0"; // uptime=1h, idle=4h (all cores idle 100% of time)
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();

        // Verify idle time = uptime * num_cpus (100% idle)
        let expected_idle = info.uptime_seconds * num_cpus as f64;
        assert!((info.idle_time_seconds - expected_idle).abs() < 0.1);
    }

    #[test]
    fn test_uptime_precision_microseconds() {
        // Verify microsecond precision is maintained
        let content = "123456.789012 987654.321098";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        // f64 should maintain this precision
        assert!((info.uptime_seconds - 123456.789012).abs() < 0.000001);
    }

    #[test]
    fn test_uptime_info_debug_format() {
        let info = UptimeInfo {
            uptime_seconds: 3600.5,
            idle_time_seconds: 7200.25,
            boot_time_seconds: 1609459200,
        };

        let debug_string = format!("{:?}", info);
        assert!(debug_string.contains("uptime_seconds"));
        assert!(debug_string.contains("idle_time_seconds"));
        assert!(debug_string.contains("boot_time_seconds"));
    }

    #[test]
    fn test_parse_with_multiple_spaces() {
        // Multiple consecutive spaces between fields
        let content = "3600.0     7200.0";
        let result = parse_uptime_content(content, Some(1609459200));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.uptime_seconds, 3600.0);
        assert_eq!(info.idle_time_seconds, 7200.0);
    }

    #[test]
    fn test_uptime_extreme_long_running_system() {
        // System that has been running for a very long time
        let content = "1000000000.0 4000000000.0"; // ~31.7 years
        let result = parse_uptime_content(content, Some(1609459200 + 1000000000));

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.uptime_seconds > 999999999.0);
    }
}
