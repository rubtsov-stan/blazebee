use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::{Mutex, RwLock as TokioRwLock};
use tracing::{debug, info, trace};

use super::{
    config::{FsyncPolicy, QueueConfig},
    error::Result,
    recovery::RecoveryManager,
    retention::{RetentionManager, RetentionPolicy},
    state::QueueState,
    storage::Segment,
};

pub struct Queue {
    state: Arc<QueueState>,
    current_segments: Arc<DashMap<usize, Mutex<Option<Segment>>>>,
    recovery_manager: Arc<RecoveryManager>,
    fsync_interval: Arc<TokioRwLock<Option<tokio::task::JoinHandle<()>>>>,
    retention_manager: Arc<RetentionManager>,
    rotation_task: Arc<TokioRwLock<Option<tokio::task::JoinHandle<()>>>>,
    segment_cache: Arc<DashMap<(u32, u64), Arc<Segment>>>,
}

impl Queue {
    pub async fn new(config: QueueConfig) -> Result<Self> {
        Self::new_with_retention(config, RetentionPolicy::default()).await
    }

    pub async fn new_with_retention(
        config: QueueConfig,
        retention: RetentionPolicy,
    ) -> Result<Self> {
        config.validate()?;

        let state = Arc::new(QueueState::new(config.clone()));
        let recovery_manager = Arc::new(RecoveryManager::new(config.clone()));
        let retention_manager = Arc::new(RetentionManager::new(config.clone(), retention));

        // Recover segments and write offset
        recovery_manager.recover(&state).await?;

        let queue = Self {
            state,
            current_segments: Arc::new(DashMap::new()),
            recovery_manager,
            fsync_interval: Arc::new(TokioRwLock::new(None)),
            retention_manager,
            rotation_task: Arc::new(TokioRwLock::new(None)),
            segment_cache: Arc::new(DashMap::new()),
        };

        queue.start_fsync_background_task().await?;
        queue.start_rotation_background_task().await?;

        info!("Queue initialized successfully");
        Ok(queue)
    }

    pub fn segment_cache(&self) -> &Arc<DashMap<(u32, u64), Arc<Segment>>> {
        &self.segment_cache
    }
    pub async fn open(data_dir: PathBuf) -> Result<Self> {
        Self::new(QueueConfig::new(data_dir)).await
    }
    pub async fn append(&self, data: Bytes) -> Result<u64> {
        if self.state.is_closed() {
            return Err(crate::QueueError::QueueClosed);
        }

        if data.len() > self.state.config().max_message_size {
            return Err(crate::QueueError::MessageTooLarge {
                size: data.len(),
                max_size: self.state.config().max_message_size,
            });
        }

        let offset = self.state.next_write_offset();
        let partition_id = (offset as usize) % self.state.config().partition_count;

        trace!(
            "Appending at offset {} to partition {}",
            offset,
            partition_id
        );

        self.write_to_partition(partition_id, offset, data).await?;

        Ok(offset)
    }
    pub async fn append_batch(&self, messages: Vec<Bytes>) -> Result<Vec<u64>> {
        if self.state.is_closed() {
            return Err(crate::QueueError::QueueClosed);
        }

        let mut offsets = Vec::new();
        let mut batches: std::collections::BTreeMap<usize, Vec<(u64, Bytes)>> =
            std::collections::BTreeMap::new();

        for data in messages {
            if data.len() > self.state.config().max_message_size {
                return Err(crate::QueueError::MessageTooLarge {
                    size: data.len(),
                    max_size: self.state.config().max_message_size,
                });
            }

            let offset = self.state.next_write_offset();
            let partition_id = (offset as usize) % self.state.config().partition_count;

            offsets.push(offset);
            batches
                .entry(partition_id)
                .or_insert_with(Vec::new)
                .push((offset, data));
        }

        for (partition_id, batch) in batches {
            self.write_batch_to_partition(partition_id, batch).await?;
        }
        self.handle_fsync_policy().await?;

        Ok(offsets)
    }
    pub async fn read(&self, offset: u64) -> Result<Option<Bytes>> {
        let partition_id = (offset as usize) % self.state.config().partition_count;

        // Try to read from the partition that should contain this offset
        if let Ok(Some(msg)) = self.read_from_partition(offset, partition_id).await {
            return Ok(Some(msg));
        }

        // If not found in the expected partition, try all partitions (for recovery/testing)
        for pid in 0..self.state.config().partition_count {
            if pid != partition_id {
                if let Ok(Some(msg)) = self.read_from_partition(offset, pid).await {
                    return Ok(Some(msg));
                }
            }
        }

        Ok(None)
    }

    pub async fn read_from_partition(
        &self,
        offset: u64,
        partition_id: usize,
    ) -> Result<Option<Bytes>> {
        // Check current_segments first
        if let Some(segment_guard) = self.current_segments.get(&partition_id) {
            let guard = segment_guard.lock().await;
            if let Some(ref segment) = *guard {
                let meta = segment.metadata();
                if offset >= meta.start_offset && offset <= meta.end_offset {
                    return segment.read_by_offset(offset);
                }
            }
        }

        // Check persisted segments
        let segments = self.state.get_segments_for_partition(partition_id as u32);

        for seg_meta in segments {
            if offset >= seg_meta.start_offset && offset <= seg_meta.end_offset {
                let key = (partition_id as u32, seg_meta.segment_id);

                let segment = if let Some(cached) = self.segment_cache.get(&key) {
                    cached.clone()
                } else {
                    let seg = Arc::new(Segment::open(
                        seg_meta.file_path.clone(),
                        seg_meta.segment_id,
                        partition_id,
                        seg_meta.start_offset,
                        self.state.config().enable_mmap,
                    )?);

                    self.segment_cache.insert(key, seg.clone());
                    seg
                };

                return segment.read_by_offset(offset);
            }
        }

        Ok(None)
    }
    pub async fn create_consumer_group(&self, group_id: String) {
        self.state.create_consumer_group(group_id);
    }

    pub async fn commit_offset(&self, group_id: &str, partition_id: usize, offset: u64) {
        self.state
            .update_consumer_offset(group_id, partition_id, offset);
    }

    pub async fn get_consumer_offset(&self, group_id: &str, partition_id: usize) -> u64 {
        self.state.get_consumer_offset(group_id, partition_id)
    }

    pub async fn fetch(
        &self,
        group_id: &str,
        partition_id: usize,
        max_bytes: usize,
    ) -> Result<Vec<Bytes>> {
        let offset = self.state.get_consumer_offset(group_id, partition_id);
        let committed = self.state.get_partition_committed(partition_id as u32);

        let mut messages = Vec::new();
        let mut current_offset = offset;
        let mut bytes_read = 0;

        while current_offset <= committed && bytes_read < max_bytes {
            match self
                .read_from_partition(current_offset, partition_id)
                .await?
            {
                Some(msg) => {
                    bytes_read += msg.len();
                    messages.push(msg);
                    current_offset += 1;
                }
                None => break,
            }
        }

        Ok(messages)
    }

    pub fn get_retention_info(&self) -> RetentionInfo {
        let total_size = self.state.get_total_size();
        let utilization = self.retention_manager.get_utilization(&self.state);
        let segments = self.state.get_all_segments();

        RetentionInfo {
            total_size_bytes: total_size,
            segment_count: segments.len() as u64,
            utilization_percent: utilization,
            policy: self.retention_manager.policy().clone(),
        }
    }

    pub fn state(&self) -> &Arc<QueueState> {
        &self.state
    }
    pub async fn shutdown(&self) -> Result<()> {
        info!("Queue shutdown initiated");
        self.state.mark_closed();

        for mut entry in self.current_segments.iter_mut() {
            let mut guard = entry.value_mut().lock().await;
            if let Some(ref mut segment) = *guard {
                segment.flush_and_refresh_mmap()?;
                segment.persist_metadata()?;
                self.state.register_segment(segment.metadata().clone());
                segment.fsync()?;
            }
        }

        // Persist state
        self.state.persist_write_offset()?;
        self.state.persist_consumer_offsets()?;

        if let Some(handle) = self.fsync_interval.write().await.take() {
            handle.abort();
        }

        if let Some(handle) = self.rotation_task.write().await.take() {
            handle.abort();
        }

        info!("Queue shutdown complete");
        Ok(())
    }

    async fn write_to_partition(
        &self,
        partition_id: usize,
        offset: u64,
        data: Bytes,
    ) -> Result<()> {
        self.write_batch_to_partition(partition_id, vec![(offset, data)])
            .await
    }
    async fn write_batch_to_partition(
        &self,
        partition_id: usize,
        batch: Vec<(u64, Bytes)>,
    ) -> Result<()> {
        let segment_guard = self
            .current_segments
            .entry(partition_id)
            .or_insert_with(|| Mutex::new(None));

        let mut guard = segment_guard.lock().await;

        // Log what we're writing
        trace!(
            "Writing to partition {}: offsets {:?}",
            partition_id,
            batch.iter().map(|(offset, _)| offset).collect::<Vec<_>>()
        );

        if guard.is_none() {
            *guard = Some(self.create_new_segment(partition_id).await?);
            println!("Created new segment for partition {}", partition_id);
        }

        // Check if we need to rotate
        let should_rotate = guard
            .as_ref()
            .map(|seg| {
                let size = seg.metadata().size_bytes;
                let limit = self.state.config().segment_size;
                size >= limit
            })
            .unwrap_or(false);

        if should_rotate {
            trace!("Rotating segment for partition {}", partition_id);
            if let Some(ref mut segment) = *guard {
                segment.seal()?;
                segment.flush_and_refresh_mmap()?;
                segment.persist_metadata()?;
                self.state.register_segment(segment.metadata().clone());
            }
            *guard = Some(self.create_new_segment(partition_id).await?);
        }

        if let Some(ref mut segment) = *guard {
            // Write the batch
            segment.append_batch(&batch)?;

            // Force sync to disk
            segment.flush_and_refresh_mmap()?;

            // Update metadata
            let metadata = segment.metadata().clone();
            debug!(
                "After write: segment end_offset={}, count={}",
                metadata.end_offset, metadata.message_count
            );

            self.state.register_segment(metadata);

            // Update committed offset for this partition
            let end_offset = segment.metadata().end_offset;
            let partition_id_u32 = partition_id as u32;
            self.state
                .set_partition_committed(partition_id_u32, end_offset);

            // Update global committed offset
            self.state.update_committed_offset(end_offset);
        }

        Ok(())
    }
    async fn create_new_segment(&self, partition_id: usize) -> Result<Segment> {
        let segment_id = self.state.next_segment_id(partition_id);
        let partition_dir = self
            .state
            .config()
            .data_dir
            .join(format!("partition-{}", partition_id));

        std::fs::create_dir_all(&partition_dir)?;

        let filename = format!("{}.log", segment_id);
        let file_path = partition_dir.join(&filename);

        // Get the next offset for this partition from the state
        let partition = self.state.partition(partition_id as u32);
        let last_committed = partition.committed_offset.load(Ordering::Acquire);
        let next_offset = last_committed + 1;

        info!(
            "Creating segment {} for partition {} with next_offset={} (last_committed={})",
            segment_id, partition_id, next_offset, last_committed
        );

        let segment = Segment::open(
            file_path,
            segment_id,
            partition_id,
            next_offset,
            self.state.config().enable_mmap,
        )?;

        // Register the segment
        self.state.register_segment(segment.metadata().clone());

        Ok(segment)
    }

    async fn handle_fsync_policy(&self) -> Result<()> {
        match self.state.config().fsync_policy {
            FsyncPolicy::Immediate => {
                for mut entry in self.current_segments.iter_mut() {
                    let mut guard = entry.value_mut().lock().await;
                    if let Some(ref mut segment) = *guard {
                        segment.flush_and_refresh_mmap()?;
                        segment.persist_metadata()?;

                        let end_offset = segment.metadata().end_offset;
                        let partition_id = segment.metadata().partition_id as u32;
                        self.state.set_partition_committed(partition_id, end_offset);
                        self.state.register_segment(segment.metadata().clone());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn start_fsync_background_task(&self) -> Result<()> {
        if let FsyncPolicy::Interval(ms) = self.state.config().fsync_policy {
            let queue = self.clone();

            let handle = tokio::spawn(async move {
                let interval = Duration::from_millis(ms);

                loop {
                    tokio::time::sleep(interval).await;

                    for entry in queue.current_segments.iter() {
                        let mut guard = entry.value().lock().await;
                        if let Some(ref mut segment) = *guard {
                            if segment.fsync().is_ok() {
                                segment.metadata_mut().persisted_high_watermark =
                                    segment.metadata().end_offset;

                                let partition = queue
                                    .state
                                    .partition(segment.metadata().partition_id as u32);

                                partition
                                    .committed_offset
                                    .store(segment.metadata().end_offset, Ordering::Release);

                                let _ = segment.persist_metadata();
                                let _ = segment.flush_and_refresh_mmap();
                            }
                        }
                    }
                }
            });

            *self.fsync_interval.write().await = Some(handle);
        }

        Ok(())
    }

    async fn start_rotation_background_task(&self) -> Result<()> {
        let queue_clone = self.clone();
        let policy = self.retention_manager.policy().clone();

        let handle = tokio::spawn(async move {
            let interval = Duration::from_millis(policy.check_interval_ms);
            loop {
                tokio::time::sleep(interval).await;

                if let Err(e) = queue_clone
                    .retention_manager
                    .check_and_cleanup(&queue_clone.state, &queue_clone)
                    .await
                {
                    tracing::error!("Rotation check failed: {}", e);
                }
            }
        });

        *self.rotation_task.write().await = Some(handle);
        Ok(())
    }
}

impl Clone for Queue {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            current_segments: self.current_segments.clone(),
            recovery_manager: self.recovery_manager.clone(),
            fsync_interval: self.fsync_interval.clone(),
            retention_manager: self.retention_manager.clone(),
            rotation_task: self.rotation_task.clone(),
            segment_cache: self.segment_cache.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetentionInfo {
    pub total_size_bytes: u64,
    pub segment_count: u64,
    pub utilization_percent: f64,
    pub policy: RetentionPolicy,
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn test_queue_basic() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());
        let queue = Queue::new(config).await.unwrap();

        let data = Bytes::from("hello");
        let offset = queue.append(data).await.unwrap();
        assert_eq!(offset, 0);

        queue.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_append() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());
        let queue = Queue::new(config).await.unwrap();

        let messages = vec![
            Bytes::from("msg1"),
            Bytes::from("msg2"),
            Bytes::from("msg3"),
        ];

        let offsets = queue.append_batch(messages).await.unwrap();
        assert_eq!(offsets.len(), 3);

        queue.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_consumer_groups() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());
        let queue = Queue::new(config).await.unwrap();

        queue.create_consumer_group("group1".to_string()).await;

        for i in 0..50 {
            queue
                .append(Bytes::from(format!("Msg {}", i)))
                .await
                .unwrap();
        }

        queue.commit_offset("group1", 0, 25).await;
        let offset = queue.get_consumer_offset("group1", 0).await;

        assert_eq!(offset, 25);

        queue.shutdown().await.unwrap();
    }
}
#[cfg(test)]
mod queue_tests {
    use serial_test::serial;
    use tempfile::TempDir;

    use super::*;

    fn create_test_config() -> (TempDir, QueueConfig) {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());
        (temp_dir, config)
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_append_and_read() {
        let (_temp_dir, config) = create_test_config();
        println!("Test directory: {:?}", _temp_dir.path());

        let queue = Queue::new(config.clone()).await.unwrap();

        let data = Bytes::from("test message");
        let offset = queue.append(data.clone()).await.unwrap();
        assert_eq!(offset, 0, "First message should have offset 0");

        // Force sync to disk
        queue.shutdown().await.unwrap();

        // Reopen to test persistence
        let queue = Queue::new(config).await.unwrap();
        let read_data = queue.read(offset).await.unwrap();
        assert_eq!(
            read_data,
            Some(data),
            "Message at offset {} should be readable",
            offset
        );

        queue.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_append_batch_ordering() {
        let (_temp_dir, config) = create_test_config();
        println!("Test directory: {:?}", _temp_dir.path());

        let queue = Queue::new(config.clone()).await.unwrap();

        let messages = vec![
            Bytes::from("first"),
            Bytes::from("second"),
            Bytes::from("third"),
        ];

        let offsets = queue.append_batch(messages.clone()).await.unwrap();
        assert_eq!(offsets.len(), 3);
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 1);
        assert_eq!(offsets[2], 2);

        // Force sync and reopen
        queue.shutdown().await.unwrap();
        let queue = Queue::new(config.clone()).await.unwrap();

        // Read after reopen
        for (i, expected) in messages.iter().enumerate() {
            let read = queue.read(offsets[i]).await.unwrap();
            assert_eq!(
                read,
                Some(expected.clone()),
                "Failed at offset {}",
                offsets[i]
            );
        }

        queue.shutdown().await.unwrap();
    }
    #[tokio::test]
    #[serial]
    async fn test_queue_consumer_group_fetch() {
        let (_temp_dir, config) = create_test_config();
        println!("Test directory: {:?}", _temp_dir.path());

        // First session - write messages and commit offset
        let offset_to_commit = 25;
        {
            let queue = Queue::new(config.clone()).await.unwrap();
            queue.create_consumer_group("test_group".to_string()).await;

            for i in 0..50 {
                queue
                    .append(Bytes::from(format!("Msg{}", i)))
                    .await
                    .unwrap();
            }

            // Commit offset and verify
            queue.commit_offset("test_group", 0, offset_to_commit).await;
            let offset = queue.get_consumer_offset("test_group", 0).await;
            assert_eq!(offset, offset_to_commit, "Offset should be committed");

            // Force sync and shutdown
            queue.shutdown().await.unwrap();
        }

        // Second session - verify offset is persisted
        {
            let queue = Queue::new(config.clone()).await.unwrap();

            // Don't recreate the group - it should be loaded from persistence
            println!("After recovery, checking consumer groups:");
            let recovered_offset = queue.get_consumer_offset("test_group", 0).await;
            assert_eq!(
                recovered_offset, offset_to_commit,
                "Committed offset should be persisted"
            );

            queue.shutdown().await.unwrap();
        }
    }
    #[tokio::test]
    #[serial]
    async fn test_queue_multiple_partitions() {
        let (_temp_dir, mut config) = create_test_config();
        config.partition_count = 4;
        println!(
            "Test directory: {:?}, partitions: {}",
            _temp_dir.path(),
            config.partition_count
        );

        let queue = Queue::new(config.clone()).await.unwrap();

        for i in 0..100 {
            queue
                .append(Bytes::from(format!("msg{}", i)))
                .await
                .unwrap();
        }

        // Force sync
        queue.shutdown().await.unwrap();
        let queue = Queue::new(config).await.unwrap();

        // All messages should be readable
        for i in 0..100 {
            let data = queue.read(i).await.unwrap();
            assert!(data.is_some(), "Message at offset {} should exist", i);
            assert_eq!(data.unwrap(), Bytes::from(format!("msg{}", i)));
        }

        queue.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_batch_with_large_messages() {
        let (_temp_dir, config) = create_test_config();
        println!("Test directory: {:?}", _temp_dir.path());

        let queue = Queue::new(config.clone()).await.unwrap();

        let messages = vec![
            Bytes::from(vec![1u8; 100]),
            Bytes::from(vec![2u8; 200]),
            Bytes::from(vec![3u8; 300]),
        ];

        let offsets = queue.append_batch(messages.clone()).await.unwrap();
        assert_eq!(offsets.len(), 3);

        // Force sync
        queue.shutdown().await.unwrap();
        let queue = Queue::new(config).await.unwrap();

        // Read after recovery
        for (i, expected) in messages.iter().enumerate() {
            let read = queue.read(offsets[i]).await.unwrap();
            assert_eq!(
                read,
                Some(expected.clone()),
                "Failed at offset {}",
                offsets[i]
            );
        }

        queue.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_persistence_after_shutdown() {
        let (temp_dir, config) = create_test_config();
        println!("Test directory: {:?}", temp_dir.path());

        // First session
        {
            let queue = Queue::new(config.clone()).await.unwrap();
            queue.append(Bytes::from("persistent")).await.unwrap();
            queue.shutdown().await.unwrap();
        }

        // Small delay to ensure files are flushed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Second session - should recover data
        {
            let queue = Queue::new(config.clone()).await.unwrap();

            let data = queue.read(0).await.unwrap();
            assert!(data.is_some(), "Persisted message should be readable");
            assert_eq!(data.unwrap(), Bytes::from("persistent"));

            queue.shutdown().await.unwrap();
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_retention_info() {
        let (_temp_dir, config) = create_test_config();
        println!("Test directory: {:?}", _temp_dir.path());

        let queue = Queue::new(config).await.unwrap();

        for i in 0..10 {
            queue
                .append(Bytes::from(format!("msg{}", i)))
                .await
                .unwrap();
        }

        // Give time for metadata updates
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let info = queue.get_retention_info();
        assert!(info.total_size_bytes > 0, "Total size should be > 0");
        assert!(info.segment_count >= 1);
        assert!(info.utilization_percent >= 0.0);

        queue.shutdown().await.unwrap();
    }
}
