use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Disk I/O statistics for a specific storage device.
/// Contains comprehensive information about read and write operations,
/// including counts, data volume, and timing information.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStats<'a> {
    /// Device name (e.g., "sda", "nvme0n1", "dm-0")
    pub device: Cow<'a, str>,
    /// Number of reads completed successfully
    pub reads_completed: u64,
    /// Number of read operations merged (adjacent requests combined)
    pub reads_merged: u64,
    /// Total number of sectors read
    pub read_bytes: u64,
    /// Total time spent reading (milliseconds)
    pub read_time_ms: u64,
    /// Number of writes completed successfully
    pub writes_completed: u64,
    /// Number of write operations merged (adjacent requests combined)
    pub writes_merged: u64,
    /// Total number of sectors written
    pub written_bytes: u64,
    /// Total time spent writing (milliseconds)
    pub write_time_ms: u64,
    /// Number of I/O operations currently in progress
    pub io_in_progress: u64,
    /// Total time device has had I/O in progress (milliseconds)
    pub io_time_ms: u64,
}

/// Buffer for storing disk statistics from multiple storage devices.
/// Uses Vec internally for efficient storage and iteration.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskBuffer<'a> {
    stats: Vec<DiskStats<'a>>,
}

/// Methods for working with the disk buffer
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
impl<'a> DiskBuffer<'a> {
    /// Creates a new empty disk buffer
    pub fn new() -> Self {
        DiskBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of devices.
    /// Typical capacity would be the number of storage devices on the system.
    pub fn with_capacity(capacity: usize) -> Self {
        DiskBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds a disk statistics entry to the buffer
    pub fn push(&mut self, stats: DiskStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the disk statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &DiskStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of disk entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Returns a slice of all disk statistics entries
    pub fn as_slice(&self) -> &[DiskStats<'a>] {
        &self.stats
    }
}

/// The Disk collector that reads from /proc/diskstats on Linux systems.
/// Collects detailed disk I/O statistics for individual storage devices.
/// Supports filtering to include only specific devices.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
pub struct DiskStatsCollector {
    /// Optional list of device names to include (if None, collects all devices)
    pub include_devices: Option<Vec<String>>,
}

/// Data producer implementation for Disk collector.
/// Reads from /proc/diskstats and parses disk I/O statistics for all or selected devices.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for DiskStatsCollector {
    type Output = DiskBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the /proc/diskstats file which contains disk I/O statistics
        let content = tokio::fs::read_to_string("/proc/diskstats")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/diskstats".to_string(),
                source,
            })?;

        // Initialize buffer with capacity for up to 32 devices
        let mut buffer = DiskBuffer::with_capacity(32);

        // Process each line in /proc/diskstats
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Each disk line must have at least 14 fields
            if parts.len() < 14 {
                continue;
            }

            // Extract device name from field index 2
            let device = parts[2];

            // If include_devices filter is set, skip devices not in the list
            if let Some(ref include) = self.include_devices {
                if !include.contains(&device.to_string()) {
                    continue;
                }
            }

            // Parse number of reads completed (field index 3)
            let reads_completed =
                parts[3]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "reads_completed".to_string(),
                        location: format!("/proc/diskstats device={}", device),
                        reason: format!("invalid value: {}", parts[3]),
                    })?;

            // Parse number of reads merged (field index 4)
            let reads_merged = parts[4]
                .parse::<u64>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "reads_merged".to_string(),
                    location: format!("/proc/diskstats device={}", device),
                    reason: format!("invalid value: {}", parts[4]),
                })?;

            // Parse sectors read (field index 5)
            let read_bytes = parts[5]
                .parse::<u64>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "read_bytes".to_string(),
                    location: format!("/proc/diskstats device={}", device),
                    reason: format!("invalid value: {}", parts[5]),
                })?;

            // Parse time spent reading in milliseconds (field index 6)
            let read_time_ms = parts[6]
                .parse::<u64>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "read_time_ms".to_string(),
                    location: format!("/proc/diskstats device={}", device),
                    reason: format!("invalid value: {}", parts[6]),
                })?;

            // Parse number of writes completed (field index 7)
            let writes_completed =
                parts[7]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "writes_completed".to_string(),
                        location: format!("/proc/diskstats device={}", device),
                        reason: format!("invalid value: {}", parts[7]),
                    })?;

            // Parse number of writes merged (field index 8)
            let writes_merged =
                parts[8]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "writes_merged".to_string(),
                        location: format!("/proc/diskstats device={}", device),
                        reason: format!("invalid value: {}", parts[8]),
                    })?;

            // Parse sectors written (field index 9)
            let written_bytes =
                parts[9]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "written_bytes".to_string(),
                        location: format!("/proc/diskstats device={}", device),
                        reason: format!("invalid value: {}", parts[9]),
                    })?;

            // Parse time spent writing in milliseconds (field index 10)
            let write_time_ms =
                parts[10]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "write_time_ms".to_string(),
                        location: format!("/proc/diskstats device={}", device),
                        reason: format!("invalid value: {}", parts[10]),
                    })?;

            // Parse number of I/O operations in progress (field index 11)
            let io_in_progress =
                parts[11]
                    .parse::<u64>()
                    .map_err(|_| CollectorError::ParseError {
                        metric: "io_in_progress".to_string(),
                        location: format!("/proc/diskstats device={}", device),
                        reason: format!("invalid value: {}", parts[11]),
                    })?;

            // Parse total time device had I/O in progress in milliseconds (field index 12)
            let io_time_ms = parts[12]
                .parse::<u64>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "io_time_ms".to_string(),
                    location: format!("/proc/diskstats device={}", device),
                    reason: format!("invalid value: {}", parts[12]),
                })?;

            // Create a DiskStats entry with the parsed values
            buffer.push(DiskStats {
                device: Cow::Owned(device.to_string()),
                reads_completed,
                reads_merged,
                read_bytes,
                read_time_ms,
                writes_completed,
                writes_merged,
                written_bytes,
                write_time_ms,
                io_in_progress,
                io_time_ms,
            });
        }

        Ok(buffer)
    }
}

/// Constructor methods for DiskStatsCollector
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
impl DiskStatsCollector {
    /// Creates a new DiskStatsCollector instance with optional device filtering
    pub fn new(include_devices: Option<Vec<String>>) -> Self {
        DiskStatsCollector { include_devices }
    }
}

/// Default trait implementation for DiskStatsCollector
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
impl Default for DiskStatsCollector {
    fn default() -> Self {
        DiskStatsCollector::new(None)
    }
}

#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
register_collector!(DiskStatsCollector, "disk");

/// Fallback implementations for unsupported platforms or disabled features

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStats<'a> {
    pub device: Cow<'a, str>,
    pub reads_completed: u64,
    pub reads_merged: u64,
    pub read_bytes: u64,
    pub read_time_ms: u64,
    pub writes_completed: u64,
    pub writes_merged: u64,
    pub written_bytes: u64,
    pub write_time_ms: u64,
    pub io_in_progress: u64,
    pub io_time_ms: u64,
}

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskBuffer<'a> {
    stats: Vec<DiskStats<'a>>,
}

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
impl<'a> DiskBuffer<'a> {
    pub fn new() -> Self {
        DiskBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        DiskBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stats: DiskStats<'a>) {
        self.stats.push(stats);
    }

    pub fn iter(&self) -> impl Iterator<Item = &DiskStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    pub fn as_slice(&self) -> &[DiskStats<'a>] {
        &self.stats
    }
}

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
pub struct DiskStatsCollector {
    pub include_devices: Option<Vec<String>>,
}

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
impl DiskStatsCollector {
    pub fn new(include_devices: Option<Vec<String>>) -> Self {
        DiskStatsCollector { include_devices }
    }
}

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
impl Default for DiskStatsCollector {
    fn default() -> Self {
        DiskStatsCollector::new(None)
    }
}

#[cfg(not(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for DiskStatsCollector {
    type Output = DiskBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Disk collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_stats_creation() {
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        assert_eq!(stats.device, "sda");
        assert_eq!(stats.reads_completed, 10000);
        assert_eq!(stats.writes_completed, 8000);
    }

    #[test]
    fn test_disk_stats_zero_io() {
        // Test with a device that has no I/O activity
        let stats = DiskStats {
            device: Cow::Borrowed("sdb"),
            reads_completed: 0,
            reads_merged: 0,
            read_bytes: 0,
            read_time_ms: 0,
            writes_completed: 0,
            writes_merged: 0,
            written_bytes: 0,
            write_time_ms: 0,
            io_in_progress: 0,
            io_time_ms: 0,
        };

        assert_eq!(stats.reads_completed, 0);
        assert_eq!(stats.io_in_progress, 0);
    }

    #[test]
    fn test_disk_stats_nvme_device() {
        // Test with NVMe device naming
        let stats = DiskStats {
            device: Cow::Borrowed("nvme0n1"),
            reads_completed: 50000,
            reads_merged: 200,
            read_bytes: 25000000,
            read_time_ms: 25000,
            writes_completed: 40000,
            writes_merged: 150,
            written_bytes: 20000000,
            write_time_ms: 20000,
            io_in_progress: 3,
            io_time_ms: 45000,
        };

        assert_eq!(stats.device, "nvme0n1");
        assert!(stats.reads_completed > stats.writes_completed);
    }

    #[test]
    fn test_disk_buffer_creation() {
        let buffer = DiskBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_disk_buffer_with_capacity() {
        let buffer = DiskBuffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_disk_buffer_push_single() {
        let mut buffer = DiskBuffer::new();
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        buffer.push(stats);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_disk_buffer_multiple_devices() {
        let mut buffer = DiskBuffer::with_capacity(4);

        let devices = vec!["sda", "sdb", "sdc", "nvme0n1"];
        for device in devices {
            let stats = DiskStats {
                device: Cow::Owned(device.to_string()),
                reads_completed: 10000,
                reads_merged: 100,
                read_bytes: 5000000,
                read_time_ms: 50000,
                writes_completed: 8000,
                writes_merged: 80,
                written_bytes: 4000000,
                write_time_ms: 40000,
                io_in_progress: 5,
                io_time_ms: 90000,
            };
            buffer.push(stats);
        }

        assert_eq!(buffer.len(), 4);
    }

    #[test]
    fn test_disk_buffer_iterator() {
        let mut buffer = DiskBuffer::new();

        for i in 0..3 {
            let stats = DiskStats {
                device: Cow::Owned(format!("sd{}", (b'a' + i as u8) as char)),
                reads_completed: 10000,
                reads_merged: 100,
                read_bytes: 5000000,
                read_time_ms: 50000,
                writes_completed: 8000,
                writes_merged: 80,
                written_bytes: 4000000,
                write_time_ms: 40000,
                io_in_progress: 5,
                io_time_ms: 90000,
            };
            buffer.push(stats);
        }

        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_disk_collector_creation() {
        let collector = DiskStatsCollector::new(None);
        assert!(collector.include_devices.is_none());
    }

    #[test]
    fn test_disk_collector_with_devices() {
        let devices = vec!["sda".to_string(), "nvme0n1".to_string()];
        let collector = DiskStatsCollector::new(Some(devices.clone()));

        assert!(collector.include_devices.is_some());
        assert_eq!(collector.include_devices.unwrap().len(), 2);
    }

    #[test]
    fn test_disk_collector_default() {
        let collector = DiskStatsCollector::default();
        assert!(collector.include_devices.is_none());
    }

    #[test]
    fn test_disk_stats_serialize() {
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        assert!(json.contains("sda"));
        assert!(json.contains("10000"));
    }

    #[test]
    fn test_disk_stats_deserialize() {
        let json = r#"{
            "device": "sda",
            "reads_completed": 15000,
            "reads_merged": 150,
            "read_bytes": 7500000,
            "read_time_ms": 75000,
            "writes_completed": 12000,
            "writes_merged": 120,
            "written_bytes": 6000000,
            "write_time_ms": 60000,
            "io_in_progress": 2,
            "io_time_ms": 135000
        }"#;

        let stats: DiskStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(stats.device, "sda");
        assert_eq!(stats.reads_completed, 15000);
        assert_eq!(stats.writes_completed, 12000);
    }

    #[test]
    fn test_disk_stats_total_io() {
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        let total_ops = stats.reads_completed + stats.writes_completed;
        assert_eq!(total_ops, 18000);

        let total_bytes = stats.read_bytes + stats.written_bytes;
        assert_eq!(total_bytes, 9000000);
    }

    #[test]
    fn test_disk_stats_io_utilization() {
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        // Calculate utilization percentage
        let total_time = 100000; // Arbitrary total time for comparison
        let utilization = (stats.io_time_ms as f64 / total_time as f64) * 100.0;
        assert!(utilization > 0.0);
    }

    #[test]
    fn test_disk_stats_merged_io_ratio() {
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        // Calculate read merge ratio
        if stats.reads_completed > 0 {
            let read_merge_ratio =
                (stats.reads_merged as f64 / stats.reads_completed as f64) * 100.0;
            assert!(read_merge_ratio >= 0.0 && read_merge_ratio <= 100.0);
        }
    }

    #[test]
    fn test_disk_stats_large_numbers() {
        // Test with large numbers from heavily used disks
        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 1_000_000_000,
            reads_merged: 10_000_000,
            read_bytes: 500_000_000_000,
            read_time_ms: 5_000_000,
            writes_completed: 800_000_000,
            writes_merged: 8_000_000,
            written_bytes: 400_000_000_000,
            write_time_ms: 4_000_000,
            io_in_progress: 20,
            io_time_ms: 9_000_000,
        };

        assert_eq!(stats.reads_completed, 1_000_000_000);
        assert_eq!(stats.read_bytes, 500_000_000_000);
    }

    #[test]
    fn test_disk_buffer_as_slice() {
        let mut buffer = DiskBuffer::new();

        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        buffer.push(stats);
        let slice = buffer.as_slice();

        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].device, "sda");
    }

    #[test]
    fn test_disk_buffer_clone() {
        let mut buffer1 = DiskBuffer::new();

        let stats = DiskStats {
            device: Cow::Borrowed("sda"),
            reads_completed: 10000,
            reads_merged: 100,
            read_bytes: 5000000,
            read_time_ms: 50000,
            writes_completed: 8000,
            writes_merged: 80,
            written_bytes: 4000000,
            write_time_ms: 40000,
            io_in_progress: 5,
            io_time_ms: 90000,
        };

        buffer1.push(stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
    }
}
