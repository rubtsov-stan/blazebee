//! Memory totals from `/proc/meminfo`, in kibibytes as reported by the kernel.

use serde::Serialize;

use super::{
    procfs::{fields, parse as parse_num, ProcFile},
    sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const PATH: &str = "/proc/meminfo";

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct MemoryStats {
    pub mem_total: u64,
    pub mem_free: u64,
    /// Kernel estimate of memory available for new workloads without swapping.
    pub mem_available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

pub struct MemoryCollector {
    file: ProcFile,
}

impl Default for MemoryCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<MemoryStats> {
    let mut stats = MemoryStats::default();
    let mut seen_total = false;
    for line in content.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let slot = match key {
            "MemTotal" => &mut stats.mem_total,
            "MemFree" => &mut stats.mem_free,
            "MemAvailable" => &mut stats.mem_available,
            "Buffers" => &mut stats.buffers,
            "Cached" => &mut stats.cached,
            "SwapTotal" => &mut stats.swap_total,
            "SwapFree" => &mut stats.swap_free,
            _ => continue,
        };
        let Some(token) = fields(rest).next() else {
            continue;
        };
        *slot = parse_num(token, PATH, "memory counter")?;
        if key == "MemTotal" {
            seen_total = true;
        }
    }
    if !seen_total {
        return Err(CollectorError::missing(PATH, "MemTotal"));
    }
    Ok(stats)
}

impl Collector for MemoryCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/meminfo"
    ));

    #[test]
    fn parses_real_meminfo() {
        let m = parse(FIXTURE).unwrap();
        assert!(m.mem_total > 0 && m.mem_available > 0);
        assert!(m.mem_free <= m.mem_total);
        assert!(m.swap_free <= m.swap_total);
    }

    #[test]
    fn optional_fields_default_and_memtotal_is_required() {
        let m = parse("MemTotal:       1024 kB\nUnknown: 5 kB\n").unwrap();
        assert_eq!(
            m,
            MemoryStats {
                mem_total: 1024,
                ..Default::default()
            }
        );
        assert!(matches!(
            parse("MemFree: 1 kB\n"),
            Err(CollectorError::Missing { .. })
        ));
        assert!(matches!(
            parse("MemTotal: x kB\n"),
            Err(CollectorError::Parse { .. })
        ));
    }
}
