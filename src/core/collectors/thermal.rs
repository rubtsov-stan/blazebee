use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// ============================================================================
// LINUX-SPECIFIC TYPES AND IMPLEMENTATIONS
// ============================================================================

/// Represents temperature information for a single thermal zone on the system.
/// A thermal zone is typically a distinct temperature sensor, such as a CPU core,
/// GPU, hard drive, or chassis sensor. Temperatures are expressed in Celsius and
/// can be monitored to detect overheating conditions or thermal throttling events.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZoneInfo<'a> {
    /// Unique identifier for the thermal zone (e.g., "thermal_zone0", "thermal_zone1").
    /// Each thermal zone corresponds to a separate temperature sensor in the kernel's
    /// thermal management subsystem. The name is derived from the directory name in
    /// /sys/class/thermal.
    pub zone_id: Cow<'a, str>,
    /// Current temperature reading in Celsius. Values are converted from millidegrees
    /// (as stored in the kernel) to degrees by dividing by 1000. Negative values may
    /// indicate sensor errors or unsupported zones. Monitor for unusual spikes or
    /// sustained high temperatures that may indicate thermal issues.
    pub temperature: i64,
}

/// Container for thermal zone temperature readings from all available sensors.
/// This buffer collects temperature data from all thermal zones on the system and
/// provides convenient access through standard collection methods. Data comes from
/// /sys/class/thermal which exposes kernel thermal sensor information.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThermalBuffer<'a> {
    zones: Vec<ThermalZoneInfo<'a>>,
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
impl<'a> ThermalBuffer<'a> {
    /// Creates an empty thermal buffer with no pre-allocated capacity.
    pub fn new() -> Self {
        ThermalBuffer { zones: Vec::new() }
    }

    /// Creates an empty thermal buffer with pre-allocated capacity.
    /// Useful when the number of thermal zones is known in advance.
    pub fn with_capacity(capacity: usize) -> Self {
        ThermalBuffer {
            zones: Vec::with_capacity(capacity),
        }
    }

    /// Appends a thermal zone temperature reading to the buffer.
    pub fn push(&mut self, zone: ThermalZoneInfo<'a>) {
        self.zones.push(zone);
    }

    /// Returns an iterator over all thermal zone readings in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &ThermalZoneInfo<'a>> {
        self.zones.iter()
    }

    /// Returns the number of thermal zones whose readings are stored in the buffer.
    pub fn len(&self) -> usize {
        self.zones.len()
    }

    /// Checks if the buffer contains any thermal zone readings.
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }

    /// Returns the thermal zone readings as a slice for direct array access.
    pub fn as_slice(&self) -> &[ThermalZoneInfo<'a>] {
        &self.zones
    }
}

/// The main collector responsible for gathering temperature data from all thermal zones.
/// Reads from /sys/class/thermal which exposes kernel thermal sensor information for
/// hardware temperature monitoring. This collector dynamically discovers all available
/// thermal zones and reads their current temperatures. Essential for system health
/// monitoring, detecting overheating, and thermal throttling events.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
pub struct ThermalZoneCollector;

impl ThermalZoneCollector {
    pub fn new() -> Self {
        ThermalZoneCollector
    }
}

impl Default for ThermalZoneCollector {
    fn default() -> Self {
        ThermalZoneCollector::new()
    }
}

/// Implementation of the DataProducer trait for asynchronously collecting thermal zone temperatures.
/// Reads from /sys/class/thermal which is a sysfs interface to the kernel's thermal subsystem.
/// The collector enumerates all directories matching "thermal_zone*" and reads temperature
/// files for each. Temperature values are stored in millidegrees and converted to degrees.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for ThermalZoneCollector {
    type Output = ThermalBuffer<'static>;

    /// Asynchronously reads temperature data for all thermal zones.
    ///
    /// The method enumerates /sys/class/thermal for directories matching "thermal_zone*" pattern.
    /// For each thermal zone found, it attempts to read the "temp" file which contains the
    /// temperature in millidegrees Celsius. The temperature is divided by 1000 to convert
    /// to degrees.
    ///
    /// File structure:
    /// ```text
    /// /sys/class/thermal/
    ///   thermal_zone0/
    ///     temp          <- contains temperature in millidegrees (e.g., "45000" for 45°C)
    ///   thermal_zone1/
    ///     temp          <- another temperature sensor
    /// ```
    ///
    /// Errors reading individual temperature files are silently ignored to maintain
    /// robustness—some thermal zones may not have readable temperature files on certain systems.
    /// A directory read error returns early with an error, but individual temperature read
    /// failures are skipped. Parse errors on valid temperature values are also silently ignored.
    async fn produce(&self) -> CollectorResult<Self::Output> {
        let mut buffer = ThermalBuffer::new();

        match tokio::fs::read_dir("/sys/class/thermal").await {
            Ok(mut entries) => {
                // Iterate over all entries in /sys/class/thermal
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(name) = entry.file_name().into_string() {
                        // Only process directories matching "thermal_zone*"
                        if name.starts_with("thermal_zone") {
                            let path = format!("/sys/class/thermal/{}/temp", name);
                            match tokio::fs::read_to_string(path).await {
                                Ok(temp_str) => {
                                    // Parse the temperature value (in millidegrees)
                                    if let Ok(temp) = temp_str.trim().parse::<i64>() {
                                        // Convert from millidegrees to degrees
                                        buffer.push(ThermalZoneInfo {
                                            zone_id: Cow::Owned(name),
                                            temperature: temp / 1000,
                                        });
                                    }
                                    // Parse errors are silently ignored
                                }
                                Err(_) => {
                                    // Temperature file read errors are silently ignored
                                    // Some zones may not have readable temperature files
                                }
                            }
                        }
                    }
                }
            }
            Err(source) => {
                // Directory read failure is a hard error
                return Err(CollectorError::FileRead {
                    path: "/sys/class/thermal".to_string(),
                    source,
                });
            }
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
register_collector!(ThermalZoneCollector, "thermal");

// ============================================================================
// FALLBACK IMPLEMENTATIONS FOR UNSUPPORTED PLATFORMS
// ============================================================================

/// Fallback type definition for non-Linux platforms or when the collector is not enabled.
/// Maintains the same structure as the Linux version to provide API compatibility.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZoneInfo<'a> {
    pub zone_id: Cow<'a, str>,
    pub temperature: i64,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThermalBuffer<'a> {
    zones: Vec<ThermalZoneInfo<'a>>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
)))]
impl<'a> ThermalBuffer<'a> {
    pub fn new() -> Self {
        ThermalBuffer { zones: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        ThermalBuffer {
            zones: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, zone: ThermalZoneInfo<'a>) {
        self.zones.push(zone);
    }

    pub fn iter(&self) -> impl Iterator<Item = &ThermalZoneInfo<'a>> {
        self.zones.iter()
    }

    pub fn len(&self) -> usize {
        self.zones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }

    pub fn as_slice(&self) -> &[ThermalZoneInfo<'a>] {
        &self.zones
    }
}

/// Fallback collector for unsupported platforms. Provides the same interface but always
/// returns an error indicating that thermal zone monitoring is not available on this platform.
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
)))]
pub struct ThermalZoneCollector;

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for ThermalZoneCollector {
    type Output = ThermalBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Thermal Zone collector not enabled or not supported on this platform".to_string(),
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

    // ---- Helper function for creating test zones ----

    fn create_zone(id: &str, temp: i64) -> ThermalZoneInfo<'static> {
        ThermalZoneInfo {
            zone_id: Cow::Owned(id.to_string()),
            temperature: temp,
        }
    }

    // ---- Test cases ----

    #[test]
    fn test_thermal_zone_info_creation() {
        let zone = create_zone("thermal_zone0", 45);
        assert_eq!(zone.zone_id, "thermal_zone0");
        assert_eq!(zone.temperature, 45);
    }

    #[test]
    fn test_thermal_buffer_creation() {
        let buffer = ThermalBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_thermal_buffer_with_capacity() {
        let buffer = ThermalBuffer::with_capacity(10);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_thermal_buffer_push_and_iter() {
        let mut buffer = ThermalBuffer::new();

        buffer.push(create_zone("thermal_zone0", 45));
        buffer.push(create_zone("thermal_zone1", 62));
        buffer.push(create_zone("thermal_zone2", 51));

        assert_eq!(buffer.len(), 3);
        assert!(!buffer.is_empty());

        let temps: Vec<_> = buffer.iter().map(|z| z.temperature).collect();
        assert_eq!(temps, vec![45, 62, 51]);
    }

    #[test]
    fn test_thermal_buffer_as_slice() {
        let mut buffer = ThermalBuffer::new();

        buffer.push(create_zone("thermal_zone0", 40));
        buffer.push(create_zone("thermal_zone1", 55));

        let slice = buffer.as_slice();
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0].zone_id, "thermal_zone0");
        assert_eq!(slice[1].temperature, 55);
    }

    #[test]
    fn test_thermal_buffer_multiple_zones() {
        let mut buffer = ThermalBuffer::new();

        // Simulate a system with multiple thermal zones
        for i in 0..8 {
            let temp = 40 + (i as i64 * 2);
            buffer.push(create_zone(&format!("thermal_zone{}", i), temp));
        }

        assert_eq!(buffer.len(), 8);
        let zones = buffer.as_slice();
        assert_eq!(zones[0].temperature, 40);
        assert_eq!(zones[7].temperature, 54);
    }

    #[test]
    fn test_thermal_zone_zero_temperature() {
        let zone = create_zone("thermal_zone0", 0);
        assert_eq!(zone.temperature, 0);
    }

    #[test]
    fn test_thermal_zone_negative_temperature() {
        // Some sensors may report negative values (sensor errors or unsupported zones)
        let zone = create_zone("thermal_zone0", -5);
        assert_eq!(zone.temperature, -5);
    }

    #[test]
    fn test_thermal_zone_high_temperature() {
        // Test with high temperature values
        let zone = create_zone("thermal_zone0", 120);
        assert_eq!(zone.temperature, 120);
    }

    #[test]
    fn test_thermal_zone_extreme_temperatures() {
        // Test with extreme values
        let cold = create_zone("thermal_zone0", -100);
        let hot = create_zone("thermal_zone1", 300);

        assert_eq!(cold.temperature, -100);
        assert_eq!(hot.temperature, 300);
    }

    #[test]
    fn test_thermal_zone_i64_boundaries() {
        // Test near i64 boundaries
        let max_zone = ThermalZoneInfo {
            zone_id: Cow::Borrowed("thermal_zone_max"),
            temperature: i64::MAX,
        };

        let min_zone = ThermalZoneInfo {
            zone_id: Cow::Borrowed("thermal_zone_min"),
            temperature: i64::MIN,
        };

        assert_eq!(max_zone.temperature, i64::MAX);
        assert_eq!(min_zone.temperature, i64::MIN);
    }

    #[test]
    fn test_thermal_zone_serialization() {
        let zone = create_zone("thermal_zone0", 65);

        // Verify JSON serialization works
        let json = serde_json::to_string(&zone);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<ThermalZoneInfo, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.zone_id, "thermal_zone0");
        assert_eq!(restored.temperature, 65);
    }

    #[test]
    fn test_thermal_zone_clone() {
        let original = create_zone("thermal_zone5", 78);
        let cloned = original.clone();

        assert_eq!(original.zone_id, cloned.zone_id);
        assert_eq!(original.temperature, cloned.temperature);
    }

    #[test]
    fn test_thermal_buffer_serialization() {
        let mut buffer = ThermalBuffer::new();
        buffer.push(create_zone("thermal_zone0", 45));
        buffer.push(create_zone("thermal_zone1", 60));

        let json = serde_json::to_string(&buffer);
        assert!(json.is_ok());

        let serialized = json.unwrap();
        let deserialized: Result<ThermalBuffer, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.as_slice()[0].temperature, 45);
    }

    #[test]
    fn test_thermal_buffer_clone() {
        let mut original = ThermalBuffer::new();
        original.push(create_zone("thermal_zone0", 50));
        original.push(create_zone("thermal_zone1", 70));

        let cloned = original.clone();

        assert_eq!(original.len(), cloned.len());
        assert_eq!(cloned.as_slice()[0].temperature, 50);
    }

    #[test]
    fn test_thermal_collector_creation() {
        let collector = ThermalZoneCollector::new();
        let default_collector = ThermalZoneCollector::default();

        let _ = (collector, default_collector);
    }

    #[test]
    fn test_thermal_zone_name_variations() {
        // Test various thermal zone naming patterns
        let zones = vec![
            create_zone("thermal_zone0", 40),
            create_zone("thermal_zone10", 50),
            create_zone("thermal_zone99", 60),
            create_zone("thermal_zone_custom", 45),
        ];

        assert_eq!(zones.len(), 4);
        assert_eq!(zones[0].zone_id, "thermal_zone0");
        assert_eq!(zones[3].zone_id, "thermal_zone_custom");
    }

    #[test]
    fn test_thermal_zone_default_threshold_detection() {
        // Scenario: Detecting overheating using common thermal thresholds
        let normal_zone = create_zone("thermal_zone0", 45);
        let warm_zone = create_zone("thermal_zone1", 75);
        let hot_zone = create_zone("thermal_zone2", 95);

        let throttle_threshold = 80; // Common throttling point for CPUs
        let critical_threshold = 100;

        assert!(normal_zone.temperature < throttle_threshold);
        assert!(warm_zone.temperature < throttle_threshold);
        assert!(hot_zone.temperature < critical_threshold);
    }

    #[test]
    fn test_thermal_buffer_find_hottest() {
        // Scenario: Finding the hottest thermal zone
        let mut buffer = ThermalBuffer::new();
        buffer.push(create_zone("thermal_zone0", 45));
        buffer.push(create_zone("thermal_zone1", 72));
        buffer.push(create_zone("thermal_zone2", 38));
        buffer.push(create_zone("thermal_zone3", 68));

        let hottest = buffer
            .iter()
            .max_by_key(|z| z.temperature)
            .map(|z| (z.zone_id.as_ref(), z.temperature));

        assert_eq!(hottest, Some(("thermal_zone1", 72)));
    }

    #[test]
    fn test_thermal_buffer_find_coldest() {
        // Scenario: Finding the coldest thermal zone
        let mut buffer = ThermalBuffer::new();
        buffer.push(create_zone("thermal_zone0", 50));
        buffer.push(create_zone("thermal_zone1", 35));
        buffer.push(create_zone("thermal_zone2", 62));

        let coldest = buffer
            .iter()
            .min_by_key(|z| z.temperature)
            .map(|z| z.temperature);

        assert_eq!(coldest, Some(35));
    }

    #[test]
    fn test_thermal_buffer_average_temperature() {
        // Scenario: Computing average system temperature
        let mut buffer = ThermalBuffer::new();
        buffer.push(create_zone("thermal_zone0", 40));
        buffer.push(create_zone("thermal_zone1", 50));
        buffer.push(create_zone("thermal_zone2", 60));

        let avg: i64 = buffer.iter().map(|z| z.temperature).sum::<i64>() / buffer.len() as i64;

        assert_eq!(avg, 50);
    }

    #[test]
    fn test_thermal_buffer_temperature_range() {
        // Scenario: Finding temperature range (delta)
        let mut buffer = ThermalBuffer::new();
        buffer.push(create_zone("thermal_zone0", 30));
        buffer.push(create_zone("thermal_zone1", 85));
        buffer.push(create_zone("thermal_zone2", 55));

        let max_temp = buffer.iter().map(|z| z.temperature).max().unwrap();
        let min_temp = buffer.iter().map(|z| z.temperature).min().unwrap();
        let range = max_temp - min_temp;

        assert_eq!(range, 55);
    }

    #[test]
    fn test_thermal_zone_debug_format() {
        let zone = create_zone("thermal_zone0", 58);
        let debug_string = format!("{:?}", zone);

        assert!(debug_string.contains("zone_id"));
        assert!(debug_string.contains("temperature"));
        assert!(debug_string.contains("thermal_zone0"));
    }

    #[test]
    fn test_thermal_buffer_filter_by_temperature() {
        // Scenario: Finding zones above a certain temperature threshold
        let mut buffer = ThermalBuffer::new();
        buffer.push(create_zone("thermal_zone0", 35));
        buffer.push(create_zone("thermal_zone1", 65));
        buffer.push(create_zone("thermal_zone2", 48));
        buffer.push(create_zone("thermal_zone3", 72));

        let threshold = 60;
        let hot_zones: Vec<_> = buffer
            .iter()
            .filter(|z| z.temperature >= threshold)
            .collect();

        assert_eq!(hot_zones.len(), 2);
        assert_eq!(hot_zones[0].temperature, 65);
        assert_eq!(hot_zones[1].temperature, 72);
    }

    #[test]
    fn test_thermal_zone_temperature_conversion() {
        // Verify temperature unit handling is consistent
        // If raw values were in millidegrees (1000 = 1°C)
        let temp_millidegrees = 45000;
        let temp_celsius = temp_millidegrees / 1000;

        assert_eq!(temp_celsius, 45);
    }

    #[test]
    fn test_thermal_buffer_capacity_efficiency() {
        // Verify pre-allocation efficiency
        let mut buffer = ThermalBuffer::with_capacity(100);

        for i in 0..50 {
            buffer.push(create_zone(&format!("thermal_zone{}", i), 40 + i as i64));
        }

        assert_eq!(buffer.len(), 50);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_thermal_zone_info_borrowed_vs_owned() {
        // Test both borrowed and owned variants
        let borrowed = ThermalZoneInfo {
            zone_id: Cow::Borrowed("thermal_zone0"),
            temperature: 50,
        };

        let owned = ThermalZoneInfo {
            zone_id: Cow::Owned("thermal_zone1".to_string()),
            temperature: 60,
        };

        assert_eq!(borrowed.zone_id, "thermal_zone0");
        assert_eq!(owned.zone_id, "thermal_zone1");
    }

    #[test]
    fn test_thermal_buffer_multi_socket_system() {
        // Scenario: Multi-socket system with multiple thermal zones per socket
        let mut buffer = ThermalBuffer::new();

        // Socket 0 thermal zones
        buffer.push(create_zone("thermal_zone0", 52));
        buffer.push(create_zone("thermal_zone1", 55));

        // Socket 1 thermal zones
        buffer.push(create_zone("thermal_zone2", 48));
        buffer.push(create_zone("thermal_zone3", 51));

        assert_eq!(buffer.len(), 4);

        // Check socket 0 average
        let socket0_avg: i64 = buffer.iter().take(2).map(|z| z.temperature).sum::<i64>() / 2;
        assert_eq!(socket0_avg, 53);
    }
}
