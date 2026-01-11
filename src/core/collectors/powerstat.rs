use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Power supply statistics.
/// Contains information about batteries, AC adapters, and other power sources.
/// These statistics help monitor power usage, battery health, and power source status.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStats<'a> {
    /// Power supply name (e.g., "BAT0", "AC0", "USB")
    pub name: Cow<'a, str>,
    /// Type of power supply (e.g., "Battery", "Mains", "USB")
    pub supply_type: Cow<'a, str>,
    /// Current status (e.g., "Charging", "Discharging", "Full", "Online")
    pub status: Cow<'a, str>,
    /// Current battery capacity percentage (0-100, None for non-battery sources)
    pub capacity_percent: Option<u8>,
    /// Current energy available in microjoules
    pub energy_now: Option<u64>,
    /// Full energy capacity in microjoules
    pub energy_full: Option<u64>,
    /// Current power draw or charge rate in microwatts
    pub power_now: Option<u64>,
    /// Current voltage in microvolts
    pub voltage_now: Option<u64>,
}

/// Buffer for storing power supply statistics from multiple power sources.
/// Uses Vec internally for efficient storage and iteration.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PowerBuffer<'a> {
    stats: Vec<PowerStats<'a>>,
}

/// Methods for working with the power supply buffer
impl<'a> PowerBuffer<'a> {
    /// Creates a new empty power supply buffer
    pub fn new() -> Self {
        PowerBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of power sources.
    /// Typical capacity would be the number of power supplies available on the system.
    pub fn with_capacity(capacity: usize) -> Self {
        PowerBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds power supply statistics entry to the buffer
    pub fn push(&mut self, stats: PowerStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the power supply statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &PowerStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of power supply entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// The Power Supply collector that reads from /sys/class/power_supply on Linux systems.
/// Collects information about batteries, AC adapters, and power management.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
pub struct PowerCollector;

/// Constructor methods for PowerCollector
impl PowerCollector {
    /// Creates a new power supply collector instance
    pub fn new() -> Self {
        PowerCollector
    }
}

/// Default trait implementation for PowerCollector
impl Default for PowerCollector {
    fn default() -> Self {
        PowerCollector::new()
    }
}

/// Data producer implementation for Power Supply collector.
/// Reads from /sys/class/power_supply and parses information about all power sources.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for PowerCollector {
    type Output = PowerBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        use tokio::fs;

        // Initialize buffer with capacity for up to 4 power sources (typical for laptops)
        let mut buffer = PowerBuffer::with_capacity(4);
        let power_path = "/sys/class/power_supply";

        // Read all power supply entries in /sys/class/power_supply
        let mut entries =
            fs::read_dir(power_path)
                .await
                .map_err(|source| CollectorError::FileRead {
                    path: power_path.to_string(),
                    source,
                })?;

        // Process each power supply entry
        while let Some(entry) =
            entries
                .next_entry()
                .await
                .map_err(|source| CollectorError::FileRead {
                    path: power_path.to_string(),
                    source,
                })?
        {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Helper function to read and parse a file
            async fn read_file<P: AsRef<std::path::Path>>(path: P, file: &str) -> Option<String> {
                tokio::fs::read_to_string(path.as_ref().join(file))
                    .await
                    .ok()
                    .map(|s| s.trim().to_string())
            }

            // Read various power supply attributes
            let supply_type = read_file(&path, "type").await.unwrap_or_default();
            let status = read_file(&path, "status").await.unwrap_or_default();
            let capacity_percent = read_file(&path, "capacity")
                .await
                .and_then(|s| s.parse::<u8>().ok());
            let energy_now = read_file(&path, "energy_now")
                .await
                .and_then(|s| s.parse::<u64>().ok());
            let energy_full = read_file(&path, "energy_full")
                .await
                .and_then(|s| s.parse::<u64>().ok());
            let power_now = read_file(&path, "power_now")
                .await
                .and_then(|s| s.parse::<u64>().ok());
            let voltage_now = read_file(&path, "voltage_now")
                .await
                .and_then(|s| s.parse::<u64>().ok());

            // Create power supply statistics entry with the parsed values
            buffer.push(PowerStats {
                name: Cow::Owned(name),
                supply_type: Cow::Owned(supply_type),
                status: Cow::Owned(status),
                capacity_percent,
                energy_now,
                energy_full,
                power_now,
                voltage_now,
            });
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
register_collector!(PowerCollector, "power");

/// Fallback power supply statistics for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStats<'a> {
    /// Power supply name (e.g., "BAT0", "AC0", "USB")
    pub name: Cow<'a, str>,
    /// Type of power supply (e.g., "Battery", "Mains", "USB")
    pub supply_type: Cow<'a, str>,
    /// Current status (e.g., "Charging", "Discharging", "Full", "Online")
    pub status: Cow<'a, str>,
    /// Current battery capacity percentage (0-100, None for non-battery sources)
    pub capacity_percent: Option<u8>,
    /// Current energy available in microjoules
    pub energy_now: Option<u64>,
    /// Full energy capacity in microjoules
    pub energy_full: Option<u64>,
    /// Current power draw or charge rate in microwatts
    pub power_now: Option<u64>,
    /// Current voltage in microvolts
    pub voltage_now: Option<u64>,
}

/// Fallback buffer for storing power supply statistics
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PowerBuffer<'a> {
    stats: Vec<PowerStats<'a>>,
}

/// Fallback methods for power supply buffer
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
impl<'a> PowerBuffer<'a> {
    /// Creates a new empty power supply buffer
    pub fn new() -> Self {
        PowerBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory
    pub fn with_capacity(capacity: usize) -> Self {
        PowerBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds power supply statistics entry to the buffer
    pub fn push(&mut self, stats: PowerStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the power supply statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &PowerStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of power supply entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// Fallback power supply collector for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
pub struct PowerCollector;

/// Fallback constructor methods for PowerCollector
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
impl PowerCollector {
    /// Creates a new power supply collector instance
    pub fn new() -> Self {
        PowerCollector
    }
}

/// Fallback Default trait implementation for PowerCollector
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
impl Default for PowerCollector {
    fn default() -> Self {
        PowerCollector::new()
    }
}

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for PowerCollector {
    type Output = PowerBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Power supply collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Power Supply tests
    #[test]
    fn test_power_stats_creation() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Discharging"),
            capacity_percent: Some(75),
            energy_now: Some(50_000_000),
            energy_full: Some(60_000_000),
            power_now: Some(10_000_000),
            voltage_now: Some(12_000_000),
        };

        assert_eq!(power_stats.name, "BAT0");
        assert_eq!(power_stats.status, "Discharging");
        assert_eq!(power_stats.capacity_percent, Some(75));
        assert_eq!(power_stats.supply_type, "Battery");
    }

    #[test]
    fn test_power_buffer() {
        let mut buffer = PowerBuffer::new();

        let power_stats = PowerStats {
            name: Cow::Borrowed("AC0"),
            supply_type: Cow::Borrowed("Mains"),
            status: Cow::Borrowed("Online"),
            capacity_percent: None,
            energy_now: None,
            energy_full: None,
            power_now: None,
            voltage_now: Some(220_000_000),
        };

        buffer.push(power_stats);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_power_battery_charging() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Charging"),
            capacity_percent: Some(50),
            energy_now: Some(30_000_000),
            energy_full: Some(60_000_000),
            power_now: Some(15_000_000),
            voltage_now: Some(12_500_000),
        };

        assert_eq!(power_stats.status, "Charging");
        assert!(power_stats.capacity_percent.unwrap() < 100);
        // Power should be positive when charging
        assert!(power_stats.power_now.unwrap() > 0);
    }

    #[test]
    fn test_power_battery_full() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Full"),
            capacity_percent: Some(100),
            energy_now: Some(60_000_000),
            energy_full: Some(60_000_000),
            power_now: Some(0),
            voltage_now: Some(12_600_000),
        };

        assert_eq!(power_stats.status, "Full");
        assert_eq!(power_stats.capacity_percent, Some(100));
        assert_eq!(power_stats.energy_now, power_stats.energy_full);
    }

    #[test]
    fn test_power_stats_serialize() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Discharging"),
            capacity_percent: Some(80),
            energy_now: Some(40_000_000),
            energy_full: Some(50_000_000),
            power_now: Some(8_000_000),
            voltage_now: Some(11_800_000),
        };

        let json = serde_json::to_string(&power_stats).expect("serialization failed");
        assert!(json.contains("BAT0"));
        assert!(json.contains("Battery"));
        assert!(json.contains("80"));
        assert!(json.contains("Discharging"));
    }

    #[test]
    fn test_power_usb_supply() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("USB1"),
            supply_type: Cow::Borrowed("USB"),
            status: Cow::Borrowed("Online"),
            capacity_percent: None,
            energy_now: None,
            energy_full: None,
            power_now: Some(5_000_000),
            voltage_now: Some(5_000_000),
        };

        assert_eq!(power_stats.supply_type, "USB");
        assert_eq!(power_stats.status, "Online");
        assert!(power_stats.capacity_percent.is_none()); // USB doesn't have capacity
    }

    #[test]
    fn test_power_buffer_multiple_sources() {
        let mut buffer = PowerBuffer::with_capacity(3);

        let sources = vec![
            PowerStats {
                name: Cow::Borrowed("BAT0"),
                supply_type: Cow::Borrowed("Battery"),
                status: Cow::Borrowed("Charging"),
                capacity_percent: Some(60),
                energy_now: Some(36_000_000),
                energy_full: Some(60_000_000),
                power_now: Some(12_000_000),
                voltage_now: Some(12_200_000),
            },
            PowerStats {
                name: Cow::Borrowed("AC0"),
                supply_type: Cow::Borrowed("Mains"),
                status: Cow::Borrowed("Online"),
                capacity_percent: None,
                energy_now: None,
                energy_full: None,
                power_now: Some(45_000_000),
                voltage_now: Some(230_000_000),
            },
            PowerStats {
                name: Cow::Borrowed("USB0"),
                supply_type: Cow::Borrowed("USB"),
                status: Cow::Borrowed("Disconnected"),
                capacity_percent: None,
                energy_now: None,
                energy_full: None,
                power_now: None,
                voltage_now: None,
            },
        ];

        for source in sources {
            buffer.push(source);
        }

        assert_eq!(buffer.len(), 3);
        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_power_stats_deserialize() {
        let json = r#"{
            "name": "BAT1",
            "supply_type": "Battery",
            "status": "Discharging",
            "capacity_percent": 42,
            "energy_now": 21000000,
            "energy_full": 50000000,
            "power_now": 7500000,
            "voltage_now": 11600000
        }"#;

        let power_stats: PowerStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(power_stats.name, "BAT1");
        assert_eq!(power_stats.supply_type, "Battery");
        assert_eq!(power_stats.status, "Discharging");
        assert_eq!(power_stats.capacity_percent, Some(42));
        assert_eq!(power_stats.energy_now, Some(21_000_000));
    }

    #[test]
    fn test_power_battery_critical() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Discharging"),
            capacity_percent: Some(5), // Critical level
            energy_now: Some(3_000_000),
            energy_full: Some(60_000_000),
            power_now: Some(8_000_000),
            voltage_now: Some(11_000_000),
        };

        assert_eq!(power_stats.capacity_percent, Some(5));
        assert_eq!(power_stats.status, "Discharging");
        // Critical battery level
        assert!(power_stats.capacity_percent.unwrap() < 10);
    }

    #[test]
    fn test_power_collector_creation() {
        let _ = PowerCollector::new();
        let _ = PowerCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_power_buffer_clone() {
        let mut buffer1 = PowerBuffer::new();

        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Charging"),
            capacity_percent: Some(50),
            energy_now: Some(30_000_000),
            energy_full: Some(60_000_000),
            power_now: Some(15_000_000),
            voltage_now: Some(12_200_000),
        };

        buffer1.push(power_stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
        assert_eq!(buffer1.len(), 1);
    }

    #[test]
    fn test_power_stats_none_values() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("AC0"),
            supply_type: Cow::Borrowed("Mains"),
            status: Cow::Borrowed("Online"),
            capacity_percent: None,
            energy_now: None,
            energy_full: None,
            power_now: None,
            voltage_now: None,
        };

        // AC adapters typically don't have battery metrics
        assert!(power_stats.capacity_percent.is_none());
        assert!(power_stats.energy_now.is_none());
        assert!(power_stats.energy_full.is_none());
        assert!(power_stats.power_now.is_none());
        assert!(power_stats.voltage_now.is_none());
    }

    #[test]
    fn test_power_remaining_time_calculation() {
        let power_stats = PowerStats {
            name: Cow::Borrowed("BAT0"),
            supply_type: Cow::Borrowed("Battery"),
            status: Cow::Borrowed("Discharging"),
            capacity_percent: Some(50),
            energy_now: Some(30_000_000),  // 30 watt-hours
            energy_full: Some(60_000_000), // 60 watt-hours
            power_now: Some(15_000_000),   // 15 watts
            voltage_now: Some(12_000_000), // 12 volts
        };

        // Approximate remaining time in hours = energy_now / power_now
        if let (Some(energy), Some(power)) = (power_stats.energy_now, power_stats.power_now) {
            let remaining_hours = energy as f64 / power as f64;
            assert_eq!(remaining_hours, 2.0); // 30 watt-hours / 15 watts = 2 hours
        }
    }
}
