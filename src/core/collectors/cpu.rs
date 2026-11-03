//! Per-CPU time counters from `/proc/stat`, in clock ticks (`USER_HZ`).

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    Collector, CollectorResult, Sample,
};

const PATH: &str = "/proc/stat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CpuStats {
    /// `cpu` for the aggregate line, `cpu0`, `cpu1`, ... per logical CPU.
    pub cpu: String,
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    /// Time stolen by the hypervisor; 0 on kernels without the field.
    pub steal: u64,
    pub guest: u64,
    pub guest_nice: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuBuffer {
    pub stats: Vec<CpuStats>,
}

pub struct CpuCollector {
    file: ProcFile,
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<CpuBuffer> {
    let mut stats = Vec::with_capacity(17);
    for line in content.lines() {
        if !line.starts_with("cpu") {
            // CPU lines come first; stop at the first other line.
            if !stats.is_empty() {
                break;
            }
            continue;
        }
        let mut f = fields(line);
        let cpu = f.next().unwrap_or_default().to_string();
        let mut counters = [0u64; 10];
        const NAMES: [&str; 10] = [
            "user",
            "nice",
            "system",
            "idle",
            "iowait",
            "irq",
            "softirq",
            "steal",
            "guest",
            "guest_nice",
        ];
        for (i, slot) in counters.iter_mut().enumerate() {
            if i < 7 {
                *slot = next_field(&mut f, PATH, NAMES[i])?;
            } else {
                match f.next() {
                    Some(token) => *slot = super::procfs::parse(token, PATH, NAMES[i])?,
                    None => break,
                }
            }
        }
        stats.push(CpuStats {
            cpu,
            user: counters[0],
            nice: counters[1],
            system: counters[2],
            idle: counters[3],
            iowait: counters[4],
            irq: counters[5],
            softirq: counters[6],
            steal: counters[7],
            guest: counters[8],
            guest_nice: counters[9],
        });
    }
    if stats.is_empty() {
        return Err(super::CollectorError::missing(PATH, "cpu lines"));
    }
    Ok(CpuBuffer { stats })
}

#[async_trait]
impl Collector for CpuCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        Ok(Box::new(parse(self.file.read()?)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/stat"
    ));

    #[test]
    fn parses_real_stat() {
        let out = parse(FIXTURE).unwrap();
        assert_eq!(out.stats.len(), 7, "aggregate + 6 cpus");
        assert_eq!(out.stats[0].cpu, "cpu");
        assert_eq!(out.stats[0].user, 81730);
        assert_eq!(out.stats[0].softirq, 213);
        assert_eq!(out.stats[1].cpu, "cpu0");
        assert_eq!(out.stats[1].idle, 34449);
    }

    #[test]
    fn old_kernel_without_steal_defaults_to_zero() {
        let out = parse("cpu  1 2 3 4 5 6 7\ncpu0 1 2 3 4 5 6 7\nintr 5\n").unwrap();
        assert_eq!(out.stats.len(), 2);
        assert_eq!(out.stats[0].steal, 0);
        assert_eq!(out.stats[0].guest_nice, 0);
    }

    #[test]
    fn rejects_missing_or_garbled_fields() {
        assert!(parse("cpu 1 2 3\n").is_err());
        assert!(parse("cpu 1 2 3 x 5 6 7\n").is_err());
        assert!(parse("intr 5\n").is_err());
    }
}
