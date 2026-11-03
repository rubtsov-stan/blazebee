//! Memory error counters from `/sys/devices/system/edac/mc`.

use std::{fs, path::Path};

use serde::Serialize;

use super::{
    procfs::read_trimmed, sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const ROOT: &str = "/sys/devices/system/edac/mc";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EdacStats {
    /// Memory controller, e.g. `mc0`.
    pub controller: String,
    pub correctable_errors: u64,
    pub uncorrectable_errors: u64,
    /// Comma-separated DIMM labels of the controller, if exposed.
    pub dimm_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdacBuffer {
    pub stats: Vec<EdacStats>,
}

#[derive(Default)]
pub struct EdacCollector;

pub fn read_controllers(root: &Path) -> CollectorResult<EdacBuffer> {
    let entries =
        fs::read_dir(root).map_err(|e| CollectorError::read(root.display().to_string(), e))?;
    let mut stats = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("mc") || !name[2..].bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let dir = entry.path();
        let count = |file: &str| -> u64 {
            read_trimmed(&dir.join(file))
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };
        let mut labels = Vec::new();
        if let Ok(dimms) = fs::read_dir(&dir) {
            for dimm in dimms.flatten() {
                if dimm
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("dimm") || n.starts_with("rank"))
                {
                    if let Ok(label) = read_trimmed(&dimm.path().join("dimm_label")) {
                        if !label.is_empty() {
                            labels.push(label);
                        }
                    }
                }
            }
        }
        labels.sort();
        stats.push(EdacStats {
            controller: name.to_string(),
            correctable_errors: count("ce_count"),
            uncorrectable_errors: count("ue_count"),
            dimm_label: labels.join(","),
        });
    }
    stats.sort_by(|a, b| a.controller.cmp(&b.controller));
    Ok(EdacBuffer { stats })
}

impl Collector for EdacCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(read_controllers(Path::new(ROOT))?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_counts_and_labels() {
        let dir = tempfile::tempdir().unwrap();
        let mc = dir.path().join("mc0");
        fs::create_dir_all(mc.join("dimm0")).unwrap();
        fs::create_dir_all(mc.join("dimm1")).unwrap();
        fs::write(mc.join("ce_count"), "3\n").unwrap();
        fs::write(mc.join("ue_count"), "0\n").unwrap();
        fs::write(mc.join("dimm0/dimm_label"), "CPU_SrcID#0_DIMM_A0\n").unwrap();
        fs::write(mc.join("dimm1/dimm_label"), "CPU_SrcID#0_DIMM_B0\n").unwrap();
        fs::create_dir(dir.path().join("mc_other")).unwrap();

        let out = read_controllers(dir.path()).unwrap();
        assert_eq!(out.stats.len(), 1);
        assert_eq!(out.stats[0].correctable_errors, 3);
        assert_eq!(
            out.stats[0].dimm_label,
            "CPU_SrcID#0_DIMM_A0,CPU_SrcID#0_DIMM_B0"
        );
    }
}
