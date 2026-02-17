use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub data_dir: PathBuf,
    pub segment_size: u64,
    pub fsync_policy: FsyncPolicy,
    pub max_message_size: usize,
    pub enable_encryption: bool,
    pub enable_metrics: bool,
    pub shutdown_timeout: Duration,
    pub partition_count: usize,
    pub enable_mmap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq)]
pub enum FsyncPolicy {
    Immediate,
    Interval(u64),
    MessageCount(u64),
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./queue_data"),
            segment_size: 256 * 1024 * 1024, // 256MB
            fsync_policy: FsyncPolicy::Immediate,
            max_message_size: 100 * 1024 * 1024, // 100MB
            enable_encryption: cfg!(feature = "encryption"),
            enable_metrics: cfg!(feature = "metrics"),
            shutdown_timeout: Duration::from_secs(30),
            partition_count: num_cpus::get().min(16),
            enable_mmap: true,
        }
    }
}

impl QueueConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            ..Default::default()
        }
    }

    pub fn high_throughput() -> Self {
        Self {
            data_dir: PathBuf::from("./queue_data"),
            segment_size: 512 * 1024 * 1024,     // 512MB segments
            max_message_size: 200 * 1024 * 1024, // 200MB messages (39% of segment)
            fsync_policy: FsyncPolicy::Interval(1000), // Fsync every 1s
            enable_encryption: cfg!(feature = "encryption"),
            enable_metrics: cfg!(feature = "metrics"),
            shutdown_timeout: Duration::from_secs(30),
            partition_count: num_cpus::get().min(16),
            enable_mmap: true,
        }
    }

    pub fn low_latency() -> Self {
        Self {
            data_dir: PathBuf::from("./queue_data"),
            segment_size: 64 * 1024 * 1024,     // 64MB segments
            max_message_size: 24 * 1024 * 1024, // 24MB messages (37% of segment)
            fsync_policy: FsyncPolicy::Immediate,
            enable_encryption: cfg!(feature = "encryption"),
            enable_metrics: cfg!(feature = "metrics"),
            shutdown_timeout: Duration::from_secs(30),
            partition_count: num_cpus::get().min(16),
            enable_mmap: true,
        }
    }

    pub fn with_retention() -> Self {
        Self {
            data_dir: PathBuf::from("./queue_data"),
            segment_size: 100 * 1024 * 1024,    // 100MB segments
            max_message_size: 40 * 1024 * 1024, // 40MB messages (40% of segment)
            fsync_policy: FsyncPolicy::Interval(500),
            enable_encryption: cfg!(feature = "encryption"),
            enable_metrics: cfg!(feature = "metrics"),
            shutdown_timeout: Duration::from_secs(30),
            partition_count: num_cpus::get().min(16),
            enable_mmap: true,
        }
    }

    pub fn for_testing() -> Self {
        Self {
            data_dir: PathBuf::from("./queue_test"),
            segment_size: 10 * 1024 * 1024,    // 10MB segments
            max_message_size: 2 * 1024 * 1024, // 2MB messages (20% of segment)
            fsync_policy: FsyncPolicy::Interval(100),
            enable_encryption: cfg!(feature = "encryption"),
            enable_metrics: cfg!(feature = "metrics"),
            shutdown_timeout: Duration::from_secs(5),
            partition_count: 2,
            enable_mmap: true,
        }
    }

    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = path;
        self
    }

    pub fn with_segment_size(mut self, size: u64) -> Self {
        self.segment_size = size;
        self.max_message_size = (size / 3) as usize;
        self
    }

    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    pub fn with_sizes(mut self, segment_size: u64, max_msg_size: usize) -> Self {
        self.segment_size = segment_size;
        self.max_message_size = max_msg_size;
        self
    }

    pub fn with_fsync_policy(mut self, policy: FsyncPolicy) -> Self {
        self.fsync_policy = policy;
        self
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.segment_size < 1024 * 1024 {
            return Err(crate::QueueError::InvalidConfig(
                "Segment size must be at least 1 MB".to_string(),
            ));
        }

        let max_allowed = self.segment_size / 2;
        if self.max_message_size as u64 > max_allowed {
            return Err(crate::QueueError::InvalidConfig(
                format!(
                    "Max message size ({} MB) must be less than half segment size ({} MB).\n\
                    \n\
                    SOLUTIONS:\n\
                    1. Increase segment_size: config.with_segment_size({} * 1024 * 1024)\n\
                    2. Decrease max_message_size: config.with_max_message_size({} * 1024 * 1024)\n\
                    3. Use builder: config.with_segment_size({} * 1024 * 1024) // auto-adjusts max_msg_size\n\
                    4. Use preset: QueueConfig::high_throughput()\n",
                    self.max_message_size / (1024 * 1024),
                    max_allowed / (1024 * 1024),
                    (self.max_message_size as u64 * 3) / (1024 * 1024),
                    (max_allowed / 2) / (1024 * 1024),
                    (self.max_message_size as u64 * 3) / (1024 * 1024)
                ),
            ));
        }

        if self.partition_count == 0 {
            return Err(crate::QueueError::InvalidConfig(
                "Partition count must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}
