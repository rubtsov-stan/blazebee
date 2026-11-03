//! systemd unit states via `systemctl list-units`.
//!
//! `--plain` removes the bullet column that otherwise shifts the fields of
//! failed units, `--full` prevents truncation of long unit names, and the
//! child is killed if the collector is cancelled or exceeds the deadline.

use std::{process::Stdio, time::Duration};

use serde::Serialize;
use tokio::process::Command;

use super::{procfs::fields, sample, CollectFuture, Collector, CollectorError};

const COMMAND: &str = "systemctl";
const ARGS: [&str; 6] = [
    "list-units",
    "--all",
    "--plain",
    "--no-legend",
    "--no-pager",
    "--full",
];
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SystemdUnitStats {
    pub unit_name: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemdBuffer {
    pub stats: Vec<SystemdUnitStats>,
}

#[derive(Default)]
pub struct SystemdCollector;

pub fn parse(output: &str) -> SystemdBuffer {
    let stats = output
        .lines()
        .filter_map(|line| {
            let mut f = fields(line);
            let unit_name = f.next()?;
            let load_state = f.next()?;
            let active_state = f.next()?;
            let sub_state = f.next()?;
            // Everything after the fourth field, preserving inner spaces.
            let consumed = sub_state.as_ptr() as usize - line.as_ptr() as usize + sub_state.len();
            let description = line[consumed..].trim();
            Some(SystemdUnitStats {
                unit_name: unit_name.to_string(),
                load_state: load_state.to_string(),
                active_state: active_state.to_string(),
                sub_state: sub_state.to_string(),
                description: description.to_string(),
            })
        })
        .collect();
    SystemdBuffer { stats }
}

impl Collector for SystemdCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move {
            let child = Command::new(COMMAND)
                .args(ARGS)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output();
            let output = tokio::time::timeout(TIMEOUT, child)
                .await
                .map_err(|_| CollectorError::Command {
                    command: COMMAND,
                    reason: format!("did not finish within {TIMEOUT:?}"),
                })?
                .map_err(|e| CollectorError::Command {
                    command: COMMAND,
                    reason: e.to_string(),
                })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CollectorError::Command {
                    command: COMMAND,
                    reason: format!("{}: {}", output.status, stderr.trim()),
                });
            }
            Ok(sample(parse(&String::from_utf8_lossy(&output.stdout))))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_output_including_failed_units() {
        let out = "\
  dev-sda1.device                loaded active   plugged   SanDisk SSD 1
  nginx.service                  loaded failed   failed    A high performance web server
  snapd.service                  not-found inactive dead  snapd.service
  user.slice                     loaded active   active    User and Session Slice
";
        let s = parse(out).stats;
        assert_eq!(s.len(), 4);
        assert_eq!(s[1].unit_name, "nginx.service");
        assert_eq!(
            (s[1].active_state.as_str(), s[1].sub_state.as_str()),
            ("failed", "failed")
        );
        assert_eq!(s[1].description, "A high performance web server");
        assert_eq!(s[2].load_state, "not-found");
        assert_eq!(s[2].description, "snapd.service");
    }

    #[test]
    fn ignores_short_lines() {
        assert!(parse("a b\n\n").stats.is_empty());
    }
}
