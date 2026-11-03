//! Kernel entropy pool size from `/proc/sys/kernel/random/entropy_avail`.

use std::path::Path;

use serde::Serialize;

use super::{procfs::read_number, sample, CollectFuture, Collector};

const PATH: &str = "/proc/sys/kernel/random/entropy_avail";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EntropyStats {
    /// Bits of entropy the kernel estimates to be available.
    pub available_entropy: u32,
}

#[derive(Default)]
pub struct EntropyCollector;

impl Collector for EntropyCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move {
            let available_entropy = read_number(Path::new(PATH), "entropy_avail")?;
            Ok(sample(EntropyStats { available_entropy }))
        })
    }
}
