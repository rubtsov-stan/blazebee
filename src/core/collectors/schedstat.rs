//! Per-CPU scheduler accounting from `/proc/schedstat`.
//!
//! On a `cpuN` line the last three fields are, in order: time spent running
//! tasks, time tasks spent waiting on the run queue (both in nanoseconds),
//! and the number of timeslices run.

use serde::Serialize;

use super::{
    procfs::{fields, parse as parse_num, ProcFile},
    sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const PATH: &str = "/proc/schedstat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SchedstatCpu {
    /// Logical CPU number as a string, e.g. `0`.
    pub cpu: String,
    pub running_ns: u64,
    pub run_delay_ns: u64,
    pub timeslices: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchedstatBuffer {
    pub stats: Vec<SchedstatCpu>,
}

pub struct SchedstatCollector {
    file: ProcFile,
}

impl Default for SchedstatCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<SchedstatBuffer> {
    let mut stats = Vec::with_capacity(16);
    for line in content.lines() {
        let Some(rest) = line.strip_prefix("cpu") else {
            continue;
        };
        let mut f = fields(rest);
        let cpu = f.next().unwrap_or_default();
        let values: Vec<&str> = f.collect();
        if values.len() < 9 {
            return Err(CollectorError::parse(
                PATH,
                "cpu line",
                format!("expected at least 9 fields, got {}", values.len()),
            ));
        }
        let tail = &values[values.len() - 3..];
        stats.push(SchedstatCpu {
            cpu: cpu.to_string(),
            running_ns: parse_num(tail[0], PATH, "running_ns")?,
            run_delay_ns: parse_num(tail[1], PATH, "run_delay_ns")?,
            timeslices: parse_num(tail[2], PATH, "timeslices")?,
        });
    }
    Ok(SchedstatBuffer { stats })
}

impl Collector for SchedstatCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/schedstat"
    ));

    #[test]
    fn parses_real_schedstat_version_17() {
        let out = parse(FIXTURE).unwrap();
        assert_eq!(out.stats.len(), 6);
        let cpu0 = &out.stats[0];
        assert_eq!(cpu0.cpu, "0");
        assert_eq!(cpu0.running_ns, 133_049_419_543);
        assert_eq!(cpu0.run_delay_ns, 126_475_041_861);
        assert_eq!(cpu0.timeslices, 87_974);
    }

    #[test]
    fn rejects_short_cpu_lines() {
        assert!(parse("version 15\ncpu0 1 2 3\n").is_err());
    }
}
