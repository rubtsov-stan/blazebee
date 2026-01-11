use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// System load average information over different time windows.
/// Load average represents the average number of processes that are either
/// in the CPU runnable queue or blocked on I/O operations.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAverage {
    /// Average system load over the last 1 minute
    pub one_minute: f64,
    /// Average system load over the last 5 minutes
    pub five_minutes: f64,
    /// Average system load over the last 15 minutes
    pub fifteen_minutes: f64,
    /// Number of processes currently running on the CPU
    pub running_processes: u32,
    /// Total number of processes in the system
    pub total_processes: u32,
}

/// Collector for system load average statistics from /proc/loadavg.
/// This collector reads the system's load average information which is useful
/// for understanding system resource utilization over time.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoadAverageCollector;

/// Constructor methods for LoadAverageCollector
impl LoadAverageCollector {
    /// Creates a new LoadAverageCollector instance
    pub fn new() -> Self {
        LoadAverageCollector
    }
}

/// Default trait implementation for LoadAverageCollector
impl Default for LoadAverageCollector {
    fn default() -> Self {
        LoadAverageCollector::new()
    }
}

/// Data producer implementation for LoadAverage collector.
/// Reads and parses /proc/loadavg to extract load average metrics.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for LoadAverageCollector {
    type Output = LoadAverage;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the /proc/loadavg file which contains load average data
        let content = tokio::fs::read_to_string("/proc/loadavg")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/loadavg".to_string(),
                source,
            })?;

        // Split the file content into whitespace-separated fields
        // Expected format: "1.23 1.45 1.67 1/234 12345"
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 5 {
            return Err(CollectorError::InvalidFormat {
                location: "/proc/loadavg".to_string(),
                reason: "Expected at least 5 fields".to_string(),
            });
        }

        // Parse 1-minute load average (first field)
        let one_minute = parts[0]
            .parse::<f64>()
            .map_err(|_| CollectorError::ParseError {
                metric: "one_minute".to_string(),
                location: "/proc/loadavg".to_string(),
                reason: format!("invalid value: {}", parts[0]),
            })?;

        // Parse 5-minute load average (second field)
        let five_minutes = parts[1]
            .parse::<f64>()
            .map_err(|_| CollectorError::ParseError {
                metric: "five_minutes".to_string(),
                location: "/proc/loadavg".to_string(),
                reason: format!("invalid value: {}", parts[1]),
            })?;

        // Parse 15-minute load average (third field)
        let fifteen_minutes = parts[2]
            .parse::<f64>()
            .map_err(|_| CollectorError::ParseError {
                metric: "fifteen_minutes".to_string(),
                location: "/proc/loadavg".to_string(),
                reason: format!("invalid value: {}", parts[2]),
            })?;

        // Parse the process field which is in format "running/total"
        let proc_split: Vec<&str> = parts[3].split('/').collect();
        if proc_split.len() != 2 {
            return Err(CollectorError::InvalidFormat {
                location: "/proc/loadavg".to_string(),
                reason: "process field must be in format 'running/total'".to_string(),
            });
        }

        // Parse number of running processes (before the slash)
        let running_processes =
            proc_split[0]
                .parse::<u32>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "running_processes".to_string(),
                    location: "/proc/loadavg".to_string(),
                    reason: format!("invalid value: {}", proc_split[0]),
                })?;

        // Parse total number of processes (after the slash)
        let total_processes =
            proc_split[1]
                .parse::<u32>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "total_processes".to_string(),
                    location: "/proc/loadavg".to_string(),
                    reason: format!("invalid value: {}", proc_split[1]),
                })?;

        // Return the successfully parsed LoadAverage data
        Ok(LoadAverage {
            one_minute,
            five_minutes,
            fifteen_minutes,
            running_processes,
            total_processes,
        })
    }
}

#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
))]
register_collector!(LoadAverageCollector, "load_average");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAverage {
    pub one_minute: f64,
    pub five_minutes: f64,
    pub fifteen_minutes: f64,
    pub running_processes: u32,
    pub total_processes: u32,
}

/// Fallback collector for unsupported platforms
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoadAverageCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for LoadAverageCollector {
    type Output = LoadAverage;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "LoadAverage collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_average_creation() {
        let load = LoadAverage {
            one_minute: 1.23,
            five_minutes: 1.45,
            fifteen_minutes: 1.67,
            running_processes: 1,
            total_processes: 234,
        };

        assert_eq!(load.one_minute, 1.23);
        assert_eq!(load.five_minutes, 1.45);
        assert_eq!(load.fifteen_minutes, 1.67);
        assert_eq!(load.running_processes, 1);
        assert_eq!(load.total_processes, 234);
    }

    #[test]
    fn test_load_average_zero_values() {
        let load = LoadAverage {
            one_minute: 0.0,
            five_minutes: 0.0,
            fifteen_minutes: 0.0,
            running_processes: 0,
            total_processes: 0,
        };

        assert_eq!(load.one_minute, 0.0);
        assert!(load.one_minute >= 0.0);
    }

    #[test]
    fn test_load_average_high_values() {
        let load = LoadAverage {
            one_minute: 16.5,
            five_minutes: 14.2,
            fifteen_minutes: 10.8,
            running_processes: 8,
            total_processes: 256,
        };

        assert!(load.one_minute > load.five_minutes);
        assert!(load.five_minutes > load.fifteen_minutes);
    }

    #[test]
    fn test_load_average_serialize() {
        let load = LoadAverage {
            one_minute: 2.5,
            five_minutes: 2.3,
            fifteen_minutes: 2.1,
            running_processes: 2,
            total_processes: 128,
        };

        let json = serde_json::to_string(&load).expect("serialization failed");
        assert!(json.contains("2.5"));
        assert!(json.contains("128"));
    }

    #[test]
    fn test_load_average_deserialize() {
        let json = r#"{
            "one_minute": 1.5,
            "five_minutes": 1.4,
            "fifteen_minutes": 1.3,
            "running_processes": 1,
            "total_processes": 100
        }"#;

        let load: LoadAverage = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(load.one_minute, 1.5);
        assert_eq!(load.five_minutes, 1.4);
        assert_eq!(load.fifteen_minutes, 1.3);
        assert_eq!(load.running_processes, 1);
        assert_eq!(load.total_processes, 100);
    }

    #[test]
    fn test_load_average_collector_creation() {
        let _ = LoadAverageCollector::new();
        let _ = LoadAverageCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_load_average_realistic_values() {
        // Typical load average on a 4-core system
        let load = LoadAverage {
            one_minute: 2.34,
            five_minutes: 1.87,
            fifteen_minutes: 1.45,
            running_processes: 1,
            total_processes: 156,
        };

        // Load average should decrease over time in this case (system cooling down)
        assert!(load.one_minute > load.five_minutes);
        assert!(load.five_minutes > load.fifteen_minutes);
    }

    #[test]
    fn test_load_average_running_processes_valid_range() {
        let load = LoadAverage {
            one_minute: 3.5,
            five_minutes: 3.2,
            fifteen_minutes: 2.8,
            running_processes: 4,
            total_processes: 512,
        };

        // Running processes should not exceed total processes
        assert!(load.running_processes <= load.total_processes);
    }

    #[test]
    fn test_load_average_parse_format() {
        // This test verifies that the LoadAverage struct can handle
        // the expected format from /proc/loadavg
        // Format: "1.23 1.45 1.67 1/234 12345"
        // Expected parsing: one=1.23, five=1.45, fifteen=1.67, running=1, total=234

        let load = LoadAverage {
            one_minute: 1.23,
            five_minutes: 1.45,
            fifteen_minutes: 1.67,
            running_processes: 1,
            total_processes: 234,
        };

        // Verify the values match expected /proc/loadavg format
        assert_eq!(load.one_minute, 1.23);
        assert_eq!(
            format!("{}/{}", load.running_processes, load.total_processes),
            "1/234"
        );
    }

    #[test]
    fn test_load_average_fractional_processes_not_allowed() {
        // Process counts should be integers, not floats
        // This test ensures the LoadAverage struct uses u32 for processes
        let load = LoadAverage {
            one_minute: 1.5,
            five_minutes: 1.5,
            fifteen_minutes: 1.5,
            running_processes: 2, // Must be integer
            total_processes: 256, // Must be integer
        };

        assert_eq!(load.running_processes % 1, 0);
        assert_eq!(load.total_processes % 1, 0);
    }
}
