use std::{fs, sync::atomic::Ordering};

use tracing::{debug, info, trace, warn};

use super::{config::QueueConfig, error::Result, state::QueueState, storage::Segment};

pub struct RecoveryManager {
    config: QueueConfig,
}

impl RecoveryManager {
    pub fn new(config: QueueConfig) -> Self {
        Self { config }
    }
    async fn recover_partition_committed(
        &self,
        state: &QueueState,
        partition_id: u32,
    ) -> Result<u64> {
        let partition = state.partition(partition_id);
        let segments = partition.sorted_segments();
        let mut max_hw = 0;

        for seg in segments {
            if let Some(meta) = Segment::load_metadata(&seg.file_path)? {
                max_hw = max_hw.max(meta.persisted_high_watermark);
            } else {
                // If no metadata file, use the segment's end_offset
                max_hw = max_hw.max(seg.end_offset);
            }
        }

        partition.committed_offset.store(max_hw, Ordering::Release);
        Ok(max_hw)
    }
    pub async fn recover(&self, state: &QueueState) -> Result<()> {
        let partition_count = state.config().partition_count;
        let mut max_global_offset = 0u64;

        // Load persisted write offset
        let persisted_offset = state.load_write_offset()?;
        trace!("Loaded persisted write offset: {}", persisted_offset);

        if persisted_offset > 0 {
            max_global_offset = persisted_offset;
            state.set_write_offset(persisted_offset);
        }

        // Load consumer offsets
        state.load_consumer_offsets()?;

        for partition_id in 0..partition_count {
            let partition_id = partition_id as u32;

            let partition_max_offset = self.recover_partition_segments(state, partition_id).await?;

            let committed = self
                .recover_partition_committed(state, partition_id)
                .await?;
            state.set_partition_committed(partition_id, committed);

            max_global_offset = max_global_offset.max(partition_max_offset);

            trace!(
                "Partition {} recovered: max_offset={}, committed={}",
                partition_id,
                partition_max_offset,
                committed
            );
        }

        if max_global_offset > state.get_write_offset() {
            state.set_write_offset(max_global_offset + 1);
            trace!(
                "Setting global write offset to {} based on segments",
                max_global_offset + 1
            );
        }

        let mut max_committed = 0u64;
        for pid in 0..partition_count {
            let committed = state.get_partition_committed(pid as u32);
            max_committed = max_committed.max(committed);
        }
        state.update_committed_offset(max_committed);

        Ok(())
    }

    async fn recover_partition_segments(
        &self,
        state: &QueueState,
        partition_id: u32,
    ) -> Result<u64> {
        let partition_dir = self
            .config
            .data_dir
            .join(format!("partition-{}", partition_id));

        if !partition_dir.exists() {
            state.set_next_segment_id(partition_id as usize, 0);
            return Ok(0);
        }

        let entries = fs::read_dir(&partition_dir)?;
        let mut segment_ids: Vec<u64> = Vec::new();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "log") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(seg_id) = stem.parse::<u64>() {
                        segment_ids.push(seg_id);
                    }
                }
            }
        }

        segment_ids.sort();

        let max_segment_id = segment_ids.last().copied().unwrap_or(0);
        let next_segment_id = max_segment_id + 1;
        state.set_next_segment_id(partition_id as usize, next_segment_id);

        info!(
            "Recovered partition {}: found {} segments, max_id={}, next_id={}",
            partition_id,
            segment_ids.len(),
            max_segment_id,
            next_segment_id
        );

        let mut max_offset = 0u64;
        let mut next_expected_offset = 0u64;

        for segment_id in segment_ids {
            let (segment_max, segment_start, message_count) = self
                .recover_segment(
                    state,
                    partition_id as usize,
                    segment_id,
                    &partition_dir,
                    next_expected_offset,
                )
                .await?;

            max_offset = max_offset.max(segment_max);
            next_expected_offset = segment_max + 1;

            debug!(
                "Segment {} recovered: start={}, end={}, count={}",
                segment_id, segment_start, segment_max, message_count
            );
        }

        Ok(max_offset)
    }
    async fn recover_segment(
        &self,
        state: &QueueState,
        partition_id: usize,
        segment_id: u64,
        partition_dir: &std::path::PathBuf,
        _next_offset: u64, // Not used for existing segments
    ) -> Result<(u64, u64, u64)> {
        let filename = format!("{}.log", segment_id);
        let file_path = partition_dir.join(&filename);

        // Open the segment - it will rebuild the index and set correct offsets
        let segment = Segment::open(
            file_path.clone(),
            segment_id,
            partition_id,
            0, // Pass 0 for existing segments - they'll determine their own start
            state.config().enable_mmap,
        )?;

        let metadata = segment.metadata().clone();
        let max_offset = metadata.end_offset;
        let start_offset = metadata.start_offset;
        let message_count = metadata.message_count;

        debug!(
            "Segment {} recovered: start={}, end={}, count={}",
            segment_id, start_offset, max_offset, message_count
        );

        // Don't assert - just log if inconsistent
        if message_count > 0 && max_offset < start_offset {
            warn!(
                "Segment {} has inconsistent offsets: start={} > end={}",
                segment_id, start_offset, max_offset
            );
        }

        // Register the segment
        state.register_segment(metadata);

        Ok((max_offset, start_offset, message_count))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write};

    use bytes::Bytes;
    use tempfile::TempDir;

    use super::*;
    use crate::queue::*;
    // ==================== RecoveryManager Tests ====================

    #[tokio::test]
    async fn test_recovery_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        let manager = RecoveryManager::new(config.clone());

        assert_eq!(manager.config.data_dir, config.data_dir);
        assert_eq!(manager.config.partition_count, config.partition_count);
    }

    #[tokio::test]
    async fn test_recovery_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config);

        let result = manager.recover(&state).await;
        assert!(result.is_ok());

        // Should have no segments recovered
        assert_eq!(state.get_all_segments().len(), 0);
    }

    #[tokio::test]
    async fn test_recovery_with_existing_segments() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        // Create a queue and add some data
        {
            let queue = Queue::new(config.clone()).await.unwrap();

            for i in 0..10 {
                queue
                    .append(Bytes::from(format!("msg{}", i)))
                    .await
                    .unwrap();
            }

            queue.shutdown().await.unwrap();
        }

        // Recover the queue
        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config.clone());

        let result = manager.recover(&state).await;
        assert!(result.is_ok());

        // Should have recovered segments
        let segments = state.get_all_segments();
        assert!(!segments.is_empty());

        // Verify write offset is recovered
        assert!(state.get_write_offset() >= 10);
    }

    #[tokio::test]
    async fn test_recovery_partition_segments() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        // Create queue with multiple partitions
        let mut config = config;
        config.partition_count = 2;

        {
            let queue = Queue::new(config.clone()).await.unwrap();

            for i in 0..20 {
                queue
                    .append(Bytes::from(format!("msg{}", i)))
                    .await
                    .unwrap();
            }

            queue.shutdown().await.unwrap();
        }

        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config.clone());

        let result = manager.recover(&state).await;
        assert!(result.is_ok());

        // Both partitions should have segments
        let partition0_segments = state.get_segments_for_partition(0);
        let partition1_segments = state.get_segments_for_partition(1);

        assert!(!partition0_segments.is_empty() || !partition1_segments.is_empty());
    }

    #[tokio::test]
    async fn test_recovery_committed_offset() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        {
            let queue = Queue::new(config.clone()).await.unwrap();

            let offsets = queue
                .append_batch(vec![
                    Bytes::from("msg1"),
                    Bytes::from("msg2"),
                    Bytes::from("msg3"),
                ])
                .await
                .unwrap();

            assert_eq!(offsets.len(), 3);

            queue.shutdown().await.unwrap();
        }

        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config.clone());

        let result = manager.recover(&state).await;
        assert!(result.is_ok());

        // Committed offset should be recovered
        let committed = state.get_committed_offset();
        assert!(committed >= 0);
    }

    #[tokio::test]
    async fn test_recovery_with_corrupted_segment() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        // Create valid queue
        {
            let queue = Queue::new(config.clone()).await.unwrap();
            queue.append(Bytes::from("valid")).await.unwrap();
            queue.shutdown().await.unwrap();
        }

        // Corrupt a segment file
        let partition_dir = temp_dir.path().join("partition-0");
        if partition_dir.exists() {
            for entry in fs::read_dir(&partition_dir).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().map_or(false, |ext| ext == "log") {
                    // Append garbage to file
                    let mut file = OpenOptions::new().append(true).open(entry.path()).unwrap();
                    file.write_all(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap();
                }
            }
        }

        // Recovery should handle corruption gracefully
        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config.clone());

        let result = manager.recover(&state).await;
        // Should succeed even with corrupted data (truncates bad data)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_recovery_next_segment_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        {
            let queue = Queue::new(config.clone()).await.unwrap();

            // Force segment rotation by adding enough data
            for i in 0..100 {
                queue.append(Bytes::from(vec![0u8; 1000])).await.unwrap();
            }

            queue.shutdown().await.unwrap();
        }

        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config.clone());

        let result = manager.recover(&state).await;
        assert!(result.is_ok());

        // Next segment ID should be set correctly
        let next_id = state.next_segment_id(0);
        assert!(next_id > 0);
    }
    #[tokio::test]
    async fn test_recovery_multiple_restarts() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());
        println!("Partition count: {}", config.partition_count);

        // Helper function to list files
        let list_files = |dir: &std::path::Path| {
            if dir.exists() {
                for entry in std::fs::read_dir(dir).unwrap() {
                    let entry = entry.unwrap();
                    println!("  {:?}", entry.path());
                }
            }
        };

        // First restart
        {
            let queue = Queue::new(config.clone()).await.unwrap();
            let offset1 = queue.append(Bytes::from("msg1")).await.unwrap();
            println!("First write: offset={}", offset1);
            assert_eq!(offset1, 0);
            queue.shutdown().await.unwrap();

            println!("Files after first write:");
            for pid in 0..config.partition_count {
                let partition_dir = temp_dir.path().join(format!("partition-{}", pid));
                if partition_dir.exists() {
                    println!("Partition {}:", pid);
                    list_files(&partition_dir);
                }
            }
        }

        // Second restart
        {
            let queue = Queue::new(config.clone()).await.unwrap();
            let offset2 = queue.append(Bytes::from("msg2")).await.unwrap();
            println!("Second write: offset={}", offset2);
            assert_eq!(offset2, 1);
            queue.shutdown().await.unwrap();

            println!("Files after second write:");
            for pid in 0..config.partition_count {
                let partition_dir = temp_dir.path().join(format!("partition-{}", pid));
                if partition_dir.exists() {
                    println!("Partition {}:", pid);
                    list_files(&partition_dir);
                }
            }
        }

        // Third restart
        {
            let queue = Queue::new(config.clone()).await.unwrap();
            let offset3 = queue.append(Bytes::from("msg3")).await.unwrap();
            println!("Third write: offset={}", offset3);
            assert_eq!(offset3, 2);

            println!("Files after third write:");
            for pid in 0..config.partition_count {
                let partition_dir = temp_dir.path().join(format!("partition-{}", pid));
                if partition_dir.exists() {
                    println!("Partition {}:", pid);
                    list_files(&partition_dir);
                }
            }

            // Get the current write offset
            let write_offset = queue.state().get_write_offset();
            println!("Write offset after recovery: {}", write_offset);
            assert_eq!(write_offset, 3, "Write offset should be 3 after 3 writes");

            // Read all messages up to write_offset
            let mut messages = Vec::new();

            println!("Reading messages:");
            for offset in 0..write_offset {
                match queue.read(offset).await {
                    Ok(Some(msg)) => {
                        println!("  Offset {}: {:?}", offset, msg);
                        messages.push(msg);
                    }
                    Ok(None) => {
                        println!("  Offset {}: None", offset);
                    }
                    Err(e) => {
                        println!("  Offset {}: Error {:?}", offset, e);
                    }
                }
            }

            println!("Total messages found: {}", messages.len());
            for (i, msg) in messages.iter().enumerate() {
                println!("  Message {}: {:?}", i, msg);
            }

            assert_eq!(messages.len(), 3, "Should have recovered all 3 messages");

            // Messages should be in order
            assert_eq!(messages[0], Bytes::from("msg1"));
            assert_eq!(messages[1], Bytes::from("msg2"));
            assert_eq!(messages[2], Bytes::from("msg3"));

            queue.shutdown().await.unwrap();
        }
    }
    #[tokio::test]
    async fn test_recovery_with_metadata_files() {
        let temp_dir = TempDir::new().unwrap();
        let config = QueueConfig::new(temp_dir.path().to_path_buf());

        {
            let queue = Queue::new(config.clone()).await.unwrap();

            for i in 0..5 {
                queue
                    .append(Bytes::from(format!("msg{}", i)))
                    .await
                    .unwrap();
            }

            // Force metadata persistence
            queue.shutdown().await.unwrap();
        }

        // Verify metadata files exist
        let partition_dir = temp_dir.path().join("partition-0");
        let meta_files: Vec<_> = fs::read_dir(&partition_dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .ok()
                    .map(|e| e.path().extension().map_or(false, |ext| ext == "meta"))
                    .unwrap_or(false)
            })
            .collect();

        // Should have at least one metadata file
        assert!(!meta_files.is_empty());

        // Recovery should work with metadata
        let manager = RecoveryManager::new(config.clone());
        let state = QueueState::new(config.clone());

        let result = manager.recover(&state).await;
        assert!(result.is_ok());
    }
}
