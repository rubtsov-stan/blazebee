//! Neighbour table entries from `/proc/net/arp`.

use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    sample, CollectFuture, Collector,
};

const PATH: &str = "/proc/net/arp";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArpEntry {
    pub ip_address: String,
    pub hw_type: String,
    pub flags: String,
    pub hw_address: String,
    pub mask: String,
    pub device: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArpBuffer {
    pub entries: Vec<ArpEntry>,
}

pub struct ArpCollector {
    file: ProcFile,
}

impl Default for ArpCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> ArpBuffer {
    let entries = content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut f = fields(line);
            Some(ArpEntry {
                ip_address: f.next()?.to_string(),
                hw_type: f.next()?.to_string(),
                flags: f.next()?.to_string(),
                hw_address: f.next()?.to_string(),
                mask: f.next()?.to_string(),
                device: f.next()?.to_string(),
            })
        })
        .collect();
    ArpBuffer { entries }
}

impl Collector for ArpCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?))) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_skips_header() {
        let content = "IP address       HW type     Flags       HW address            Mask     Device\n\
                       192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:ff     *        eth0\n\
                       192.168.1.7      0x1         0x0         00:00:00:00:00:00     *        eth0\n";
        let out = parse(content);
        assert_eq!(out.entries.len(), 2);
        assert_eq!(out.entries[0].hw_address, "aa:bb:cc:dd:ee:ff");
        assert_eq!(out.entries[1].flags, "0x0");
        assert!(parse("header only\n").entries.is_empty());
    }
}
