pub mod config;
#[cfg(feature = "encryption")]
pub mod encryption;
pub mod error;
pub mod offset;
pub mod queue;
pub mod recovery;
pub mod retention;
pub mod state;
pub mod storage;
pub mod utils;

pub use config::{FsyncPolicy, QueueConfig};
pub use error::{QueueError, Result};
pub use queue::RetentionInfo;
pub use retention::{DeletePolicy, RetentionManager, RetentionPolicy};
pub use state::QueueState;

pub mod prelude {
    pub use super::{
        config::{FsyncPolicy, QueueConfig},
        error::{QueueError, Result},
        offset::OffsetIndex,
        queue::Queue,
        retention::{DeletePolicy, RetentionManager, RetentionPolicy},
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_presets() {
        let high_tp = QueueConfig::high_throughput();
        high_tp.validate().unwrap();

        let low_lat = QueueConfig::low_latency();
        low_lat.validate().unwrap();

        let retention = QueueConfig::with_retention();
        retention.validate().unwrap();

        let testing = QueueConfig::for_testing();
        testing.validate().unwrap();
    }
}
