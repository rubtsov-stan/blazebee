//! Boot time, fork and interrupt totals from `/proc/stat`.

use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const PATH: &str = "/proc/stat";

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct StatGlobalStats {
    /// Unix time of boot (`btime`).
    pub boot_time: i64,
    /// Forks since boot (`processes`).
    pub total_forks: u64,
    /// Sum of all interrupts since boot (first value of `intr`).
    pub total_interrupts: u64,
}

pub struct StatCollector {
    file: ProcFile,
}

impl Default for StatCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<StatGlobalStats> {
    let mut stats = StatGlobalStats::default();
    let mut found = 0;
    for line in content.lines() {
        let mut f = fields(line);
        match f.next() {
            Some("btime") => stats.boot_time = next_field(&mut f, PATH, "btime")?,
            Some("processes") => stats.total_forks = next_field(&mut f, PATH, "processes")?,
            Some("intr") => stats.total_interrupts = next_field(&mut f, PATH, "intr")?,
            _ => continue,
        }
        found += 1;
    }
    if found < 3 {
        return Err(CollectorError::missing(PATH, "btime/processes/intr"));
    }
    Ok(stats)
}

impl Collector for StatCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
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
        let s = parse(FIXTURE).unwrap();
        assert!(s.boot_time > 1_600_000_000);
        assert!(s.total_forks > 0);
        assert!(s.total_interrupts > 0);
    }

    #[test]
    fn missing_lines_are_errors() {
        assert!(matches!(
            parse("btime 1\nintr 2 3\n"),
            Err(CollectorError::Missing { .. })
        ));
    }
}
