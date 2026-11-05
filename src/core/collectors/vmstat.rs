//! Virtual memory counters from `/proc/vmstat`.

use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    sample, CollectFuture, Collector,
};

const PATH: &str = "/proc/vmstat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VmstatEntry {
    pub key: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VmstatBuffer {
    pub entries: Vec<VmstatEntry>,
}

pub struct VmstatCollector {
    file: ProcFile,
}

impl Default for VmstatCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

/// Lines that do not look like `name value` are skipped; this keeps the
/// collector working across kernel versions.
pub fn parse(content: &str) -> VmstatBuffer {
    let entries = content
        .lines()
        .filter_map(|line| {
            let mut f = fields(line);
            let key = f.next()?;
            let value = f.next()?.parse().ok()?;
            Some(VmstatEntry {
                key: key.to_string(),
                value,
            })
        })
        .collect();
    VmstatBuffer { entries }
}

impl Collector for VmstatCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?))) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/vmstat"
    ));

    #[test]
    fn parses_real_vmstat() {
        let out = parse(FIXTURE);
        assert!(out.entries.len() > 100);
        assert!(out
            .entries
            .iter()
            .any(|e| e.key == "pgfault" && e.value > 0));
    }

    #[test]
    fn skips_malformed_lines() {
        let out = parse("a 1\nbroken\nb x\nc 3\n");
        assert_eq!(out.entries.len(), 2);
        assert_eq!(out.entries[1].key, "c");
    }
}
