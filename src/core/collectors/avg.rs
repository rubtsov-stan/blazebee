//! Load averages from `/proc/loadavg`.

use serde::Serialize;

use super::{
    procfs::{fields, next_field, parse as parse_num, ProcFile},
    sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const PATH: &str = "/proc/loadavg";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LoadAverage {
    pub one_minute: f64,
    pub five_minutes: f64,
    pub fifteen_minutes: f64,
    /// Currently runnable scheduling entities.
    pub running_processes: u32,
    /// Total scheduling entities (processes and threads).
    pub total_processes: u32,
}

pub struct LoadAverageCollector {
    file: ProcFile,
}

impl Default for LoadAverageCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<LoadAverage> {
    let mut f = fields(content);
    let one_minute = next_field(&mut f, PATH, "load1")?;
    let five_minutes = next_field(&mut f, PATH, "load5")?;
    let fifteen_minutes = next_field(&mut f, PATH, "load15")?;
    let procs = f
        .next()
        .ok_or_else(|| CollectorError::missing(PATH, "running/total"))?;
    let (running, total) = procs.split_once('/').ok_or_else(|| {
        CollectorError::parse(
            PATH,
            "running/total",
            format!("expected 'a/b', got '{procs}'"),
        )
    })?;
    Ok(LoadAverage {
        one_minute,
        five_minutes,
        fifteen_minutes,
        running_processes: parse_num(running, PATH, "running")?,
        total_processes: parse_num(total, PATH, "total")?,
    })
}

impl Collector for LoadAverageCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/loadavg"
    ));

    #[test]
    fn parses_real_loadavg() {
        let l = parse(FIXTURE).unwrap();
        assert_eq!(
            (l.one_minute, l.five_minutes, l.fifteen_minutes),
            (0.14, 1.57, 1.15)
        );
        assert_eq!((l.running_processes, l.total_processes), (2, 195));
    }

    #[test]
    fn rejects_malformed_process_field() {
        assert!(parse("0.1 0.2 0.3 2-195 13\n").is_err());
        assert!(parse("0.1 0.2\n").is_err());
    }
}
