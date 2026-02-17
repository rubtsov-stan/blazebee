use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use dashmap::DashMap;
use tracing::{debug, info, trace};

use super::{config::QueueConfig, error::Result, storage::SegmentMetadata};

#[derive(Clone)]
pub struct PartitionState {
    pub partition_id: u32,
    pub committed_offset: Arc<AtomicU64>,
    pub next_segment_id: Arc<AtomicU64>,
    pub segments: Arc<DashMap<u64, SegmentMetadata>>,
    pub consumer_offsets: Arc<DashMap<String, u64>>,
}

impl PartitionState {
    pub fn new(id: u32) -> Self {
        Self {
            partition_id: id,
            committed_offset: Arc::new(AtomicU64::new(0)),
            next_segment_id: Arc::new(AtomicU64::new(0)),
            segments: Arc::new(DashMap::new()),
            consumer_offsets: Arc::new(DashMap::new()),
        }
    }
    pub fn min_consumer_offset(&self) -> Option<u64> {
        if self.consumer_offsets.is_empty() {
            None
        } else {
            self.consumer_offsets.iter().map(|e| *e.value()).min()
        }
    }

    pub fn sorted_segments(&self) -> Vec<SegmentMetadata> {
        let mut v: Vec<_> = self.segments.iter().map(|e| e.value().clone()).collect();
        v.sort_by_key(|s| s.segment_id);
        v
    }
}

pub struct QueueState {
    write_offset: Arc<AtomicU64>,
    committed_offset: Arc<AtomicU64>,
    consumer_groups: Arc<DashMap<String, DashMap<usize, u64>>>,
    current_segment_ids: Arc<DashMap<usize, AtomicU64>>,
    config: QueueConfig,
    partitions: DashMap<u32, Arc<PartitionState>>,
    is_closed: Arc<AtomicU64>,
}

impl QueueState {
    pub fn new(config: QueueConfig) -> Self {
        let partitions = DashMap::new();
        for pid in 0..config.partition_count {
            partitions.insert(pid as u32, Arc::new(PartitionState::new(pid as u32)));
        }
        Self {
            write_offset: Arc::new(AtomicU64::new(0)),
            committed_offset: Arc::new(AtomicU64::new(0)),
            consumer_groups: Arc::new(DashMap::new()),
            current_segment_ids: Arc::new(DashMap::new()),
            config,
            partitions,
            is_closed: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn partitions(&self) -> Vec<Arc<PartitionState>> {
        self.partitions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
    pub fn persist_consumer_offsets(&self) -> Result<()> {
        let offsets_path = self.config.data_dir.join("consumer_offsets.json");
        let mut offsets_map = std::collections::HashMap::new();

        for group_entry in self.consumer_groups.iter() {
            let group_id = group_entry.key().clone();
            let group_offsets = group_entry.value();

            let mut partition_offsets = std::collections::HashMap::new();
            for offset_entry in group_offsets.iter() {
                partition_offsets.insert(*offset_entry.key(), *offset_entry.value());
            }
            offsets_map.insert(group_id, partition_offsets);
        }

        let json = serde_json::to_string_pretty(&offsets_map)?;
        std::fs::write(&offsets_path, json)?;
        debug!("Persisted consumer offsets to {:?}", offsets_path);
        Ok(())
    }
    pub fn load_consumer_offsets(&self) -> Result<()> {
        let offsets_path = self.config.data_dir.join("consumer_offsets.json");
        if !offsets_path.exists() {
            trace!("No consumer offsets file found at {:?}", offsets_path);
            return Ok(());
        }

        let json = std::fs::read_to_string(&offsets_path)?;
        trace!("Loaded consumer offsets from {:?}: {}", offsets_path, json);

        let offsets_map: std::collections::HashMap<String, std::collections::HashMap<usize, u64>> =
            serde_json::from_str(&json)?;

        for (group_id, partition_offsets) in offsets_map {
            trace!(
                "Restoring offsets for group {}: {:?}",
                group_id,
                partition_offsets
            );

            // Get or create the group - but don't clone it yet
            let group_ref = self
                .consumer_groups
                .entry(group_id.clone())
                .or_insert_with(|| {
                    println!("Creating new empty group map for '{}'", group_id);
                    DashMap::new()
                });

            // Now insert into the actual map, not a clone
            for (partition_id, offset) in partition_offsets {
                // Insert directly into group_ref
                group_ref.insert(partition_id, offset);
                trace!(
                    "Inserted into consumer_groups[{}][{}] = {}",
                    group_id,
                    partition_id,
                    offset
                );

                // Also update PartitionState
                if let Some(partition) = self.partitions.get(&(partition_id as u32)) {
                    partition.consumer_offsets.insert(group_id.clone(), offset);
                    trace!(
                        "Updated partition {} offset for group {} to {}",
                        partition_id,
                        group_id,
                        offset
                    );
                }
            }

            // Verify the group has the right entries by looking at group_ref
            debug!(
                "Verified group '{}' has {} entries",
                group_id,
                group_ref.len()
            );
            for entry in group_ref.iter() {
                debug!("Partition {}: {}", entry.key(), entry.value());
            }
        }

        Ok(())
    }

    pub fn persist_write_offset(&self) -> Result<()> {
        let write_offset_path = self.config.data_dir.join("write_offset");
        std::fs::write(&write_offset_path, self.get_write_offset().to_string())?;
        Ok(())
    }

    pub fn load_write_offset(&self) -> Result<u64> {
        let write_offset_path = self.config.data_dir.join("write_offset");
        if write_offset_path.exists() {
            let content = std::fs::read_to_string(write_offset_path)?;
            if let Ok(offset) = content.trim().parse::<u64>() {
                self.set_write_offset(offset);
                return Ok(offset);
            }
        }
        Ok(0)
    }
    pub fn partition(&self, id: u32) -> Arc<PartitionState> {
        self.partitions.get(&id).unwrap().clone()
    }
    pub fn get_partition_committed(&self, partition: u32) -> u64 {
        self.partitions
            .get(&partition)
            .map(|p| p.committed_offset.load(Ordering::Acquire))
            .unwrap_or(0)
    }

    pub fn set_partition_committed(&self, partition: u32, offset: u64) {
        if let Some(p) = self.partitions.get(&partition) {
            p.committed_offset.store(offset, Ordering::Release);
        }
    }

    pub fn get_partition_min_consumer_offset(&self, partition: u32) -> Option<u64> {
        self.partitions
            .get(&partition)
            .and_then(|p| p.min_consumer_offset())
    }

    pub fn next_write_offset(&self) -> u64 {
        self.write_offset.fetch_add(1, Ordering::SeqCst)
    }

    pub fn set_write_offset(&self, offset: u64) {
        self.write_offset.store(offset, Ordering::Release);
    }

    pub fn get_write_offset(&self) -> u64 {
        self.write_offset.load(Ordering::Acquire)
    }

    pub fn get_committed_offset(&self) -> u64 {
        self.committed_offset.load(Ordering::Acquire)
    }

    pub fn update_committed_offset(&self, offset: u64) {
        let current = self.committed_offset.load(Ordering::Acquire);
        if offset > current {
            self.committed_offset.store(offset, Ordering::Release);
        }
    }

    pub fn register_segment(&self, metadata: SegmentMetadata) {
        let partition_id = metadata.partition_id as u32;
        if let Some(partition) = self.partitions.get(&partition_id) {
            let end_offset = metadata.end_offset;
            partition.segments.insert(metadata.segment_id, metadata);

            // Update the partition's committed offset if needed
            let current_committed = partition.committed_offset.load(Ordering::Acquire);
            if end_offset > current_committed {
                partition
                    .committed_offset
                    .store(end_offset, Ordering::Release);
            }
        }
    }

    pub fn unregister_segment(&self, partition_id: u32, segment_id: u64) {
        if let Some(partition) = self.partitions.get(&partition_id) {
            partition.segments.remove(&segment_id);
        }
    }

    pub fn get_all_segments(&self) -> Vec<SegmentMetadata> {
        self.partitions
            .iter()
            .flat_map(|partition| {
                partition
                    .segments
                    .iter()
                    .map(|entry| entry.value().clone())
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn get_segments_for_partition(&self, partition: u32) -> Vec<SegmentMetadata> {
        if let Some(partition_state) = self.partitions.get(&partition) {
            let mut segments: Vec<_> = partition_state
                .segments
                .iter()
                .map(|entry| entry.value().clone())
                .collect();

            segments.sort_by_key(|s| s.segment_id);
            segments
        } else {
            Vec::new()
        }
    }

    pub fn get_segment_for_offset(&self, partition: u32, offset: u64) -> Option<SegmentMetadata> {
        let segments = self.get_segments_for_partition(partition);
        segments
            .iter()
            .find(|s| offset >= s.start_offset && offset <= s.end_offset)
            .cloned()
    }

    pub fn next_segment_id(&self, partition_id: usize) -> u64 {
        self.current_segment_ids
            .entry(partition_id)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::AcqRel)
    }
    pub fn create_consumer_group(&self, group_id: String) -> Arc<DashMap<usize, u64>> {
        debug!("create_consumer_group called with group='{}'", group_id);

        let group_ref = self
            .consumer_groups
            .entry(group_id.clone())
            .or_insert_with(|| {
                debug!("Creating new group map for '{}'", group_id);
                DashMap::new()
            });

        for partition in self.partitions.iter() {
            let pid = *partition.key() as usize;
            group_ref.insert(pid, 0);
            partition.consumer_offsets.insert(group_id.clone(), 0);
            info!("Initialized partition {} for group {}", pid, group_id);
        }

        // Return a clone of the group for API compatibility
        group_ref.clone().into()
    }

    pub fn update_consumer_offset(&self, group_id: &str, partition_id: usize, offset: u64) {
        let group = self
            .consumer_groups
            .entry(group_id.to_string())
            .or_insert_with(DashMap::new);

        let current = group.get(&partition_id).map(|o| *o).unwrap_or(0);
        if offset > current {
            group.insert(partition_id, offset);
        }

        if let Some(partition) = self.partitions.get(&(partition_id as u32)) {
            partition
                .consumer_offsets
                .insert(group_id.to_string(), offset);

            if let Some(min_offset) = partition.min_consumer_offset() {
                partition
                    .committed_offset
                    .store(min_offset, Ordering::Release);
            }
        }
    }

    pub fn get_consumer_offset(&self, group_id: &str, partition_id: usize) -> u64 {
        debug!(
            "get_consumer_offset: group='{}', partition={}",
            group_id, partition_id
        );

        // Debug: print all consumer groups
        debug!("Current consumer_groups keys:");
        for key in self.consumer_groups.iter() {
            debug!("  - Group: '{}'", key.key());
            for entry in key.value().iter() {
                debug!("    Partition {}: {}", entry.key(), entry.value());
            }
        }

        let result = self
            .consumer_groups
            .get(group_id)
            .and_then(|g| g.get(&partition_id).map(|o| *o))
            .unwrap_or(0);

        debug!("get_consumer_offset result = {}", result);
        result
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Acquire) != 0
    }

    pub fn mark_closed(&self) {
        self.is_closed.store(1, Ordering::Release);
    }

    pub fn config(&self) -> &QueueConfig {
        &self.config
    }

    pub fn set_next_segment_id(&self, partition_id: usize, id: u64) {
        self.current_segment_ids
            .entry(partition_id)
            .or_insert_with(|| AtomicU64::new(0))
            .store(id, Ordering::SeqCst);

        debug!(
            "Set next_segment_id for partition {} to {}",
            partition_id, id
        );
    }

    pub fn get_total_size(&self) -> u64 {
        self.partitions
            .iter()
            .flat_map(|partition| {
                partition
                    .segments
                    .iter()
                    .map(|entry| entry.value().size_bytes)
                    .collect::<Vec<_>>()
            })
            .sum()
    }
}

impl Clone for QueueState {
    fn clone(&self) -> Self {
        Self {
            write_offset: self.write_offset.clone(),
            committed_offset: self.committed_offset.clone(),
            consumer_groups: self.consumer_groups.clone(),
            current_segment_ids: self.current_segment_ids.clone(),
            config: self.config.clone(),
            is_closed: self.is_closed.clone(),
            partitions: self.partitions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn create_test_config() -> QueueConfig {
        let temp_dir = TempDir::new().unwrap();
        QueueConfig::new(temp_dir.path().to_path_buf())
    }

    // ==================== PartitionState Tests ====================

    #[test]
    fn test_partition_state_min_consumer_offset() {
        let partition = PartitionState::new(0);

        assert_eq!(partition.min_consumer_offset(), None);

        partition
            .consumer_offsets
            .insert("consumer1".to_string(), 100);
        partition
            .consumer_offsets
            .insert("consumer2".to_string(), 50);
        partition
            .consumer_offsets
            .insert("consumer3".to_string(), 75);

        assert_eq!(partition.min_consumer_offset(), Some(50));
    }

    #[test]
    fn test_partition_state_sorted_segments() {
        let partition = PartitionState::new(0);

        partition
            .segments
            .insert(5, create_segment_metadata(5, 100, 199));
        partition
            .segments
            .insert(2, create_segment_metadata(2, 50, 99));
        partition
            .segments
            .insert(8, create_segment_metadata(8, 200, 299));

        let sorted = partition.sorted_segments();

        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].segment_id, 2);
        assert_eq!(sorted[1].segment_id, 5);
        assert_eq!(sorted[2].segment_id, 8);
    }

    // ==================== QueueState Tests ====================

    #[test]
    fn test_queue_state_new() {
        let config = create_test_config();
        let state = QueueState::new(config.clone());

        assert_eq!(state.get_write_offset(), 0);
        assert_eq!(state.get_committed_offset(), 0);
        assert!(!state.is_closed());
        assert_eq!(state.config().partition_count, config.partition_count);
    }

    #[test]
    fn test_queue_state_committed_offset() {
        let config = create_test_config();
        let state = QueueState::new(config);

        assert_eq!(state.get_committed_offset(), 0);

        state.update_committed_offset(100);
        assert_eq!(state.get_committed_offset(), 100);

        state.update_committed_offset(50); // Should not decrease
        assert_eq!(state.get_committed_offset(), 100);

        state.update_committed_offset(200);
        assert_eq!(state.get_committed_offset(), 200);
    }

    #[test]
    fn test_queue_state_partition_operations() {
        let config = create_test_config();
        let state = QueueState::new(config);

        let partition = state.partition(0);
        assert_eq!(partition.partition_id, 0);

        state.set_partition_committed(0, 100);
        assert_eq!(state.get_partition_committed(0), 100);

        state.set_partition_committed(1, 200);
        assert_eq!(state.get_partition_committed(1), 200);
    }

    #[test]
    fn test_queue_state_segment_registration() {
        let config = create_test_config();
        let state = QueueState::new(config);

        let metadata = create_segment_metadata(1, 0, 99);
        state.register_segment(metadata.clone());

        let segments = state.get_segments_for_partition(0);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_id, 1);

        let all_segments = state.get_all_segments();
        assert_eq!(all_segments.len(), 1);
    }

    #[test]
    fn test_queue_state_segment_unregistration() {
        let config = create_test_config();
        let state = QueueState::new(config);

        state.register_segment(create_segment_metadata(1, 0, 99));
        state.register_segment(create_segment_metadata(2, 100, 199));

        assert_eq!(state.get_all_segments().len(), 2);

        state.unregister_segment(0, 1);
        assert_eq!(state.get_all_segments().len(), 1);

        let segments = state.get_segments_for_partition(0);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_id, 2);
    }

    #[test]
    fn test_queue_state_get_segment_for_offset() {
        let config = create_test_config();
        let state = QueueState::new(config);

        // Create test segments with proper partition_id
        let mut seg1 = create_segment_metadata(1, 0, 99);
        seg1.partition_id = 0; // Ensure partition_id is set
        state.register_segment(seg1);

        let mut seg2 = create_segment_metadata(2, 100, 199);
        seg2.partition_id = 0;
        state.register_segment(seg2);

        let mut seg3 = create_segment_metadata(3, 200, 299);
        seg3.partition_id = 0;
        state.register_segment(seg3);

        let seg = state.get_segment_for_offset(0, 50);
        assert!(seg.is_some());
        assert_eq!(seg.unwrap().segment_id, 1);

        let seg = state.get_segment_for_offset(0, 150);
        assert!(seg.is_some());
        assert_eq!(seg.unwrap().segment_id, 2);

        let seg = state.get_segment_for_offset(0, 500);
        assert!(seg.is_none());
    }

    #[test]
    fn test_queue_state_next_segment_id() {
        let config = create_test_config();
        let state = QueueState::new(config);

        assert_eq!(state.next_segment_id(0), 0);
        assert_eq!(state.next_segment_id(0), 1);
        assert_eq!(state.next_segment_id(0), 2);

        assert_eq!(state.next_segment_id(1), 0); // Different partition

        state.set_next_segment_id(0, 100);
        assert_eq!(state.next_segment_id(0), 100);
        assert_eq!(state.next_segment_id(0), 101);
    }

    #[test]
    fn test_queue_state_consumer_groups() {
        let config = create_test_config();
        let state = QueueState::new(config);

        state.create_consumer_group("group1".to_string());
        state.create_consumer_group("group2".to_string());

        state.update_consumer_offset("group1", 0, 100);
        state.update_consumer_offset("group1", 1, 200);
        state.update_consumer_offset("group2", 0, 50);

        assert_eq!(state.get_consumer_offset("group1", 0), 100);
        assert_eq!(state.get_consumer_offset("group1", 1), 200);
        assert_eq!(state.get_consumer_offset("group2", 0), 50);
        assert_eq!(state.get_consumer_offset("group1", 2), 0); // Not set
        assert_eq!(state.get_consumer_offset("group3", 0), 0); // Not exists
    }

    #[test]
    fn test_queue_state_consumer_offset_monotonic() {
        let config = create_test_config();
        let state = QueueState::new(config);

        state.create_consumer_group("group1".to_string());

        state.update_consumer_offset("group1", 0, 100);
        assert_eq!(state.get_consumer_offset("group1", 0), 100);

        state.update_consumer_offset("group1", 0, 50); // Should not decrease
        assert_eq!(state.get_consumer_offset("group1", 0), 100);

        state.update_consumer_offset("group1", 0, 150);
        assert_eq!(state.get_consumer_offset("group1", 0), 150);
    }

    #[test]
    fn test_queue_state_close() {
        let config = create_test_config();
        let state = QueueState::new(config);

        assert!(!state.is_closed());

        state.mark_closed();
        assert!(state.is_closed());
    }

    #[test]
    fn test_queue_state_total_size() {
        let config = create_test_config();
        let state = QueueState::new(config);

        assert_eq!(state.get_total_size(), 0);

        let mut meta1 = create_segment_metadata(1, 0, 99);
        meta1.size_bytes = 1000;
        state.register_segment(meta1);

        let mut meta2 = create_segment_metadata(2, 100, 199);
        meta2.size_bytes = 2000;
        state.register_segment(meta2);

        assert_eq!(state.get_total_size(), 3000);
    }

    #[test]
    fn test_queue_state_partition_min_consumer_offset() {
        let config = create_test_config();
        let state = QueueState::new(config);

        assert_eq!(state.get_partition_min_consumer_offset(0), None);

        state.update_consumer_offset("group1", 0, 100);
        state.update_consumer_offset("group2", 0, 50);
        state.update_consumer_offset("group3", 0, 75);

        assert_eq!(state.get_partition_min_consumer_offset(0), Some(50));
    }

    #[test]
    fn test_queue_state_clone() {
        let config = create_test_config();
        let state1 = QueueState::new(config);

        state1.next_write_offset();
        state1.next_write_offset();
        state1.update_committed_offset(100);

        let state2 = state1.clone();

        assert_eq!(state1.get_write_offset(), state2.get_write_offset());
        assert_eq!(state1.get_committed_offset(), state2.get_committed_offset());

        state1.next_write_offset();
        assert_eq!(state1.get_write_offset(), state2.get_write_offset()); // Shared state
    }

    // Helper function
    fn create_segment_metadata(
        segment_id: u64,
        start_offset: u64,
        end_offset: u64,
    ) -> SegmentMetadata {
        SegmentMetadata {
            segment_id,
            file_path: std::path::PathBuf::from("/tmp/test.log"),
            partition_id: 0,
            start_offset,
            end_offset,
            size_bytes: 0,
            message_count: 0,
            created_at: 0,
            is_sealed: false,
            last_verified_offset: 0,
            persisted_high_watermark: 0,
            last_fsync_time_ms: 0,
        }
    }
}
