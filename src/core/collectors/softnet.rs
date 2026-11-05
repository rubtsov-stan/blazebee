//! Per-CPU softirq network processing counters from `/proc/net/softnet_stat`.
//!
//! Every value is hexadecimal. Columns used: 1 processed, 2 dropped,
//! 3 time_squeeze, 10 received_rps, 11 flow_limit_count (when present).

use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const PATH: &str = "/proc/net/softnet_stat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SoftnetStats {
    /// Line index, which equals the CPU number.
    pub cpu: String,
    pub processed: u64,
    pub dropped: u64,
    /// Times the softirq budget or time limit was exhausted.
    pub time_squeeze: u64,
    pub received_rps: u64,
    pub flow_limit_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SoftnetBuffer {
    pub stats: Vec<SoftnetStats>,
}

pub struct SoftnetCollector {
    file: ProcFile,
}

impl Default for SoftnetCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<SoftnetBuffer> {
    let mut stats = Vec::with_capacity(16);
    for (cpu, line) in content.lines().enumerate() {
        let cols: Vec<&str> = fields(line).collect();
        if cols.len() < 3 {
            continue;
        }
        let hex = |i: usize, what: &'static str| -> CollectorResult<u64> {
            match cols.get(i) {
                Some(t) => u64::from_str_radix(t, 16).map_err(|_| {
                    CollectorError::parse(PATH, what, format!("invalid hex value '{t}'"))
                }),
                None => Ok(0),
            }
        };
        stats.push(SoftnetStats {
            cpu: cpu.to_string(),
            processed: hex(0, "processed")?,
            dropped: hex(1, "dropped")?,
            time_squeeze: hex(2, "time_squeeze")?,
            received_rps: hex(9, "received_rps")?,
            flow_limit_count: hex(10, "flow_limit_count")?,
        });
    }
    Ok(SoftnetBuffer { stats })
}

impl Collector for SoftnetCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/net/softnet_stat"
    ));

    #[test]
    fn parses_real_softnet_stat() {
        let out = parse(FIXTURE).unwrap();
        assert_eq!(out.stats.len(), 6);
        assert_eq!(out.stats[0].processed, 0xeb);
        assert_eq!(out.stats[1].time_squeeze, 0xb);
        assert_eq!(out.stats[1].cpu, "1");
    }

    #[test]
    fn old_format_without_rps_columns_and_bad_hex() {
        let out = parse("0000000a 00000001 00000002\n").unwrap();
        assert_eq!((out.stats[0].processed, out.stats[0].received_rps), (10, 0));
        assert!(parse("zz 1 2\n").is_err());
    }
}
