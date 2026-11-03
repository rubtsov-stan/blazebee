//! Temperature sensors from `/sys/class/hwmon`.
//!
//! Every `tempN_input` of every chip is reported, with the chip name and the
//! optional sensor label, so multi-sensor chips (CPU packages, NVMe drives)
//! are fully visible.

use std::{fs, path::Path};

use async_trait::async_trait;
use serde::Serialize;

use super::{procfs::read_trimmed, Collector, CollectorError, CollectorResult, Sample};

const ROOT: &str = "/sys/class/hwmon";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HwmonSensor {
    /// Device directory, e.g. `hwmon2`.
    pub name: String,
    /// Chip name from the `name` attribute, e.g. `coretemp` or `nvme`.
    pub chip: String,
    /// Sensor prefix, e.g. `temp1`.
    pub sensor: String,
    /// `tempN_label` if present, otherwise empty.
    pub label: String,
    pub value: f64,
    pub unit: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct HwmonBuffer {
    pub sensors: Vec<HwmonSensor>,
}

#[derive(Default)]
pub struct HwmonCollector;

pub fn read_sensors(root: &Path) -> CollectorResult<HwmonBuffer> {
    let devices =
        fs::read_dir(root).map_err(|e| CollectorError::read(root.display().to_string(), e))?;
    let mut sensors = Vec::new();
    for device in devices.flatten() {
        let name = device.file_name();
        let Some(name) = name.to_str() else { continue };
        let dir = device.path();
        let Ok(files) = fs::read_dir(&dir) else {
            continue;
        };
        let chip = read_trimmed(&dir.join("name")).unwrap_or_default();
        for file in files.flatten() {
            let file_name = file.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let Some(sensor) = file_name
                .strip_suffix("_input")
                .filter(|s| s.starts_with("temp"))
            else {
                continue;
            };
            let Ok(raw) = read_trimmed(&file.path()) else {
                continue;
            };
            let Ok(millidegrees) = raw.parse::<i64>() else {
                continue;
            };
            sensors.push(HwmonSensor {
                name: name.to_string(),
                chip: chip.clone(),
                sensor: sensor.to_string(),
                label: read_trimmed(&dir.join(format!("{sensor}_label"))).unwrap_or_default(),
                value: millidegrees as f64 / 1000.0,
                unit: "C",
            });
        }
    }
    sensors.sort_by(|a, b| (&a.name, &a.sensor).cmp(&(&b.name, &b.sensor)));
    Ok(HwmonBuffer { sensors })
}

#[async_trait]
impl Collector for HwmonCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        Ok(Box::new(read_sensors(Path::new(ROOT))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_all_temperature_inputs_with_labels() {
        let dir = tempfile::tempdir().unwrap();
        let chip = dir.path().join("hwmon3");
        fs::create_dir(&chip).unwrap();
        fs::write(chip.join("name"), "coretemp\n").unwrap();
        fs::write(chip.join("temp1_input"), "52000\n").unwrap();
        fs::write(chip.join("temp1_label"), "Package id 0\n").unwrap();
        fs::write(chip.join("temp2_input"), "49000\n").unwrap();
        fs::write(chip.join("fan1_input"), "1200\n").unwrap();
        fs::write(chip.join("temp3_input"), "bad\n").unwrap();

        let out = read_sensors(dir.path()).unwrap();
        assert_eq!(out.sensors.len(), 2);
        assert_eq!(out.sensors[0].sensor, "temp1");
        assert_eq!(out.sensors[0].label, "Package id 0");
        assert_eq!(out.sensors[0].value, 52.0);
        assert_eq!(out.sensors[1].label, "");
        assert_eq!(out.sensors[1].chip, "coretemp");
    }
}
