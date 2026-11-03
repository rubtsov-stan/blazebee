//! Power supplies (batteries, mains adapters) from `/sys/class/power_supply`.

use std::{fs, path::Path};

use serde::Serialize;

use super::{
    procfs::read_trimmed, sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const ROOT: &str = "/sys/class/power_supply";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PowerStats {
    /// Directory name, e.g. `BAT0` or `AC`.
    pub name: String,
    /// `Battery`, `Mains`, `USB`, ...
    pub supply_type: String,
    /// `Charging`, `Discharging`, `Full`, `Not charging`; empty for mains adapters.
    pub status: String,
    /// `1` when a mains/USB supply is plugged in; absent for batteries.
    pub online: Option<u8>,
    pub capacity_percent: Option<u8>,
    /// Microwatt-hours; present on energy-reporting batteries.
    pub energy_now: Option<u64>,
    pub energy_full: Option<u64>,
    /// Microwatts.
    pub power_now: Option<u64>,
    /// Microampere-hours; present on charge-reporting batteries.
    pub charge_now: Option<u64>,
    pub charge_full: Option<u64>,
    /// Microamperes.
    pub current_now: Option<u64>,
    /// Microvolts.
    pub voltage_now: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PowerBuffer {
    pub stats: Vec<PowerStats>,
}

#[derive(Default)]
pub struct PowerCollector;

pub fn read_supplies(root: &Path) -> CollectorResult<PowerBuffer> {
    let entries =
        fs::read_dir(root).map_err(|e| CollectorError::read(root.display().to_string(), e))?;
    let mut stats = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let dir = entry.path();
        let text = |file: &str| read_trimmed(&dir.join(file)).unwrap_or_default();
        fn number<T: std::str::FromStr>(dir: &Path, file: &str) -> Option<T> {
            read_trimmed(&dir.join(file))
                .ok()
                .and_then(|s| s.parse().ok())
        }
        stats.push(PowerStats {
            name: name.to_string(),
            supply_type: text("type"),
            status: text("status"),
            online: number(&dir, "online"),
            capacity_percent: number(&dir, "capacity"),
            energy_now: number(&dir, "energy_now"),
            energy_full: number(&dir, "energy_full"),
            power_now: number(&dir, "power_now"),
            charge_now: number(&dir, "charge_now"),
            charge_full: number(&dir, "charge_full"),
            current_now: number(&dir, "current_now"),
            voltage_now: number(&dir, "voltage_now"),
        });
    }
    stats.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(PowerBuffer { stats })
}

impl Collector for PowerCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(read_supplies(Path::new(ROOT))?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_battery_and_mains() {
        let dir = tempfile::tempdir().unwrap();
        let bat = dir.path().join("BAT0");
        fs::create_dir(&bat).unwrap();
        for (f, v) in [
            ("type", "Battery"),
            ("status", "Discharging"),
            ("capacity", "87"),
            ("charge_now", "4000000"),
            ("charge_full", "4600000"),
            ("current_now", "650000"),
            ("voltage_now", "12100000"),
        ] {
            fs::write(bat.join(f), format!("{v}\n")).unwrap();
        }
        let ac = dir.path().join("AC");
        fs::create_dir(&ac).unwrap();
        fs::write(ac.join("type"), "Mains\n").unwrap();
        fs::write(ac.join("online"), "0\n").unwrap();

        let out = read_supplies(dir.path()).unwrap();
        assert_eq!(out.stats.len(), 2);
        assert_eq!(out.stats[0].name, "AC");
        assert_eq!(out.stats[0].online, Some(0));
        assert_eq!(out.stats[1].capacity_percent, Some(87));
        assert_eq!(out.stats[1].charge_now, Some(4_000_000));
        assert_eq!(out.stats[1].energy_now, None);
    }
}
