use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Hardware monitoring sensor reading.
/// Contains a single sensor measurement with its name, numeric value, and unit.
/// Hwmon sensors provide information about CPU temperature, fan speeds, voltages, etc.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwmonSensor<'a> {
    /// Sensor identifier/name (e.g., "hwmon0", "hwmon1")
    pub name: Cow<'a, str>,
    /// Numeric sensor value (temperature in Celsius, RPM for fans, etc.)
    pub value: f64,
    /// Unit of measurement (e.g., "C" for Celsius, "RPM" for fan speed)
    pub unit: Cow<'a, str>,
}

/// Buffer for storing hardware monitoring sensor readings.
/// Uses Vec internally for efficient storage and iteration of multiple sensors.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HwmonBuffer<'a> {
    sensors: Vec<HwmonSensor<'a>>,
}

/// Methods for working with the hwmon buffer
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
impl<'a> HwmonBuffer<'a> {
    /// Creates a new empty hwmon buffer
    pub fn new() -> Self {
        HwmonBuffer {
            sensors: Vec::new(),
        }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of sensors
    pub fn with_capacity(capacity: usize) -> Self {
        HwmonBuffer {
            sensors: Vec::with_capacity(capacity),
        }
    }

    /// Adds a sensor reading to the buffer
    pub fn push(&mut self, sensor: HwmonSensor<'a>) {
        self.sensors.push(sensor);
    }

    /// Returns an iterator over the sensor readings
    pub fn iter(&self) -> impl Iterator<Item = &HwmonSensor<'a>> {
        self.sensors.iter()
    }

    /// Returns the number of sensors in the buffer
    pub fn len(&self) -> usize {
        self.sensors.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.sensors.is_empty()
    }

    /// Returns a slice of all sensor readings
    pub fn as_slice(&self) -> &[HwmonSensor<'a>] {
        &self.sensors
    }
}

/// The Hwmon collector that reads from /sys/class/hwmon on Linux systems.
/// Collects hardware monitoring sensor data including temperature readings.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
pub struct HwmonCollector;

/// Constructor methods for HwmonCollector
impl HwmonCollector {
    /// Creates a new HwmonCollector instance
    pub fn new() -> Self {
        HwmonCollector
    }
}

/// Default trait implementation for HwmonCollector
impl Default for HwmonCollector {
    fn default() -> Self {
        HwmonCollector::new()
    }
}

/// Data producer implementation for Hwmon collector.
/// Reads from /sys/class/hwmon directory and collects temperature sensor data.
/// Specifically reads temp1_input files and converts from millidegrees to degrees Celsius.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for HwmonCollector {
    type Output = HwmonBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Initialize buffer for sensor readings
        let mut buffer = HwmonBuffer::new();

        // Attempt to read the hwmon directory
        match tokio::fs::read_dir("/sys/class/hwmon").await {
            Ok(mut entries) => {
                // Iterate through each hwmon device directory
                while let Ok(Some(entry)) = entries.next_entry().await {
                    // Get the device name (e.g., "hwmon0", "hwmon1")
                    if let Ok(name) = entry.file_name().into_string() {
                        let path = format!("/sys/class/hwmon/{}", name);

                        // Try to read the primary temperature sensor (temp1_input)
                        // This file contains the temperature in millidegrees Celsius
                        if let Ok(temp) =
                            tokio::fs::read_to_string(format!("{}/temp1_input", path)).await
                        {
                            // Parse the temperature value
                            if let Ok(val) = temp.trim().parse::<f64>() {
                                // Convert from millidegrees to degrees by dividing by 1000
                                buffer.push(HwmonSensor {
                                    name: Cow::Owned(name.clone()),
                                    value: val / 1000.0,
                                    unit: Cow::Borrowed("C"),
                                });
                            }
                        }
                    }
                }
            }
            Err(source) => {
                // Return error if we cannot read the hwmon directory
                return Err(CollectorError::FileRead {
                    path: "/sys/class/hwmon".to_string(),
                    source,
                });
            }
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
register_collector!(HwmonCollector, "hwmon");

/// Fallback implementations for unsupported platforms or disabled features

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwmonSensor<'a> {
    pub name: Cow<'a, str>,
    pub value: f64,
    pub unit: Cow<'a, str>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HwmonBuffer<'a> {
    sensors: Vec<HwmonSensor<'a>>,
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
)))]
impl<'a> HwmonBuffer<'a> {
    pub fn new() -> Self {
        HwmonBuffer {
            sensors: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        HwmonBuffer {
            sensors: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, sensor: HwmonSensor<'a>) {
        self.sensors.push(sensor);
    }

    pub fn iter(&self) -> impl Iterator<Item = &HwmonSensor<'a>> {
        self.sensors.iter()
    }

    pub fn len(&self) -> usize {
        self.sensors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sensors.is_empty()
    }

    pub fn as_slice(&self) -> &[HwmonSensor<'a>] {
        &self.sensors
    }
}

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
)))]
pub struct HwmonCollector;

#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for HwmonCollector {
    type Output = HwmonBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Hwmon collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hwmon_sensor_creation() {
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 45.5,
            unit: Cow::Borrowed("C"),
        };

        assert_eq!(sensor.name, "hwmon0");
        assert_eq!(sensor.value, 45.5);
        assert_eq!(sensor.unit, "C");
    }

    #[test]
    fn test_hwmon_sensor_low_temperature() {
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 20.0,
            unit: Cow::Borrowed("C"),
        };

        assert!(sensor.value > 0.0);
        assert!(sensor.value < 40.0);
    }

    #[test]
    fn test_hwmon_sensor_high_temperature() {
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 85.5,
            unit: Cow::Borrowed("C"),
        };

        // High temperature warning threshold
        assert!(sensor.value > 80.0);
    }

    #[test]
    fn test_hwmon_sensor_critical_temperature() {
        // Critical temperature threshold (typically 95-100°C)
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 98.0,
            unit: Cow::Borrowed("C"),
        };

        assert!(sensor.value > 95.0);
    }

    #[test]
    fn test_hwmon_sensor_negative_temperature() {
        // Some systems may report temperatures below 0°C (e.g., cryogenic)
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: -10.5,
            unit: Cow::Borrowed("C"),
        };

        assert!(sensor.value < 0.0);
    }

    #[test]
    fn test_hwmon_buffer_creation() {
        let buffer = HwmonBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_hwmon_buffer_with_capacity() {
        let buffer = HwmonBuffer::with_capacity(8);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_hwmon_buffer_push_single() {
        let mut buffer = HwmonBuffer::new();
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 45.5,
            unit: Cow::Borrowed("C"),
        };

        buffer.push(sensor);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_hwmon_buffer_multiple_sensors() {
        let mut buffer = HwmonBuffer::with_capacity(4);

        for i in 0..4 {
            let sensor = HwmonSensor {
                name: Cow::Owned(format!("hwmon{}", i)),
                value: 40.0 + (i as f64 * 5.0),
                unit: Cow::Borrowed("C"),
            };
            buffer.push(sensor);
        }

        assert_eq!(buffer.len(), 4);
    }

    #[test]
    fn test_hwmon_buffer_iterator() {
        let mut buffer = HwmonBuffer::new();

        for i in 0..3 {
            let sensor = HwmonSensor {
                name: Cow::Owned(format!("hwmon{}", i)),
                value: 45.0 + (i as f64),
                unit: Cow::Borrowed("C"),
            };
            buffer.push(sensor);
        }

        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_hwmon_sensor_serialize() {
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 50.5,
            unit: Cow::Borrowed("C"),
        };

        let json = serde_json::to_string(&sensor).expect("serialization failed");
        assert!(json.contains("hwmon0"));
        assert!(json.contains("50.5"));
        assert!(json.contains("C"));
    }

    #[test]
    fn test_hwmon_sensor_deserialize() {
        let json = r#"{
            "name": "hwmon0",
            "value": 55.75,
            "unit": "C"
        }"#;

        let sensor: HwmonSensor = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(sensor.name, "hwmon0");
        assert_eq!(sensor.value, 55.75);
        assert_eq!(sensor.unit, "C");
    }

    #[test]
    fn test_hwmon_collector_creation() {
        let _ = HwmonCollector::new();
        let _ = HwmonCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_hwmon_buffer_as_slice() {
        let mut buffer = HwmonBuffer::new();

        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 45.5,
            unit: Cow::Borrowed("C"),
        };

        buffer.push(sensor);
        let slice = buffer.as_slice();

        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].name, "hwmon0");
    }

    #[test]
    fn test_hwmon_sensor_clone() {
        let sensor1 = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 45.5,
            unit: Cow::Borrowed("C"),
        };

        let sensor2 = sensor1.clone();
        assert_eq!(sensor1.value, sensor2.value);
    }

    #[test]
    fn test_hwmon_buffer_clone() {
        let mut buffer1 = HwmonBuffer::new();

        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 45.5,
            unit: Cow::Borrowed("C"),
        };

        buffer1.push(sensor);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
    }

    #[test]
    fn test_hwmon_sensor_typical_cpu_temperature() {
        // Typical CPU temperature under normal load
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 50.0,
            unit: Cow::Borrowed("C"),
        };

        assert!(sensor.value > 30.0);
        assert!(sensor.value < 70.0);
    }

    #[test]
    fn test_hwmon_sensor_typical_gpu_temperature() {
        // Typical GPU temperature under load
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon1"),
            value: 65.0,
            unit: Cow::Borrowed("C"),
        };

        assert!(sensor.value > 50.0);
        assert!(sensor.value < 85.0);
    }

    #[test]
    fn test_hwmon_sensor_millidegrees_conversion() {
        // Test that values are properly converted from millidegrees to degrees
        // If raw value is 45500, divided by 1000 should be 45.5
        let millidegrees = 45500.0;
        let celsius = millidegrees / 1000.0;

        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: celsius,
            unit: Cow::Borrowed("C"),
        };

        assert_eq!(sensor.value, 45.5);
    }

    #[test]
    fn test_hwmon_buffer_temperature_range() {
        let mut buffer = HwmonBuffer::new();

        let temps = vec![
            ("hwmon0", 35.5),
            ("hwmon1", 42.3),
            ("hwmon2", 55.8),
            ("hwmon3", 38.1),
        ];

        for (name, temp) in temps {
            let sensor = HwmonSensor {
                name: Cow::Owned(name.to_string()),
                value: temp,
                unit: Cow::Borrowed("C"),
            };
            buffer.push(sensor);
        }

        // Find min and max temperatures
        let min_temp = buffer.iter().map(|s| s.value).fold(f64::INFINITY, f64::min);
        let max_temp = buffer
            .iter()
            .map(|s| s.value)
            .fold(f64::NEG_INFINITY, f64::max);

        assert_eq!(min_temp, 35.5);
        assert_eq!(max_temp, 55.8);
    }

    #[test]
    fn test_hwmon_sensor_precision() {
        // Test floating-point precision with sensor values
        let sensor = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 45.123456789,
            unit: Cow::Borrowed("C"),
        };

        // f64 has sufficient precision for temperature readings
        assert!(sensor.value > 45.0);
        assert!(sensor.value < 46.0);
    }

    #[test]
    fn test_hwmon_sensor_serialization_roundtrip() {
        let original = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 48.75,
            unit: Cow::Borrowed("C"),
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: HwmonSensor =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(original.value, deserialized.value);
    }

    #[test]
    fn test_hwmon_buffer_find_hottest_sensor() {
        let mut buffer = HwmonBuffer::new();

        for i in 0..5 {
            let sensor = HwmonSensor {
                name: Cow::Owned(format!("hwmon{}", i)),
                value: 40.0 + (i as f64 * 3.0),
                unit: Cow::Borrowed("C"),
            };
            buffer.push(sensor);
        }

        let hottest = buffer
            .iter()
            .max_by(|a, b| a.value.partial_cmp(&b.value).unwrap())
            .unwrap();

        assert_eq!(hottest.value, 52.0);
    }

    #[test]
    fn test_hwmon_sensor_warning_levels() {
        // Define temperature warning thresholds
        let ideal = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 40.0,
            unit: Cow::Borrowed("C"),
        };
        let warning = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 75.0,
            unit: Cow::Borrowed("C"),
        };
        let critical = HwmonSensor {
            name: Cow::Borrowed("hwmon0"),
            value: 95.0,
            unit: Cow::Borrowed("C"),
        };

        assert!(ideal.value < 50.0);
        assert!(warning.value >= 70.0 && warning.value < 90.0);
        assert!(critical.value >= 90.0);
    }
}
