//! Connection tracking table usage from the `nf_conntrack` sysctls.
//!
//! `/proc/sys/net/netfilter/nf_conntrack_count` is a single integer; reading
//! it costs nothing, unlike counting the lines of `/proc/net/nf_conntrack`,
//! which lists every tracked flow and needs `CAP_NET_ADMIN`.

use std::path::Path;

use serde::Serialize;

use super::{procfs::read_number, sample, CollectFuture, Collector, CollectorError};

const COUNT: &str = "/proc/sys/net/netfilter/nf_conntrack_count";
const MAX: &str = "/proc/sys/net/netfilter/nf_conntrack_max";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConntrackStats {
    pub entries: u64,
    pub max: u64,
}

#[derive(Default)]
pub struct ConntrackCollector;

impl Collector for ConntrackCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move {
            let entries = match read_number(Path::new(COUNT), "nf_conntrack_count") {
                Err(CollectorError::Read { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    return Err(CollectorError::Unsupported("nf_conntrack is not loaded"));
                }
                other => other?,
            };
            let max = read_number(Path::new(MAX), "nf_conntrack_max")?;
            Ok(sample(ConntrackStats { entries, max }))
        })
    }
}
