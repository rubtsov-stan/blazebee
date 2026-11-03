//! Socket usage from `/proc/net/sockstat`.

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    Collector, CollectorResult, Sample,
};

const PATH: &str = "/proc/net/sockstat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SockstatEntry {
    /// `TCP`, `UDP`, `RAW`, `FRAG`, `UDPLITE` or `sockets`.
    pub protocol: String,
    /// `inuse`, `orphan`, `tw`, `alloc`, `mem`, `used`, ...
    pub metric: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SockstatBuffer {
    pub entries: Vec<SockstatEntry>,
}

pub struct SockstatCollector {
    file: ProcFile,
}

impl Default for SockstatCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> SockstatBuffer {
    let mut entries = Vec::with_capacity(16);
    for line in content.lines() {
        let mut f = fields(line);
        let Some(protocol) = f.next() else { continue };
        let protocol = protocol.trim_end_matches(':');
        while let (Some(metric), Some(value)) = (f.next(), f.next()) {
            if let Ok(value) = value.parse::<u64>() {
                entries.push(SockstatEntry {
                    protocol: protocol.to_string(),
                    metric: metric.to_string(),
                    value,
                });
            }
        }
    }
    SockstatBuffer { entries }
}

#[async_trait]
impl Collector for SockstatCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        Ok(Box::new(parse(self.file.read()?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/net/sockstat"
    ));

    #[test]
    fn parses_real_sockstat() {
        let out = parse(FIXTURE);
        let tcp_alloc = out
            .entries
            .iter()
            .find(|e| e.protocol == "TCP" && e.metric == "alloc")
            .unwrap();
        assert_eq!(tcp_alloc.value, 133);
        let used = out
            .entries
            .iter()
            .find(|e| e.protocol == "sockets" && e.metric == "used")
            .unwrap();
        assert_eq!(used.value, 0);
    }
}
