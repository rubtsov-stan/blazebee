use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// MD RAID array statistics.
/// Contains information about software RAID arrays managed by the Linux kernel.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdRaidStats<'a> {
    /// RAID device name (e.g., "md0", "md127")
    pub device: Cow<'a, str>,
    /// RAID level (e.g., "raid1", "raid5", "raid6")
    pub level: Cow<'a, str>,
    /// Array state (e.g., "active", "degraded", "recovering")
    pub state: Cow<'a, str>,
    /// Number of active devices
    pub active_devices: u32,
    /// Total number of devices
    pub total_devices: u32,
    /// Number of failed devices
    pub failed_devices: u32,
    /// Number of spare devices
    pub spare_devices: u32,
    /// Sync progress percentage (0-100, None if not syncing)
    pub sync_progress: Option<f64>,
}

/// Buffer for storing MD RAID statistics from multiple RAID arrays.
/// Uses Vec internally for efficient storage and iteration.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MdRaidBuffer<'a> {
    stats: Vec<MdRaidStats<'a>>,
}

/// Methods for working with the MD RAID buffer
impl<'a> MdRaidBuffer<'a> {
    /// Creates a new empty MD RAID buffer
    pub fn new() -> Self {
        MdRaidBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of RAID arrays.
    /// Typical capacity would be the number of RAID arrays available on the system.
    pub fn with_capacity(capacity: usize) -> Self {
        MdRaidBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds MD RAID statistics entry to the buffer
    pub fn push(&mut self, stats: MdRaidStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the MD RAID statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &MdRaidStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of RAID entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// The MD RAID collector that reads from /proc/mdstat on Linux systems.
/// Collects information about software RAID arrays including status, health, and synchronization progress.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
pub struct MdRaidCollector;

/// Constructor methods for MdRaidCollector
impl MdRaidCollector {
    /// Creates a new MD RAID collector instance
    pub fn new() -> Self {
        MdRaidCollector
    }
}

/// Default trait implementation for MdRaidCollector
impl Default for MdRaidCollector {
    fn default() -> Self {
        MdRaidCollector::new()
    }
}

/// Data producer implementation for MD RAID collector.
/// Reads from /proc/mdstat and parses information about all software RAID arrays.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for MdRaidCollector {
    type Output = MdRaidBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the entire /proc/mdstat file which contains MD RAID information
        let content = tokio::fs::read_to_string("/proc/mdstat")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/mdstat".to_string(),
                source,
            })?;

        // Initialize buffer with capacity for up to 8 RAID arrays
        let mut buffer = MdRaidBuffer::with_capacity(8);
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        // Process each line in /proc/mdstat
        while i < lines.len() {
            let line = lines[i];

            // Look for lines starting with "md" (RAID array entries)
            if line.starts_with("md") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    i += 1;
                    continue;
                }

                let device = parts[0];
                let state = parts[2];
                let level = parts[3];

                // Parse device counts from the next line which contains device information
                i += 1;
                if i >= lines.len() {
                    break;
                }

                let detail_line = lines[i];
                // Count active devices (U = Up, _ = Up but not in sync)
                let active_devices = detail_line.matches("[U").count() as u32
                    + detail_line.matches("_").count() as u32;
                let failed_devices = detail_line.matches("_").count() as u32;
                let total_devices = active_devices;
                let spare_devices = 0; // Would need more detailed parsing

                // Create MD RAID statistics entry with the parsed values
                buffer.push(MdRaidStats {
                    device: Cow::Owned(device.trim_end_matches(':').to_string()),
                    level: Cow::Owned(level.to_string()),
                    state: Cow::Owned(state.to_string()),
                    active_devices,
                    total_devices,
                    failed_devices,
                    spare_devices,
                    sync_progress: None, // TODO: Parse synchronization progress from sync lines
                });
            }

            i += 1;
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
register_collector!(MdRaidCollector, "mdraid");

/// Fallback MD RAID statistics for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdRaidStats<'a> {
    /// RAID device name (e.g., "md0", "md127")
    pub device: Cow<'a, str>,
    /// RAID level (e.g., "raid1", "raid5", "raid6")
    pub level: Cow<'a, str>,
    /// Array state (e.g., "active", "degraded", "recovering")
    pub state: Cow<'a, str>,
    /// Number of active devices
    pub active_devices: u32,
    /// Total number of devices
    pub total_devices: u32,
    /// Number of failed devices
    pub failed_devices: u32,
    /// Number of spare devices
    pub spare_devices: u32,
    /// Sync progress percentage (0-100, None if not syncing)
    pub sync_progress: Option<f64>,
}

/// Fallback buffer for storing MD RAID statistics
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MdRaidBuffer<'a> {
    stats: Vec<MdRaidStats<'a>>,
}

/// Fallback methods for MD RAID buffer
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
impl<'a> MdRaidBuffer<'a> {
    /// Creates a new empty MD RAID buffer
    pub fn new() -> Self {
        MdRaidBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory
    pub fn with_capacity(capacity: usize) -> Self {
        MdRaidBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds MD RAID statistics entry to the buffer
    pub fn push(&mut self, stats: MdRaidStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the MD RAID statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &MdRaidStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of RAID entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// Fallback MD RAID collector for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
pub struct MdRaidCollector;

/// Fallback constructor methods for MdRaidCollector
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
impl MdRaidCollector {
    /// Creates a new MD RAID collector instance
    pub fn new() -> Self {
        MdRaidCollector
    }
}

/// Fallback Default trait implementation for MdRaidCollector
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
impl Default for MdRaidCollector {
    fn default() -> Self {
        MdRaidCollector::new()
    }
}

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for MdRaidCollector {
    type Output = MdRaidBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "MD RAID collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // MD RAID tests
    #[test]
    fn test_mdraid_stats_creation() {
        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md0"),
            level: Cow::Borrowed("raid1"),
            state: Cow::Borrowed("active"),
            active_devices: 2,
            total_devices: 2,
            failed_devices: 0,
            spare_devices: 0,
            sync_progress: None,
        };

        assert_eq!(raid_stats.device, "md0");
        assert_eq!(raid_stats.level, "raid1");
        assert_eq!(raid_stats.active_devices, 2);
        assert_eq!(raid_stats.failed_devices, 0);
    }

    #[test]
    fn test_mdraid_degraded() {
        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md1"),
            level: Cow::Borrowed("raid5"),
            state: Cow::Borrowed("degraded"),
            active_devices: 2,
            total_devices: 3,
            failed_devices: 1,
            spare_devices: 0,
            sync_progress: Some(45.5),
        };

        assert_eq!(raid_stats.state, "degraded");
        assert_eq!(raid_stats.failed_devices, 1);
        assert_eq!(raid_stats.sync_progress, Some(45.5));
    }

    #[test]
    fn test_mdraid_buffer() {
        let mut buffer = MdRaidBuffer::with_capacity(4);

        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md0"),
            level: Cow::Borrowed("raid1"),
            state: Cow::Borrowed("active"),
            active_devices: 2,
            total_devices: 2,
            failed_devices: 0,
            spare_devices: 0,
            sync_progress: None,
        };

        buffer.push(raid_stats);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_mdraid_recovering_state() {
        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md2"),
            level: Cow::Borrowed("raid6"),
            state: Cow::Borrowed("recovering"),
            active_devices: 4,
            total_devices: 5,
            failed_devices: 0,
            spare_devices: 1,
            sync_progress: Some(75.0),
        };

        assert_eq!(raid_stats.state, "recovering");
        assert_eq!(raid_stats.spare_devices, 1);
        assert_eq!(raid_stats.sync_progress, Some(75.0));
    }

    #[test]
    fn test_mdraid_buffer_empty() {
        let buffer = MdRaidBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_mdraid_buffer_multiple_arrays() {
        let mut buffer = MdRaidBuffer::with_capacity(3);

        for i in 0..3 {
            let raid_stats = MdRaidStats {
                device: Cow::Owned(format!("md{}", i)),
                level: Cow::Borrowed("raid1"),
                state: Cow::Borrowed("active"),
                active_devices: 2,
                total_devices: 2,
                failed_devices: 0,
                spare_devices: 0,
                sync_progress: None,
            };
            buffer.push(raid_stats);
        }

        assert_eq!(buffer.len(), 3);
        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_mdraid_stats_serialize() {
        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md0"),
            level: Cow::Borrowed("raid1"),
            state: Cow::Borrowed("active"),
            active_devices: 2,
            total_devices: 2,
            failed_devices: 0,
            spare_devices: 0,
            sync_progress: Some(100.0),
        };

        let json = serde_json::to_string(&raid_stats).expect("serialization failed");
        assert!(json.contains("md0"));
        assert!(json.contains("raid1"));
        assert!(json.contains("active"));
        assert!(json.contains("100.0"));
    }

    #[test]
    fn test_mdraid_stats_deserialize() {
        let json = r#"{
            "device": "md127",
            "level": "raid5",
            "state": "degraded",
            "active_devices": 3,
            "total_devices": 4,
            "failed_devices": 1,
            "spare_devices": 0,
            "sync_progress": 50.5
        }"#;

        let raid_stats: MdRaidStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(raid_stats.device, "md127");
        assert_eq!(raid_stats.level, "raid5");
        assert_eq!(raid_stats.state, "degraded");
        assert_eq!(raid_stats.active_devices, 3);
        assert_eq!(raid_stats.failed_devices, 1);
        assert_eq!(raid_stats.sync_progress, Some(50.5));
    }

    #[test]
    fn test_mdraid_collector_creation() {
        let _ = MdRaidCollector::new();
        let _ = MdRaidCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_mdraid_buffer_clone() {
        let mut buffer1 = MdRaidBuffer::new();

        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md0"),
            level: Cow::Borrowed("raid1"),
            state: Cow::Borrowed("active"),
            active_devices: 2,
            total_devices: 2,
            failed_devices: 0,
            spare_devices: 0,
            sync_progress: None,
        };

        buffer1.push(raid_stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
        assert_eq!(buffer1.len(), 1);
    }

    #[test]
    fn test_mdraid_completed_sync() {
        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md3"),
            level: Cow::Borrowed("raid10"),
            state: Cow::Borrowed("clean"),
            active_devices: 4,
            total_devices: 4,
            failed_devices: 0,
            spare_devices: 0,
            sync_progress: Some(100.0),
        };

        // Sync completed successfully
        assert_eq!(raid_stats.sync_progress, Some(100.0));
        assert_eq!(raid_stats.state, "clean");
    }

    #[test]
    fn test_mdraid_all_devices_failed() {
        let raid_stats = MdRaidStats {
            device: Cow::Borrowed("md4"),
            level: Cow::Borrowed("raid1"),
            state: Cow::Borrowed("failed"),
            active_devices: 0,
            total_devices: 2,
            failed_devices: 2,
            spare_devices: 0,
            sync_progress: None,
        };

        // All devices failed - critical state
        assert_eq!(raid_stats.active_devices, 0);
        assert_eq!(raid_stats.failed_devices, 2);
        assert_eq!(raid_stats.state, "failed");
    }
}
