use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use bytes::{Bytes, BytesMut};
use crc32fast::Hasher;
use memmap2::MmapMut;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::{
    error::{QueueError, Result},
    offset::OffsetIndex,
    utils::current_time_ms,
};

const MAGIC: u32 = 0xDEADBEEF;
const MESSAGE_HEADER_SIZE: usize = 4 + 4 + 8 + 4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MessageHeader {
    magic: u32,
    size: u32,
    message_offset: u64,
    flags: u32,
}

impl MessageHeader {
    fn to_bytes(&self) -> [u8; MESSAGE_HEADER_SIZE] {
        let mut buf = [0u8; MESSAGE_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..8].copy_from_slice(&self.size.to_le_bytes());
        buf[8..16].copy_from_slice(&self.message_offset.to_le_bytes());
        buf[16..20].copy_from_slice(&self.flags.to_le_bytes());
        buf
    }

    fn from_bytes(buf: &[u8; MESSAGE_HEADER_SIZE]) -> Option<Self> {
        if buf.len() < MESSAGE_HEADER_SIZE {
            return None;
        }
        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let size = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let message_offset = u64::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
        ]);
        let flags = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);

        if magic != MAGIC {
            return None;
        }

        Some(Self {
            magic,
            size,
            message_offset,
            flags,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMetadata {
    pub segment_id: u64,
    pub file_path: PathBuf,
    pub partition_id: usize,
    pub start_offset: u64,
    pub end_offset: u64,
    pub size_bytes: u64,
    pub message_count: u64,
    pub created_at: u64,
    pub is_sealed: bool,
    pub last_verified_offset: u64,
    pub persisted_high_watermark: u64,
    pub last_fsync_time_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SegmentDiskMetadata {
    pub segment_id: u64,
    pub partition_id: u32,
    pub start_offset: u64,
    pub persisted_high_watermark: u64,
    pub is_sealed: bool,
    pub created_at_ms: u64,
}

pub struct Segment {
    file: File,
    metadata: SegmentMetadata,
    mmap: Option<MmapMut>,
    offset_index: OffsetIndex,
    current_byte_pos: u64,
    enable_mmap: bool,
}

impl Segment {
    pub fn open(
        file_path: PathBuf,
        segment_id: u64,
        partition_id: usize,
        _next_offset: u64, // Rename to indicate it's only for new segments
        enable_mmap: bool,
    ) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&file_path)?;

        let file_size = file.metadata()?.len();
        let is_new_file = file_size == 0;

        let metadata = if is_new_file {
            // New segment - use next_offset as start
            SegmentMetadata {
                segment_id,
                file_path: file_path.clone(),
                partition_id,
                start_offset: _next_offset,
                end_offset: _next_offset.saturating_sub(1),
                size_bytes: 0,
                message_count: 0,
                created_at: current_time_ms(),
                is_sealed: false,
                last_verified_offset: _next_offset.saturating_sub(1),
                persisted_high_watermark: 0,
                last_fsync_time_ms: 0,
            }
        } else {
            // Existing segment - try to load metadata, otherwise will be set during rebuild
            let (start_offset, persisted_hw) =
                if let Some(disk_meta) = Self::load_metadata(&file_path)? {
                    (disk_meta.start_offset, disk_meta.persisted_high_watermark)
                } else {
                    (0, 0) // Will be set during rebuild
                };

            SegmentMetadata {
                segment_id,
                file_path: file_path.clone(),
                partition_id,
                start_offset,
                end_offset: start_offset.saturating_sub(1),
                size_bytes: file_size,
                message_count: 0,
                created_at: current_time_ms(),
                is_sealed: false,
                last_verified_offset: start_offset.saturating_sub(1),
                persisted_high_watermark: persisted_hw,
                last_fsync_time_ms: 0,
            }
        };

        let mmap = if enable_mmap && file_size > 0 {
            Some(unsafe { MmapMut::map_mut(&file)? })
        } else {
            None
        };

        let mut segment = Self {
            file,
            metadata,
            mmap,
            offset_index: OffsetIndex::new(),
            current_byte_pos: file_size,
            enable_mmap,
        };

        if file_size > 0 {
            segment.rebuild_index()?;
        }

        Ok(segment)
    }

    pub fn persist_metadata(&self) -> Result<()> {
        let disk_meta = SegmentDiskMetadata {
            segment_id: self.metadata.segment_id,
            partition_id: self.metadata.partition_id as u32,
            start_offset: self.metadata.start_offset,
            persisted_high_watermark: self.metadata.persisted_high_watermark,
            is_sealed: self.metadata.is_sealed,
            created_at_ms: self.metadata.created_at,
        };

        let meta_path = self.metadata.file_path.with_extension("meta");
        let json = serde_json::to_string(&disk_meta)?;

        let mut meta_file = std::fs::File::create(&meta_path)?;
        meta_file.write_all(json.as_bytes())?;
        meta_file.sync_all()?;
        Ok(())
    }

    pub fn load_metadata(file_path: &Path) -> Result<Option<SegmentDiskMetadata>> {
        let meta_path = file_path.with_extension("meta");
        if !meta_path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&meta_path)?;
        let meta = serde_json::from_str(&json)?;
        Ok(Some(meta))
    }

    pub fn rebuild_index(&mut self) -> std::io::Result<()> {
        self.offset_index = OffsetIndex::new();
        self.current_byte_pos = 0;

        let file_size = self.file.metadata()?.len();
        let mut byte_pos = 0u64;
        let mut last_valid_offset = None;
        let mut recovered = 0;
        let mut first_offset = None;
        let mut min_offset = u64::MAX;
        let mut max_offset = 0u64;

        while byte_pos + MESSAGE_HEADER_SIZE as u64 <= file_size {
            self.file.seek(SeekFrom::Start(byte_pos))?;

            let mut header_buf = [0u8; MESSAGE_HEADER_SIZE];
            if self.file.read_exact(&mut header_buf).is_err() {
                break;
            }

            let header = match MessageHeader::from_bytes(&header_buf) {
                Some(h) => h,
                None => break,
            };

            let total = MESSAGE_HEADER_SIZE as u64 + header.size as u64 + 4;
            if byte_pos + total > file_size {
                break;
            }

            let mut payload = vec![0u8; header.size as usize];
            self.file.read_exact(&mut payload)?;

            let mut crc_buf = [0u8; 4];
            self.file.read_exact(&mut crc_buf)?;
            let stored_crc = u32::from_le_bytes(crc_buf);

            if crc32fast::hash(&payload) != stored_crc {
                break;
            }

            let msg_offset = header.message_offset;

            if first_offset.is_none() {
                first_offset = Some(msg_offset);
            }

            min_offset = min_offset.min(msg_offset);
            max_offset = max_offset.max(msg_offset);

            self.offset_index.insert(msg_offset, byte_pos);
            last_valid_offset = Some(msg_offset);
            recovered += 1;

            byte_pos += total;
        }

        if byte_pos < file_size {
            self.file.set_len(byte_pos)?;
            self.file.sync_all()?;
        }

        self.current_byte_pos = byte_pos;

        // Update metadata based on actual data
        if recovered > 0 {
            self.metadata.start_offset = min_offset;
            self.metadata.end_offset = max_offset;
            self.metadata.message_count = recovered;
        }

        self.metadata.size_bytes = byte_pos;

        self.refresh_mmap()?;

        Ok(())
    }

    pub fn append_batch(&mut self, messages: &[(u64, Bytes)]) -> Result<()> {
        let mut buffer = BytesMut::new();
        let batch_start_pos = self.current_byte_pos;

        for (msg_offset, data) in messages {
            let message_pos = batch_start_pos + buffer.len() as u64;

            let header = MessageHeader {
                magic: MAGIC,
                size: data.len() as u32,
                message_offset: *msg_offset,
                flags: 0,
            };

            let header_bytes = header.to_bytes();
            buffer.extend_from_slice(&header_bytes);
            buffer.extend_from_slice(data);

            let mut hasher = Hasher::new();
            hasher.update(data);
            let crc = hasher.finalize();
            buffer.extend_from_slice(&crc.to_le_bytes());

            self.offset_index.insert(*msg_offset, message_pos);
        }

        self.file.write_all(&buffer)?;
        self.current_byte_pos += buffer.len() as u64;
        self.metadata.size_bytes = self.current_byte_pos;
        self.metadata.message_count += messages.len() as u64;

        if let Some((last_offset, _)) = messages.last() {
            self.metadata.end_offset = *last_offset;
        }

        Ok(())
    }
    pub fn read_by_offset(&self, message_offset: u64) -> Result<Option<Bytes>> {
        let byte_pos = match self.offset_index.get(message_offset) {
            Some(pos) => pos,
            None => return Ok(None),
        };

        self.read_at_byte_pos(byte_pos)
    }

    fn read_at_byte_pos(&self, byte_pos: u64) -> Result<Option<Bytes>> {
        let mmap = self.mmap.as_ref().ok_or_else(|| {
            QueueError::InvalidState("mmap not available for reading".to_string())
        })?;

        if byte_pos as usize >= mmap.len() {
            return Ok(None);
        }

        if (byte_pos as usize + MESSAGE_HEADER_SIZE) > mmap.len() {
            return Ok(None);
        }

        let mut header_buf = [0u8; MESSAGE_HEADER_SIZE];
        header_buf
            .copy_from_slice(&mmap[byte_pos as usize..(byte_pos as usize + MESSAGE_HEADER_SIZE)]);

        if let Some(header) = MessageHeader::from_bytes(&header_buf) {
            let data_start = byte_pos as usize + MESSAGE_HEADER_SIZE;
            let data_end = data_start + header.size as usize;
            let crc_start = data_end;
            let crc_end = crc_start + 4;

            if crc_end > mmap.len() {
                return Err(crate::QueueError::SegmentCorrupted);
            }

            let data = &mmap[data_start..data_end];
            let stored_crc = u32::from_le_bytes([
                mmap[crc_start],
                mmap[crc_start + 1],
                mmap[crc_start + 2],
                mmap[crc_start + 3],
            ]);

            let mut hasher = Hasher::new();
            hasher.update(data);
            let computed_crc = hasher.finalize();

            if computed_crc != stored_crc {
                return Err(crate::QueueError::CrcMismatch);
            }

            return Ok(Some(Bytes::copy_from_slice(data)));
        }

        Ok(None)
    }

    pub fn get_offsets_in_range(&self, start: u64, end: u64) -> Vec<u64> {
        self.offset_index
            .iter()
            .filter(|(&offset, _)| offset >= start && offset <= end)
            .map(|(&offset, _)| offset)
            .collect()
    }

    pub fn flush_and_refresh_mmap(&mut self) -> Result<()> {
        self.file.sync_data()?;

        if !self.enable_mmap {
            return Ok(());
        }

        let new_size = self.file.metadata()?.len();

        if new_size > 0 {
            drop(self.mmap.take());
            self.mmap = Some(unsafe { MmapMut::map_mut(&self.file)? });
            debug!(
                "Remapped segment {} to new size {}",
                self.metadata.segment_id, new_size
            );
        } else {
            drop(self.mmap.take());
        }

        Ok(())
    }

    fn refresh_mmap(&mut self) -> std::io::Result<()> {
        if !self.enable_mmap {
            return Ok(());
        }

        let new_size = self.file.metadata()?.len();
        if new_size > 0 {
            drop(self.mmap.take());
            self.mmap = Some(unsafe { MmapMut::map_mut(&self.file)? });
        } else {
            drop(self.mmap.take());
        }

        Ok(())
    }

    pub fn fsync(&mut self) -> Result<()> {
        self.file.sync_all()?;
        Ok(())
    }

    pub fn seal(&mut self) -> Result<()> {
        self.metadata.is_sealed = true;
        self.fsync()?;
        Ok(())
    }

    pub fn metadata(&self) -> &SegmentMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut SegmentMetadata {
        &mut self.metadata
    }

    pub fn offset_index(&self) -> &OffsetIndex {
        &self.offset_index
    }
}

#[cfg(test)]
mod integration_tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_segment_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("lifecycle.log");

        // Create
        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        // Write
        segment
            .append_batch(&[(0u64, Bytes::from("msg1")), (1u64, Bytes::from("msg2"))])
            .unwrap();

        segment.flush_and_refresh_mmap().unwrap();

        // Persist metadata
        segment.persist_metadata().unwrap();

        let msg = segment.read_by_offset(0).unwrap();
        assert!(msg.is_some(), "Message at offset 0 should exist");
        assert_eq!(msg.unwrap(), Bytes::from("msg1"));

        let msg = segment.read_by_offset(1).unwrap();
        assert!(msg.is_some(), "Message at offset 1 should exist");
        assert_eq!(msg.unwrap(), Bytes::from("msg2"));

        // Seal
        segment.seal().unwrap();
        assert!(segment.metadata().is_sealed);

        // Fsync
        segment.fsync().unwrap();
    }

    #[test]
    fn test_segment_offset_index_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("index_consistency.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        // Append messages with gaps in offsets
        segment
            .append_batch(&[(0u64, Bytes::from("msg0"))])
            .unwrap();
        segment
            .append_batch(&[(5u64, Bytes::from("msg5"))])
            .unwrap();
        segment
            .append_batch(&[(10u64, Bytes::from("msg10"))])
            .unwrap();

        // Index should have 3 entries
        assert_eq!(segment.offset_index().len(), 3);

        // Verify offsets exist
        assert!(segment.offset_index().get(0).is_some());
        assert!(segment.offset_index().get(5).is_some());
        assert!(segment.offset_index().get(10).is_some());

        // Verify gaps don't exist
        assert!(segment.offset_index().get(1).is_none());
        assert!(segment.offset_index().get(6).is_none());
    }

    #[test]
    fn test_segment_file_size_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("size_tracking.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        let initial_size = segment.metadata().size_bytes;
        assert_eq!(initial_size, 0);

        segment
            .append_batch(&[(0u64, Bytes::from("test"))])
            .unwrap();

        let new_size = segment.metadata().size_bytes;
        assert!(new_size > initial_size);

        // Verify actual file size matches metadata
        let file_metadata = std::fs::metadata(&file_path).unwrap();
        assert_eq!(file_metadata.len(), new_size);
    }

    #[test]
    fn test_segment_batch_append_positions() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("batch_positions.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        // Append batch with multiple messages
        segment
            .append_batch(&[
                (0u64, Bytes::from("first")),
                (1u64, Bytes::from("second")),
                (2u64, Bytes::from("third")),
            ])
            .unwrap();

        // Refresh mmap for reading
        segment.flush_and_refresh_mmap().unwrap();

        // All messages should be readable
        assert_eq!(
            segment.read_by_offset(0).unwrap().unwrap(),
            Bytes::from("first")
        );
        assert_eq!(
            segment.read_by_offset(1).unwrap().unwrap(),
            Bytes::from("second")
        );
        assert_eq!(
            segment.read_by_offset(2).unwrap().unwrap(),
            Bytes::from("third")
        );

        // Index should have all 3 entries
        assert_eq!(segment.offset_index().len(), 3);
    }

    #[test]
    fn test_segment_rebuild_after_batch_append() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("rebuild_batch.log");

        // Create and append
        {
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
            segment
                .append_batch(&[
                    (0u64, Bytes::from("msg1")),
                    (1u64, Bytes::from("msg2")),
                    (2u64, Bytes::from("msg3")),
                ])
                .unwrap();
            segment.fsync().unwrap();
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
        segment.rebuild_index().unwrap();

        assert_eq!(segment.metadata().message_count, 3);
        assert_eq!(segment.offset_index().len(), 3);

        assert!(
            segment.offset_index().get(0).is_some(),
            "Offset 0 should exist in index"
        );
        assert!(
            segment.offset_index().get(1).is_some(),
            "Offset 1 should exist in index"
        );
        assert!(
            segment.offset_index().get(2).is_some(),
            "Offset 2 should exist in index"
        );

        let msg0 = segment.read_by_offset(0).unwrap();
        assert!(msg0.is_some(), "Message at offset 0 should be readable");
        assert_eq!(msg0.unwrap(), Bytes::from("msg1"));

        let msg1 = segment.read_by_offset(1).unwrap();
        assert!(msg1.is_some(), "Message at offset 1 should be readable");
        assert_eq!(msg1.unwrap(), Bytes::from("msg2"));

        let msg2 = segment.read_by_offset(2).unwrap();
        assert!(msg2.is_some(), "Message at offset 2 should be readable");
        assert_eq!(msg2.unwrap(), Bytes::from("msg3"));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    // ==================== Basic Segment Tests ====================

    #[test]
    fn test_segment_create_new() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        assert_eq!(segment.metadata().segment_id, 1);
        assert_eq!(segment.metadata().partition_id, 0);
        assert_eq!(segment.metadata().start_offset, 0);
        assert_eq!(segment.metadata().end_offset, 0);
        assert_eq!(segment.metadata().message_count, 0);
        assert!(!segment.metadata().is_sealed);
    }

    #[test]
    fn test_segment_create_with_mmap() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_mmap.log");

        let segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        assert!(segment.mmap.is_none());

        let mut segment = segment;
        let messages = vec![(0u64, Bytes::from("hello"))];
        segment.append_batch(&messages).unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        assert!(segment.mmap.is_some());
    }

    // ==================== Append Tests ====================

    #[test]
    fn test_segment_append_single_message() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("append.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        let messages = vec![(0u64, Bytes::from("test message"))];
        segment.append_batch(&messages).unwrap();

        assert_eq!(segment.metadata().message_count, 1);
        assert_eq!(segment.metadata().end_offset, 0);
        assert!(segment.metadata().size_bytes > 0);
    }

    #[test]
    fn test_segment_append_batch() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("batch.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        let messages = vec![
            (0u64, Bytes::from("msg1")),
            (1u64, Bytes::from("msg2")),
            (2u64, Bytes::from("msg3")),
        ];
        segment.append_batch(&messages).unwrap();

        assert_eq!(segment.metadata().message_count, 3);
        assert_eq!(segment.metadata().end_offset, 2);
        assert_eq!(segment.offset_index().len(), 3);
    }

    #[test]
    fn test_segment_append_multiple_batches() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("multi_batch.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        segment
            .append_batch(&[(0u64, Bytes::from("batch1"))])
            .unwrap();
        assert_eq!(segment.metadata().end_offset, 0);

        segment
            .append_batch(&[(1u64, Bytes::from("batch2"))])
            .unwrap();
        assert_eq!(segment.metadata().end_offset, 1);
        assert_eq!(segment.metadata().message_count, 2);
    }

    #[test]
    fn test_segment_append_with_custom_start_offset() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("custom_offset.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 100, false).unwrap();

        assert_eq!(segment.metadata().start_offset, 100);
        assert_eq!(segment.metadata().end_offset, 99);

        segment
            .append_batch(&[(100u64, Bytes::from("msg"))])
            .unwrap();

        assert_eq!(segment.metadata().end_offset, 100);
    }

    // ==================== Read Tests ====================

    #[test]
    fn test_segment_read_by_offset() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("read.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        let messages = vec![
            (0u64, Bytes::from("first")),
            (1u64, Bytes::from("second")),
            (2u64, Bytes::from("third")),
        ];
        segment.append_batch(&messages).unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        let msg = segment.read_by_offset(0).unwrap().unwrap();
        assert_eq!(msg, Bytes::from("first"));

        let msg = segment.read_by_offset(1).unwrap().unwrap();
        assert_eq!(msg, Bytes::from("second"));

        let msg = segment.read_by_offset(2).unwrap().unwrap();
        assert_eq!(msg, Bytes::from("third"));
    }

    #[test]
    fn test_segment_read_nonexistent_offset() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("read_missing.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        segment.append_batch(&[(0u64, Bytes::from("msg"))]).unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        let result = segment.read_by_offset(999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_segment_read_empty_segment() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.log");

        let segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        let result = segment.read_by_offset(0).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_segment_get_offsets_in_range() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("range.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        let messages: Vec<(u64, Bytes)> = (0..10)
            .map(|i| (i, Bytes::from(format!("msg{}", i))))
            .collect();
        segment.append_batch(&messages).unwrap();

        let offsets = segment.get_offsets_in_range(3, 7);
        assert_eq!(offsets, vec![3, 4, 5, 6, 7]);

        let offsets = segment.get_offsets_in_range(0, 100);
        assert_eq!(offsets.len(), 10);

        let offsets = segment.get_offsets_in_range(50, 60);
        assert!(offsets.is_empty());
    }

    // ==================== Index Rebuild Tests ====================

    #[test]
    fn test_segment_rebuild_index() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("rebuild.log");

        {
            // enable_mmap = true
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
            let messages = vec![
                (0u64, Bytes::from("msg1")),
                (1u64, Bytes::from("msg2")),
                (2u64, Bytes::from("msg3")),
            ];
            segment.append_batch(&messages).unwrap();
            segment.fsync().unwrap();
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
        segment.rebuild_index().unwrap();

        assert_eq!(segment.metadata().message_count, 3);
        assert_eq!(segment.metadata().end_offset, 2);
        assert_eq!(segment.offset_index().len(), 3);

        let msg = segment.read_by_offset(1).unwrap().unwrap();
        assert_eq!(msg, Bytes::from("msg2"));
    }

    #[test]
    fn test_segment_rebuild_index_with_corrupted_data() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("corrupted.log");

        {
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
            segment
                .append_batch(&[(0u64, Bytes::from("valid"))])
                .unwrap();
            segment.fsync().unwrap();
        }

        {
            let mut file = OpenOptions::new().append(true).open(&file_path).unwrap();
            file.write_all(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap();
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.rebuild_index().unwrap();

        assert_eq!(segment.metadata().message_count, 1);
        assert_eq!(segment.metadata().end_offset, 0);
    }

    // ==================== Metadata Tests ====================

    #[test]
    fn test_segment_persist_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("meta.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.append_batch(&[(0u64, Bytes::from("msg"))]).unwrap();
        segment.metadata_mut().persisted_high_watermark = 100;
        segment.persist_metadata().unwrap();

        let meta_path = file_path.with_extension("meta");
        assert!(meta_path.exists());

        let loaded = Segment::load_metadata(&file_path).unwrap().unwrap();
        assert_eq!(loaded.segment_id, 1);
        assert_eq!(loaded.partition_id, 0);
        assert_eq!(loaded.persisted_high_watermark, 100);
    }

    #[test]
    fn test_segment_load_metadata_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("no_meta.log");

        File::create(&file_path).unwrap();

        let result = Segment::load_metadata(&file_path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_segment_metadata_after_reopen() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("reopen.log");

        {
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
            segment
                .append_batch(&[(0u64, Bytes::from("msg1"))])
                .unwrap();
            segment
                .append_batch(&[(1u64, Bytes::from("msg2"))])
                .unwrap();
            segment.persist_metadata().unwrap();
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.rebuild_index().unwrap();

        assert_eq!(segment.metadata().message_count, 2);
        assert_eq!(segment.metadata().end_offset, 1);
    }

    // ==================== Seal and Fsync Tests ====================

    #[test]
    fn test_segment_seal() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("seal.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.append_batch(&[(0u64, Bytes::from("msg"))]).unwrap();

        assert!(!segment.metadata().is_sealed);

        segment.seal().unwrap();

        assert!(segment.metadata().is_sealed);
    }

    #[test]
    fn test_segment_fsync() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("fsync.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.append_batch(&[(0u64, Bytes::from("msg"))]).unwrap();

        segment.fsync().unwrap();

        let metadata = fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_segment_flush_and_refresh_mmap() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("flush_mmap.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
        segment.append_batch(&[(0u64, Bytes::from("msg"))]).unwrap();

        segment.flush_and_refresh_mmap().unwrap();

        assert!(segment.mmap.is_some());
        assert!(segment.metadata().size_bytes > 0);
    }

    // ==================== CRC Verification Tests ====================

    #[test]
    fn test_segment_crc_verification() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("crc.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
        segment
            .append_batch(&[(0u64, Bytes::from("test data"))])
            .unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        let result = segment.read_by_offset(0);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_segment_crc_mismatch_detection() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("crc_bad.log");

        {
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
            segment
                .append_batch(&[(0u64, Bytes::from("test"))])
                .unwrap();
            segment.fsync().unwrap();
        }

        {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&file_path)
                .unwrap();
            let len = file.metadata().unwrap().len();
            if len > 4 {
                file.seek(SeekFrom::End(-4)).unwrap();
                file.write_all(&[0x00, 0x00, 0x00, 0x00]).unwrap();
            }
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
        segment.rebuild_index().unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        let result = segment.read_by_offset(0);
        match result {
            Ok(_) => {}
            Err(crate::QueueError::CrcMismatch) => {}
            Err(_) => {}
        }
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_segment_empty_message() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_msg.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        segment.append_batch(&[(0u64, Bytes::new())]).unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        assert_eq!(segment.metadata().message_count, 1);

        let result = segment.read_by_offset(0).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_segment_large_message() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_msg.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        let large_data = vec![0x42u8; 1024 * 1024];
        segment
            .append_batch(&[(0u64, Bytes::from(large_data.clone()))])
            .unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        assert_eq!(segment.metadata().message_count, 1);

        let result = segment.read_by_offset(0).unwrap().unwrap();
        assert_eq!(result.len(), 1024 * 1024);
        assert_eq!(result[0], 0x42);
        assert_eq!(result[1024 * 1024 - 1], 0x42);
    }

    #[test]
    fn test_segment_many_messages() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("many.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        let messages: Vec<(u64, Bytes)> = (0..1000)
            .map(|i| (i, Bytes::from(format!("message{}", i))))
            .collect();

        segment.append_batch(&messages).unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        assert_eq!(segment.metadata().message_count, 1000);
        assert_eq!(segment.metadata().end_offset, 999);
        assert_eq!(segment.offset_index().len(), 1000);

        for offset in [0, 100, 500, 999] {
            let msg = segment.read_by_offset(offset).unwrap().unwrap();
            assert_eq!(msg, Bytes::from(format!("message{}", offset)));
        }
    }

    #[test]
    fn test_segment_concurrent_append_simulation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("concurrent.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        for batch in 0..10 {
            let messages: Vec<(u64, Bytes)> = (0..10)
                .map(|i| {
                    (
                        batch * 10 + i,
                        Bytes::from(format!("batch{}-msg{}", batch, i)),
                    )
                })
                .collect();
            segment.append_batch(&messages).unwrap();
        }

        segment.flush_and_refresh_mmap().unwrap();

        assert_eq!(segment.metadata().message_count, 100);
        assert_eq!(segment.metadata().end_offset, 99);

        for offset in 0..100 {
            let msg = segment.read_by_offset(offset).unwrap().unwrap();
            let batch = offset / 10;
            let msg_in_batch = offset % 10;
            assert_eq!(
                msg,
                Bytes::from(format!("batch{}-msg{}", batch, msg_in_batch))
            );
        }
    }

    // ==================== Mmap Specific Tests ====================

    #[test]
    fn test_segment_mmap_read_performance() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("mmap_perf.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        let messages: Vec<(u64, Bytes)> = (0..100)
            .map(|i| (i, Bytes::from(format!("perf_test_{}", i))))
            .collect();
        segment.append_batch(&messages).unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        for offset in 0..100 {
            let msg = segment.read_by_offset(offset).unwrap().unwrap();
            assert_eq!(msg, Bytes::from(format!("perf_test_{}", offset)));
        }
    }

    #[test]
    fn test_segment_mmap_resize() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("mmap_resize.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();

        segment
            .append_batch(&[(0u64, Bytes::from("initial"))])
            .unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        let initial_size = segment.metadata().size_bytes;

        segment
            .append_batch(&[(1u64, Bytes::from("more data"))])
            .unwrap();
        segment.flush_and_refresh_mmap().unwrap();

        let new_size = segment.metadata().size_bytes;
        assert!(new_size > initial_size);
        assert!(segment.mmap.is_some());
    }

    // ==================== Recovery Tests ====================

    #[test]
    fn test_segment_recovery_after_crash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("crash.log");

        {
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
            segment
                .append_batch(&[(0u64, Bytes::from("msg1"))])
                .unwrap();
            segment
                .append_batch(&[(1u64, Bytes::from("msg2"))])
                .unwrap();
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, true).unwrap();
        segment.rebuild_index().unwrap();

        assert_eq!(segment.metadata().message_count, 2);
        assert_eq!(segment.metadata().end_offset, 1);

        assert!(segment.read_by_offset(0).unwrap().is_some());
        assert!(segment.read_by_offset(1).unwrap().is_some());
    }

    #[test]
    fn test_segment_partial_write_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("partial.log");

        {
            let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
            segment
                .append_batch(&[(0u64, Bytes::from("complete"))])
                .unwrap();
            segment.fsync().unwrap();
        }

        {
            let mut file = OpenOptions::new().append(true).open(&file_path).unwrap();
            file.write_all(&[0xEF, 0xBE, 0xAD, 0xDE]).unwrap();
        }

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.rebuild_index().unwrap();

        assert_eq!(segment.metadata().message_count, 1);
        assert_eq!(segment.metadata().end_offset, 0);
    }

    // ==================== Metadata Field Tests ====================

    #[test]
    fn test_segment_metadata_fields() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("meta_fields.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();

        assert_eq!(segment.metadata().segment_id, 1);
        assert_eq!(segment.metadata().partition_id, 0);
        assert!(segment.metadata().created_at > 0);
        assert!(!segment.metadata().is_sealed);
        assert_eq!(segment.metadata().persisted_high_watermark, 0);

        segment.metadata_mut().persisted_high_watermark = 50;
        segment.metadata_mut().last_verified_offset = 25;

        assert_eq!(segment.metadata().persisted_high_watermark, 50);
        assert_eq!(segment.metadata().last_verified_offset, 25);
    }

    #[test]
    fn test_segment_disk_metadata_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("disk_meta.log");

        let mut segment = Segment::open(file_path.clone(), 1, 0, 0, false).unwrap();
        segment.metadata_mut().persisted_high_watermark = 100;
        segment.metadata_mut().is_sealed = true;
        segment.persist_metadata().unwrap();

        let loaded = Segment::load_metadata(&file_path).unwrap().unwrap();

        assert_eq!(loaded.segment_id, 1);
        assert_eq!(loaded.partition_id, 0);
        assert_eq!(loaded.persisted_high_watermark, 100);
        assert!(loaded.is_sealed);
        assert!(loaded.created_at_ms > 0);
    }
}
