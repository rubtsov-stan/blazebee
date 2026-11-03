//! Block device counters from `/proc/diskstats`.
//!
//! The kernel reports sectors in 512-byte units regardless of the device's
//! logical block size; byte fields below are converted accordingly.

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    Collector, CollectorResult, Sample,
};

const PATH: &str = "/proc/diskstats";
const SECTOR_BYTES: u64 = 512;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiskStats {
    pub device: String,
    pub reads_completed: u64,
    pub reads_merged: u64,
    pub read_bytes: u64,
    pub read_time_ms: u64,
    pub writes_completed: u64,
    pub writes_merged: u64,
    pub written_bytes: u64,
    pub write_time_ms: u64,
    pub io_in_progress: u64,
    pub io_time_ms: u64,
    /// Field 14: time in flight weighted by queue depth.
    pub weighted_io_time_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskBuffer {
    pub stats: Vec<DiskStats>,
}

pub struct DiskCollector {
    file: ProcFile,
}

impl Default for DiskCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<DiskBuffer> {
    let mut stats = Vec::with_capacity(16);
    for line in content.lines() {
        let mut f = fields(line);
        let (Some(_major), Some(_minor), Some(device)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let mut v = [0u64; 11];
        const NAMES: [&str; 11] = [
            "reads_completed",
            "reads_merged",
            "sectors_read",
            "read_time_ms",
            "writes_completed",
            "writes_merged",
            "sectors_written",
            "write_time_ms",
            "io_in_progress",
            "io_time_ms",
            "weighted_io_time_ms",
        ];
        for (slot, name) in v.iter_mut().zip(NAMES) {
            *slot = next_field(&mut f, PATH, name)?;
        }
        stats.push(DiskStats {
            device: device.to_string(),
            reads_completed: v[0],
            reads_merged: v[1],
            read_bytes: v[2].saturating_mul(SECTOR_BYTES),
            read_time_ms: v[3],
            writes_completed: v[4],
            writes_merged: v[5],
            written_bytes: v[6].saturating_mul(SECTOR_BYTES),
            write_time_ms: v[7],
            io_in_progress: v[8],
            io_time_ms: v[9],
            weighted_io_time_ms: v[10],
        });
    }
    Ok(DiskBuffer { stats })
}

#[async_trait]
impl Collector for DiskCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        Ok(Box::new(parse(self.file.read()?)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/diskstats"
    ));

    #[test]
    fn parses_real_diskstats_with_byte_conversion() {
        let out = parse(FIXTURE).unwrap();
        assert_eq!(out.stats.len(), 5);
        let vda = &out.stats[0];
        assert_eq!(vda.device, "vda");
        assert_eq!(vda.reads_completed, 18717);
        assert_eq!(vda.read_bytes, 2_601_934 * 512);
        assert_eq!(vda.written_bytes, 4_650_010 * 512);
        assert_eq!(vda.io_time_ms, 6626);
        assert_eq!(vda.weighted_io_time_ms, 33067);
    }

    #[test]
    fn short_lines_are_errors_and_blank_lines_ignored() {
        assert!(parse("   8       0 sda 1 2 3\n").is_err());
        assert!(parse("\n\n").unwrap().stats.is_empty());
    }
}
