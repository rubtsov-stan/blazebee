//! IPVS totals from `/proc/net/ip_vs_stats`.
//!
//! The file has two header lines, one line of cumulative totals in
//! hexadecimal, a blank line and two lines of per-second rates. Only the
//! totals are reported.

use serde::Serialize;

use super::{
    procfs::{fields, ProcFile},
    sample, CollectFuture, Collector, CollectorError, CollectorResult,
};

const PATH: &str = "/proc/net/ip_vs_stats";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IpvsStats {
    pub connections: u64,
    pub incoming_packets: u64,
    pub outgoing_packets: u64,
    pub incoming_bytes: u64,
    pub outgoing_bytes: u64,
}

pub struct IpvsCollector {
    file: ProcFile,
}

impl Default for IpvsCollector {
    fn default() -> Self {
        Self {
            file: ProcFile::new(PATH),
        }
    }
}

pub fn parse(content: &str) -> CollectorResult<IpvsStats> {
    let line = content
        .lines()
        .nth(2)
        .ok_or_else(|| CollectorError::missing(PATH, "totals line"))?;
    let mut v = [0u64; 5];
    let mut f = fields(line);
    for (slot, what) in v.iter_mut().zip([
        "conns",
        "in_packets",
        "out_packets",
        "in_bytes",
        "out_bytes",
    ]) {
        let token = f
            .next()
            .ok_or_else(|| CollectorError::missing(PATH, what))?;
        *slot = u64::from_str_radix(token, 16).map_err(|_| {
            CollectorError::parse(PATH, what, format!("invalid hex value '{token}'"))
        })?;
    }
    Ok(IpvsStats {
        connections: v[0],
        incoming_packets: v[1],
        outgoing_packets: v[2],
        incoming_bytes: v[3],
        outgoing_bytes: v[4],
    })
}

impl Collector for IpvsCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_totals_and_ignores_rates() {
        let content = "\
   Total Incoming Outgoing         Incoming         Outgoing
   Conns  Packets  Packets            Bytes            Bytes
 0000000A 00000014 0000001E 00000000000003E8 00000000000007D0

 Conns/s   Pkts/s   Pkts/s          Bytes/s          Bytes/s
        1        2        3                4                5
";
        let s = parse(content).unwrap();
        assert_eq!(
            s,
            IpvsStats {
                connections: 10,
                incoming_packets: 20,
                outgoing_packets: 30,
                incoming_bytes: 1000,
                outgoing_bytes: 2000
            }
        );
    }

    #[test]
    fn rejects_truncated_file() {
        assert!(parse("a\nb\n").is_err());
        assert!(parse("a\nb\n 1 2 3\n").is_err());
    }
}
