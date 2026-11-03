//! Pressure stall information from `/proc/pressure/{cpu,memory,io}`.

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, parse as parse_num},
    Collector, CollectorError, CollectorResult, Sample,
};

const RESOURCES: [(&str, &str); 3] = [
    ("cpu", "/proc/pressure/cpu"),
    ("memory", "/proc/pressure/memory"),
    ("io", "/proc/pressure/io"),
];

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct PressureStats {
    /// `cpu`, `memory` or `io`.
    pub metric_type: String,
    pub some_avg10: f64,
    pub some_avg60: f64,
    pub some_avg300: f64,
    /// Cumulative stall time with at least one task stalled, in microseconds.
    pub some_total_us: u64,
    pub full_avg10: Option<f64>,
    pub full_avg60: Option<f64>,
    pub full_avg300: Option<f64>,
    pub full_total_us: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PressureBuffer {
    pub stats: Vec<PressureStats>,
}

#[derive(Default)]
pub struct PressureCollector {
    buf: String,
}

pub fn parse(metric_type: &str, path: &str, content: &str) -> CollectorResult<PressureStats> {
    let mut stats = PressureStats {
        metric_type: metric_type.to_string(),
        ..Default::default()
    };
    let mut seen_some = false;
    for line in content.lines() {
        let mut f = fields(line);
        let kind = f.next();
        let mut avg = [0f64; 3];
        let mut total = 0u64;
        for token in f {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            match key {
                "avg10" => avg[0] = parse_num(value, path, "avg10")?,
                "avg60" => avg[1] = parse_num(value, path, "avg60")?,
                "avg300" => avg[2] = parse_num(value, path, "avg300")?,
                "total" => total = parse_num(value, path, "total")?,
                _ => {}
            }
        }
        match kind {
            Some("some") => {
                seen_some = true;
                stats.some_avg10 = avg[0];
                stats.some_avg60 = avg[1];
                stats.some_avg300 = avg[2];
                stats.some_total_us = total;
            }
            Some("full") => {
                stats.full_avg10 = Some(avg[0]);
                stats.full_avg60 = Some(avg[1]);
                stats.full_avg300 = Some(avg[2]);
                stats.full_total_us = Some(total);
            }
            _ => {}
        }
    }
    if !seen_some {
        return Err(CollectorError::missing(path, "some line"));
    }
    Ok(stats)
}

#[async_trait]
impl Collector for PressureCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        let mut stats = Vec::with_capacity(3);
        let mut absent = 0;
        for (metric_type, path) in RESOURCES {
            self.buf.clear();
            match super::procfs::read_into(std::path::Path::new(path), &mut self.buf) {
                Ok(()) => stats.push(parse(metric_type, path, &self.buf)?),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => absent += 1,
                Err(e) => return Err(CollectorError::read(path, e)),
            }
        }
        if absent == RESOURCES.len() {
            return Err(CollectorError::Unsupported(
                "pressure stall information is not available (CONFIG_PSI or psi=1 missing)",
            ));
        }
        Ok(Box::new(PressureBuffer { stats }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPU: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/pressure/cpu"
    ));
    const MEMORY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/pressure/memory"
    ));

    #[test]
    fn parses_real_psi_files() {
        let cpu = parse("cpu", "/proc/pressure/cpu", CPU).unwrap();
        assert_eq!(cpu.metric_type, "cpu");
        assert_eq!(cpu.some_avg300, 7.47);
        assert_eq!(cpu.some_total_us, 82_279_656);
        let mem = parse("memory", "/proc/pressure/memory", MEMORY).unwrap();
        assert_eq!(mem.full_total_us, Some(7282));
    }

    #[test]
    fn cpu_without_full_line_and_errors() {
        let s = parse(
            "cpu",
            "p",
            "some avg10=1.00 avg60=2.00 avg300=3.00 total=4\n",
        )
        .unwrap();
        assert_eq!((s.some_avg10, s.some_total_us), (1.0, 4));
        assert_eq!(s.full_avg10, None);
        assert!(parse(
            "cpu",
            "p",
            "full avg10=0.00 avg60=0.00 avg300=0.00 total=0\n"
        )
        .is_err());
        assert!(parse("cpu", "p", "some avg10=x avg60=0 avg300=0 total=0\n").is_err());
    }
}
