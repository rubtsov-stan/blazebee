//! Thermal zone temperatures from `/sys/class/thermal`.

use std::{fs, path::Path};

use serde::Serialize;

use super::{
    procfs::read_trimmed, sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const ROOT: &str = "/sys/class/thermal";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ThermalZoneInfo {
    /// Directory name, e.g. `thermal_zone0`.
    pub zone_id: String,
    /// Driver-provided zone type, e.g. `x86_pkg_temp` or `cpu-thermal`.
    pub zone_type: String,
    /// Degrees Celsius with millidegree resolution.
    pub temperature: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThermalBuffer {
    pub zones: Vec<ThermalZoneInfo>,
}

#[derive(Default)]
pub struct ThermalCollector;

pub fn read_zones(root: &Path) -> CollectorResult<ThermalBuffer> {
    let entries =
        fs::read_dir(root).map_err(|e| CollectorError::read(root.display().to_string(), e))?;
    let mut zones = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let dir = entry.path();
        // Some zones report EAGAIN/ENODEV transiently; skip them for this cycle.
        let Ok(raw) = read_trimmed(&dir.join("temp")) else {
            continue;
        };
        let Ok(millidegrees) = raw.parse::<i64>() else {
            continue;
        };
        zones.push(ThermalZoneInfo {
            zone_id: name.to_string(),
            zone_type: read_trimmed(&dir.join("type")).unwrap_or_default(),
            temperature: millidegrees as f64 / 1000.0,
        });
    }
    zones.sort_by(|a, b| a.zone_id.cmp(&b.zone_id));
    Ok(ThermalBuffer { zones })
}

impl Collector for ThermalCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(read_zones(Path::new(ROOT))?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_synthetic_sysfs_tree() {
        let dir = tempfile::tempdir().unwrap();
        for (zone, temp, kind) in [
            ("thermal_zone1", "45999\n", "x86_pkg_temp\n"),
            ("thermal_zone0", "-5000", "acpitz"),
        ] {
            let z = dir.path().join(zone);
            fs::create_dir(&z).unwrap();
            fs::write(z.join("temp"), temp).unwrap();
            fs::write(z.join("type"), kind).unwrap();
        }
        fs::create_dir(dir.path().join("cooling_device0")).unwrap();
        let broken = dir.path().join("thermal_zone2");
        fs::create_dir(&broken).unwrap();
        fs::write(broken.join("temp"), "n/a").unwrap();

        let out = read_zones(dir.path()).unwrap();
        assert_eq!(out.zones.len(), 2);
        assert_eq!(out.zones[0].zone_id, "thermal_zone0");
        assert_eq!(out.zones[0].temperature, -5.0);
        assert_eq!(out.zones[1].temperature, 45.999);
        assert_eq!(out.zones[1].zone_type, "x86_pkg_temp");
    }

    #[test]
    fn missing_root_is_an_error() {
        assert!(matches!(
            read_zones(Path::new("/nonexistent")),
            Err(CollectorError::Read { .. })
        ));
    }
}
