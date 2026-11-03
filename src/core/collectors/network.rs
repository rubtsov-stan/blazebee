//! Per-interface traffic counters from `/proc/net/dev`.
//!
//! Column layout after the interface name: receive `bytes packets errs drop
//! fifo frame compressed multicast`, then transmit `bytes packets errs drop
//! fifo colls carrier compressed`.

use async_trait::async_trait;
use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    Collector, CollectorResult, Sample,
};

const PATH: &str = "/proc/net/dev";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NetworkStats {
    pub interface: String,
    pub receive_bytes: u64,
    pub receive_packets: u64,
    pub receive_errors: u64,
    pub receive_drops: u64,
    pub transmit_bytes: u64,
    pub transmit_packets: u64,
    pub transmit_errors: u64,
    pub transmit_drops: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkBuffer {
    pub stats: Vec<NetworkStats>,
}

pub struct NetworkCollector {
    file: ProcFile,
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<NetworkBuffer> {
    let mut stats = Vec::with_capacity(8);
    for line in content.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let interface = name.trim();
        if interface.is_empty() {
            continue;
        }
        let mut f = fields(rest);
        let mut rx = [0u64; 8];
        for (slot, what) in rx.iter_mut().zip([
            "rx_bytes",
            "rx_packets",
            "rx_errs",
            "rx_drop",
            "rx_fifo",
            "rx_frame",
            "rx_compressed",
            "rx_multicast",
        ]) {
            *slot = next_field(&mut f, PATH, what)?;
        }
        let mut tx = [0u64; 4];
        for (slot, what) in tx
            .iter_mut()
            .zip(["tx_bytes", "tx_packets", "tx_errs", "tx_drop"])
        {
            *slot = next_field(&mut f, PATH, what)?;
        }
        stats.push(NetworkStats {
            interface: interface.to_string(),
            receive_bytes: rx[0],
            receive_packets: rx[1],
            receive_errors: rx[2],
            receive_drops: rx[3],
            transmit_bytes: tx[0],
            transmit_packets: tx[1],
            transmit_errors: tx[2],
            transmit_drops: tx[3],
        });
    }
    Ok(NetworkBuffer { stats })
}

#[async_trait]
impl Collector for NetworkCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        Ok(Box::new(parse(self.file.read()?)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/net/dev"
    ));

    #[test]
    fn transmit_counters_come_from_the_transmit_columns() {
        let out = parse(FIXTURE).unwrap();
        assert_eq!(out.stats.len(), 2);
        let eth0 = &out.stats[1];
        assert_eq!(eth0.interface, "eth0");
        assert_eq!(eth0.receive_bytes, 344);
        assert_eq!(eth0.receive_packets, 4);
        assert_eq!(eth0.transmit_bytes, 132);
        assert_eq!(eth0.transmit_packets, 2);
    }

    #[test]
    fn distinguishes_rx_fifo_from_tx_bytes() {
        let content = "h1\nh2\n  eth1: 100 1 2 3 4 5 6 7 800 9 10 11 12 13 14 15\n";
        let out = parse(content).unwrap();
        let s = &out.stats[0];
        assert_eq!((s.receive_bytes, s.receive_drops), (100, 3));
        assert_eq!(
            (
                s.transmit_bytes,
                s.transmit_packets,
                s.transmit_errors,
                s.transmit_drops
            ),
            (800, 9, 10, 11)
        );
    }

    #[test]
    fn truncated_line_is_an_error() {
        assert!(parse("h1\nh2\n  eth1: 100 1 2 3 4 5 6 7 800\n").is_err());
    }
}
