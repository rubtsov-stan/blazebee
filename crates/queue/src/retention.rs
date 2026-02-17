use std::{
    fs,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::{
    config::QueueConfig, error::Result, queue::Queue, state::QueueState, storage::SegmentMetadata,
    utils::current_time_ms,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub max_segment_size: Option<u64>,
    pub max_size_bytes: Option<u64>,
    pub max_messages: Option<u64>,
    pub max_age_seconds: Option<u64>,
    pub max_segments: Option<u64>,
    pub delete_policy: DeletePolicy,
    pub enable_auto_rotation: bool,
    pub check_interval_ms: u64,
    pub grace_period_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeletePolicy {
    DeleteOldest,
    ArchiveOldest,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_segment_size: Some(512 * 1024 * 1024),
            max_size_bytes: Some(100 * 1024 * 1024 * 1024),
            max_messages: None,
            max_age_seconds: Some(7 * 24 * 3600),
            max_segments: None,
            delete_policy: DeletePolicy::DeleteOldest,
            enable_auto_rotation: true,
            check_interval_ms: 1000,
            grace_period_ms: 300_000,
        }
    }
}

impl RetentionPolicy {
    pub fn no_limit() -> Self {
        Self {
            max_segment_size: None,
            max_size_bytes: None,
            max_messages: None,
            max_age_seconds: None,
            max_segments: None,
            delete_policy: DeletePolicy::DeleteOldest,
            enable_auto_rotation: false,
            check_interval_ms: 1000,
            grace_period_ms: 300_000,
        }
    }

    pub fn with_max_size(mut self, size: u64) -> Self {
        self.max_size_bytes = Some(size);
        self
    }

    pub fn with_max_messages(mut self, count: u64) -> Self {
        self.max_messages = Some(count);
        self
    }

    pub fn with_max_age(mut self, seconds: u64) -> Self {
        self.max_age_seconds = Some(seconds);
        self
    }

    pub fn with_max_segments(mut self, count: u64) -> Self {
        self.max_segments = Some(count);
        self
    }

    pub fn with_grace_period(mut self, ms: u64) -> Self {
        self.grace_period_ms = ms;
        self
    }
}

pub struct RetentionManager {
    policy: RetentionPolicy,
    config: QueueConfig,
}

impl RetentionManager {
    pub fn new(config: crate::config::QueueConfig, policy: RetentionPolicy) -> Self {
        Self { config, policy }
    }

    pub async fn check_and_cleanup(&self, state: &QueueState, queue: &Queue) -> Result<()> {
        for pid in 0..self.config.partition_count {
            let partition = state.partition(pid as u32);

            let committed = partition.committed_offset.load(Ordering::Acquire);
            let min_consumer = partition.min_consumer_offset();

            let segments = partition.sorted_segments();

            for segment in segments {
                if committed <= segment.end_offset {
                    continue;
                }

                if let Some(min_c) = min_consumer {
                    if min_c <= segment.end_offset {
                        continue;
                    }
                }

                let age = current_time_ms() - segment.created_at;
                if age < self.policy.grace_period_ms {
                    continue;
                }

                queue
                    .segment_cache()
                    .remove(&(pid as u32, segment.segment_id));

                if segment.file_path.exists() {
                    std::fs::remove_file(&segment.file_path)?;
                }

                state.unregister_segment((pid as u32).try_into().unwrap(), segment.segment_id);
            }
        }

        Ok(())
    }

    async fn can_delete_segment(&self, state: &QueueState, segment: &SegmentMetadata) -> bool {
        let committed = state.get_committed_offset();
        if committed < segment.end_offset {
            return false;
        }

        let now = current_time_ms();
        let age_ms = now.saturating_sub(segment.created_at);
        if age_ms < self.policy.grace_period_ms {
            debug!(
                "Segment {} is within grace period ({} ms / {} ms)",
                segment.segment_id, age_ms, self.policy.grace_period_ms
            );
            return false;
        }

        true
    }

    async fn delete_segment(
        &self,
        state: &QueueState,
        queue: &Queue,
        segment: &SegmentMetadata,
    ) -> Result<()> {
        queue
            .segment_cache()
            .remove(&(segment.partition_id as u32, segment.segment_id));

        state.unregister_segment(
            (segment.partition_id as u32).try_into().unwrap(),
            segment.segment_id,
        );

        info!(
            "Deleted segment {} from partition {}",
            segment.segment_id, segment.partition_id
        );

        Ok(())
    }

    pub fn get_utilization(&self, state: &QueueState) -> f64 {
        if let Some(max_size) = self.policy.max_size_bytes {
            let segments = state.get_all_segments();
            let current_size: u64 = segments.iter().map(|s| s.size_bytes).sum();
            (current_size as f64 / max_size as f64) * 100.0
        } else {
            0.0
        }
    }

    pub fn policy(&self) -> &RetentionPolicy {
        &self.policy
    }
}
