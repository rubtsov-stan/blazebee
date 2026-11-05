//! Software RAID arrays from `/proc/mdstat`.
//!
//! ```text
//! md0 : active raid5 sdd[3] sdc[1] sdb[0](F) sde[4](S)
//!       1953260544 blocks super 1.2 level 5, 512k chunk, algorithm 2 [3/2] [UU_]
//!       [=>...................]  recovery =  5.3% (51800064/976630272) finish=77.1min speed=199875K/sec
//! ```

use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    sample, CollectFuture, Collector,
};

const PATH: &str = "/proc/mdstat";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct MdRaidStats {
    pub device: String,
    /// `raid1`, `raid5`, `linear`, ... or empty for inactive arrays.
    pub level: String,
    /// `active`, `inactive`, `active (auto-read-only)`, ...
    pub state: String,
    /// Working member devices (`m` in `[n/m]`).
    pub active_devices: u32,
    /// Member slots of the array (`n` in `[n/m]`).
    pub total_devices: u32,
    /// Members flagged `(F)`.
    pub failed_devices: u32,
    /// Members flagged `(S)`.
    pub spare_devices: u32,
    pub degraded: bool,
    /// `recovery`, `resync`, `reshape` or `check` while an operation runs.
    pub sync_action: Option<String>,
    /// Progress of `sync_action` in percent.
    pub sync_progress: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MdRaidBuffer {
    pub stats: Vec<MdRaidStats>,
}

pub struct MdRaidCollector {
    file: ProcFile,
}

impl Default for MdRaidCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

fn is_level(token: &str) -> bool {
    token.starts_with("raid") || matches!(token, "linear" | "multipath" | "faulty")
}

pub fn parse(content: &str) -> MdRaidBuffer {
    let mut stats: Vec<MdRaidStats> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if line.starts_with("md") && line.contains(" : ") {
            let mut f = fields(line);
            let device = f.next().unwrap_or_default().to_string();
            f.next(); // ':'
            let mut state = String::new();
            let mut level = String::new();
            let mut failed = 0;
            let mut spare = 0;
            let mut members = 0u32;
            for token in f {
                if token.contains('[') {
                    members += 1;
                    if token.contains("(F)") {
                        failed += 1;
                    }
                    if token.contains("(S)") {
                        spare += 1;
                    }
                } else if level.is_empty() && is_level(token) {
                    level = token.to_string();
                } else if level.is_empty() {
                    if !state.is_empty() {
                        state.push(' ');
                    }
                    state.push_str(token);
                }
            }
            stats.push(MdRaidStats {
                device,
                level,
                state,
                total_devices: members,
                active_devices: members.saturating_sub(failed).saturating_sub(spare),
                failed_devices: failed,
                spare_devices: spare,
                ..Default::default()
            });
            continue;
        }
        let Some(current) = stats.last_mut() else {
            continue;
        };
        if trimmed.contains(" blocks ") {
            // "[n/m]" gives the array geometry; the member list may omit removed devices.
            if let Some(start) = trimmed.find(" [") {
                let geometry = &trimmed[start + 2..];
                if let Some((n, rest)) = geometry.split_once('/') {
                    if let Some((m, _)) = rest.split_once(']') {
                        if let (Ok(n), Ok(m)) = (n.parse::<u32>(), m.parse::<u32>()) {
                            current.total_devices = n;
                            current.active_devices = m;
                            current.degraded = m < n;
                        }
                    }
                }
            }
        } else {
            for action in ["recovery", "resync", "reshape", "check"] {
                let Some(pos) = trimmed.find(&format!("{action} =")) else {
                    continue;
                };
                let after = &trimmed[pos + action.len() + 2..];
                let pct = after
                    .trim_start()
                    .split('%')
                    .next()
                    .and_then(|p| p.trim().parse::<f64>().ok());
                current.sync_action = Some(action.to_string());
                current.sync_progress = pct;
                break;
            }
        }
    }
    MdRaidBuffer { stats }
}

impl Collector for MdRaidCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?))) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Personalities : [raid1] [raid6] [raid5] [raid4]
md127 : active raid1 sdb1[1] sda1[0]
      976630464 blocks super 1.2 [2/2] [UU]
      bitmap: 0/8 pages [0KB], 65536KB chunk

md0 : active raid5 sdd[3] sdc[1] sdb[0](F) sde[4](S)
      1953260544 blocks super 1.2 level 5, 512k chunk, algorithm 2 [3/2] [UU_]
      [=>...................]  recovery =  5.3% (51800064/976630272) finish=77.1min speed=199875K/sec

md1 : active (auto-read-only) raid1 sdf1[0]
      1047552 blocks super 1.2 [2/1] [U_]

md2 : inactive sdg1[0](S)
      1047552 blocks super external:imsm

unused devices: <none>
";

    #[test]
    fn parses_geometry_flags_and_progress() {
        let out = parse(SAMPLE).stats;
        assert_eq!(out.len(), 4);

        let healthy = &out[0];
        assert_eq!(
            (
                healthy.device.as_str(),
                healthy.level.as_str(),
                healthy.state.as_str()
            ),
            ("md127", "raid1", "active")
        );
        assert_eq!(
            (
                healthy.total_devices,
                healthy.active_devices,
                healthy.failed_devices
            ),
            (2, 2, 0)
        );
        assert!(!healthy.degraded);
        assert_eq!(healthy.sync_action, None);

        let rebuilding = &out[1];
        assert_eq!(
            (rebuilding.total_devices, rebuilding.active_devices),
            (3, 2)
        );
        assert_eq!(
            (rebuilding.failed_devices, rebuilding.spare_devices),
            (1, 1)
        );
        assert!(rebuilding.degraded);
        assert_eq!(rebuilding.sync_action.as_deref(), Some("recovery"));
        assert_eq!(rebuilding.sync_progress, Some(5.3));

        let ro = &out[2];
        assert_eq!(ro.state, "active (auto-read-only)");
        assert_eq!(ro.level, "raid1");
        assert!(ro.degraded);

        let inactive = &out[3];
        assert_eq!(inactive.state, "inactive");
        assert_eq!(inactive.level, "");
        assert_eq!(inactive.spare_devices, 1);
    }

    #[test]
    fn no_arrays() {
        assert!(parse("Personalities :\nunused devices: <none>\n")
            .stats
            .is_empty());
    }
}
