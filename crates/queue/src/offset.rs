use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetIndex {
    entries: BTreeMap<u64, u64>,
}

impl OffsetIndex {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, message_offset: u64, byte_position: u64) {
        self.entries.insert(message_offset, byte_position);
    }

    pub fn get(&self, message_offset: u64) -> Option<u64> {
        self.entries.get(&message_offset).copied()
    }

    pub fn find_le(&self, target: u64) -> Option<(u64, u64)> {
        self.entries
            .range(..=target)
            .last()
            .map(|(&offset, &pos)| (offset, pos))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn max_offset(&self) -> Option<u64> {
        self.entries.keys().last().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u64, &u64)> {
        self.entries.iter()
    }

    pub fn truncate_after(&mut self, offset: u64) {
        self.entries.retain(|&k, _| k <= offset);
    }
}

impl Default for OffsetIndex {
    fn default() -> Self {
        Self::new()
    }
}
