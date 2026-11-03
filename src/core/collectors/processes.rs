//! Process and scheduler counters from `/proc/stat`.

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    Collector, CollectorError, CollectorResult, Sample,
};

const PATH: &str = "/proc/stat";

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ProcessStats {
    /// Forks since boot (monotonic counter).
    pub total_processes: u64,
    pub running_processes: u64,
    /// Tasks blocked on I/O.
    pub blocked_processes: u64,
    /// Context switches since boot (monotonic counter).
    pub context_switches: u64,
}

pub struct ProcessCollector {
    file: ProcFile,
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<ProcessStats> {
    let mut stats = ProcessStats::default();
    let mut found = 0;
    for line in content.lines() {
        let mut f = fields(line);
        let (slot, what) = match f.next() {
            Some("processes") => (&mut stats.total_processes, "processes"),
            Some("procs_running") => (&mut stats.running_processes, "procs_running"),
            Some("procs_blocked") => (&mut stats.blocked_processes, "procs_blocked"),
            Some("ctxt") => (&mut stats.context_switches, "ctxt"),
            _ => continue,
        };
        *slot = next_field(&mut f, PATH, what)?;
        found += 1;
    }
    if found < 4 {
        return Err(CollectorError::missing(
            PATH,
            "processes/procs_running/procs_blocked/ctxt",
        ));
    }
    Ok(stats)
}

#[async_trait]
impl Collector for ProcessCollector {
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
        let s = parse(FIXTURE).unwrap();
        assert!(s.total_processes > 0);
        assert!(s.context_switches > 0);
        assert!(s.running_processes >= 1);
    }

    #[test]
    fn missing_counter_is_an_error() {
        assert!(matches!(
            parse("processes 1\nctxt 2\n"),
            Err(CollectorError::Missing { .. })
        ));
        assert!(parse("processes x\nprocs_running 1\nprocs_blocked 0\nctxt 2\n").is_err());
    }
}
