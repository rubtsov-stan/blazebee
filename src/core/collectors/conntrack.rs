//! Connection tracking table usage from the `nf_conntrack` sysctls.
//!
//! `/proc/sys/net/netfilter/nf_conntrack_count` is a single integer; reading
//! it costs nothing, unlike counting the lines of `/proc/net/nf_conntrack`,
//! which lists every tracked flow and needs `CAP_NET_ADMIN`.

use std::path::Path;

use async_trait::async_trait;
use serde::Serialize;

use super::{procfs::read_number, Collector, CollectorError, CollectorResult, Sample};

const COUNT: &str = "/proc/sys/net/netfilter/nf_conntrack_count";
const MAX: &str = "/proc/sys/net/netfilter/nf_conntrack_max";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConntrackStats {
    pub entries: u64,
    pub max: u64,
}

#[derive(Default)]
pub struct ConntrackCollector;

#[async_trait]
impl Collector for ConntrackCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        let entries = match read_number(Path::new(COUNT), "nf_conntrack_count") {
            Err(CollectorError::Read { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                return Err(CollectorError::Unsupported("nf_conntrack is not loaded"));
            }
            other => other?,
        };
        let max = read_number(Path::new(MAX), "nf_conntrack_max")?;
        Ok(Box::new(ConntrackStats { entries, max }))
    }
}
