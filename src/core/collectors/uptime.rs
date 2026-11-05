//! System uptime from `/proc/uptime`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    sample, CollectFuture, Collector, CollectorResult,
};

const PATH: &str = "/proc/uptime";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UptimeInfo {
    pub uptime_seconds: f64,
    /// Idle time summed over all CPUs.
    pub idle_time_seconds: f64,
    /// Unix time of boot derived from the wall clock; shifts if the clock is stepped.
    pub boot_time_seconds: i64,
}

pub struct UptimeCollector {
    file: ProcFile,
}

impl Default for UptimeCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str, now_unix: f64) -> CollectorResult<UptimeInfo> {
    let mut f = fields(content);
    let uptime_seconds: f64 = next_field(&mut f, PATH, "uptime")?;
    let idle_time_seconds: f64 = next_field(&mut f, PATH, "idle")?;
    Ok(UptimeInfo {
        uptime_seconds,
        idle_time_seconds,
        boot_time_seconds: (now_unix - uptime_seconds).floor() as i64,
    })
}

impl Collector for UptimeCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            Ok(sample(parse(self.file.read()?, now)?))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/uptime"
    ));

    #[test]
    fn parses_real_uptime() {
        let u = parse(FIXTURE, 1_000_000.0).unwrap();
        assert_eq!(u.uptime_seconds, 478.22);
        assert_eq!(u.idle_time_seconds, 1947.88);
        assert_eq!(u.boot_time_seconds, 999_521);
    }

    #[test]
    fn rejects_incomplete_content() {
        assert!(parse("12.5\n", 0.0).is_err());
        assert!(parse("", 0.0).is_err());
    }
}
