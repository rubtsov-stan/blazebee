//! Extended protocol counters from `/proc/net/netstat` (`TcpExt`, `IpExt`, ...).
//!
//! The file alternates a header line of names and a line of values for each
//! protocol group.

use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    sample, CollectFuture, Collector,
};

const PATH: &str = "/proc/net/netstat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NetstatEntry {
    /// Group name without the trailing colon, e.g. `TcpExt`.
    pub protocol: String,
    pub stat_name: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetstatBuffer {
    pub entries: Vec<NetstatEntry>,
}

pub struct NetstatCollector {
    file: ProcFile,
}

impl Default for NetstatCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> NetstatBuffer {
    let mut entries = Vec::with_capacity(160);
    let mut lines = content.lines();
    while let (Some(header), Some(values)) = (lines.next(), lines.next()) {
        let mut names = fields(header);
        let mut numbers = fields(values);
        let (Some(protocol), Some(_)) = (names.next(), numbers.next()) else {
            continue;
        };
        let protocol = protocol.trim_end_matches(':');
        for (name, value) in names.zip(numbers) {
            if let Ok(value) = value.parse::<u64>() {
                entries.push(NetstatEntry {
                    protocol: protocol.to_string(),
                    stat_name: name.to_string(),
                    value,
                });
            }
        }
    }
    NetstatBuffer { entries }
}

impl Collector for NetstatCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?))) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/net/netstat"
    ));

    #[test]
    fn parses_real_netstat() {
        let out = parse(FIXTURE);
        assert!(out
            .entries
            .iter()
            .any(|e| e.protocol == "TcpExt" && e.stat_name == "SyncookiesSent"));
        let in_octets = out
            .entries
            .iter()
            .find(|e| e.protocol == "IpExt" && e.stat_name == "InOctets")
            .unwrap();
        assert_eq!(in_octets.value, 40);
        assert!(out.entries.iter().all(|e| !e.protocol.ends_with(':')));
    }

    #[test]
    fn pairs_names_with_values() {
        let out = parse("X: a b c\nX: 1 2\n");
        assert_eq!(out.entries.len(), 2);
        assert_eq!(
            out.entries[1],
            NetstatEntry {
                protocol: "X".into(),
                stat_name: "b".into(),
                value: 2
            }
        );
    }
}
