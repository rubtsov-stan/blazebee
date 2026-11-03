//! System-wide file handle usage from `/proc/sys/fs/file-nr`.

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    Collector, CollectorResult, Sample,
};

const PATH: &str = "/proc/sys/fs/file-nr";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileFdStats {
    /// Allocated file handles.
    pub allocated: u64,
    /// Kernel limit (`fs.file-max`).
    pub maximum: u64,
}

pub struct FileFdCollector {
    file: ProcFile,
}

impl Default for FileFdCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<FileFdStats> {
    let mut f = fields(content);
    let allocated = next_field(&mut f, PATH, "allocated")?;
    let _unused: u64 = next_field(&mut f, PATH, "unused")?;
    let maximum = next_field(&mut f, PATH, "maximum")?;
    Ok(FileFdStats { allocated, maximum })
}

#[async_trait]
impl Collector for FileFdCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        Ok(Box::new(parse(self.file.read()?)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/sys/fs/file-nr"
    ));

    #[test]
    fn parses_real_file_nr() {
        let s = parse(FIXTURE).unwrap();
        assert_eq!(s.allocated, 1571);
        assert_eq!(s.maximum, 9_223_372_036_854_775_807);
    }

    #[test]
    fn rejects_short_content() {
        assert!(parse("1 0\n").is_err());
    }
}
