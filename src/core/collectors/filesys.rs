use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use tracing::trace;

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Filesystem statistics for a mounted filesystem.
/// Contains information about disk space usage and availability.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemStats<'a> {
    /// Device name (e.g., "/dev/sda1")
    pub device: Cow<'a, str>,
    /// Mount point (e.g., "/", "/home")
    pub mount_point: Cow<'a, str>,
    /// Filesystem type (e.g., "ext4", "xfs", "btrfs")
    pub fs_type: Cow<'a, str>,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Used space in bytes
    pub used_bytes: u64,
    /// Available space in bytes
    pub available_bytes: u64,
    /// Total number of inodes
    pub total_inodes: u64,
    /// Used inodes
    pub used_inodes: u64,
    /// Available inodes
    pub available_inodes: u64,
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilesystemBuffer<'a> {
    stats: Vec<FilesystemStats<'a>>,
}

impl<'a> FilesystemBuffer<'a> {
    pub fn new() -> Self {
        FilesystemBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        FilesystemBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stats: FilesystemStats<'a>) {
        self.stats.push(stats);
    }

    pub fn iter(&self) -> impl Iterator<Item = &FilesystemStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// The Filesystem collector that reads from /proc/mounts and uses statvfs.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
pub struct FilesystemCollector;

impl FilesystemCollector {
    pub fn new() -> Self {
        FilesystemCollector
    }
}

impl Default for FilesystemCollector {
    fn default() -> Self {
        FilesystemCollector::new()
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for FilesystemCollector {
    type Output = FilesystemBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        let mounts_content = tokio::fs::read_to_string("/proc/mounts")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/mounts".to_string(),
                source,
            })?;

        let mut buffer = FilesystemBuffer::new();

        for line in mounts_content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let parts: Vec<String> = trimmed.split_whitespace().map(|s| s.to_string()).collect();
            if parts.len() < 6 {
                // min: device mount_point fs_type options freq passno
                trace!("Skipping invalid mount line: {}", trimmed);
                continue;
            }

            let device = parts[0].clone();
            let mount_point = parts[1].clone();
            let fs_type = parts[2].clone();

            if fs_type == "proc"
                || fs_type == "sysfs"
                || fs_type == "devtmpfs"
                || fs_type == "tmpfs"
                || fs_type == "cgroup"
                || fs_type == "cgroup2"
                || fs_type == "devpts"
                || fs_type == "securityfs"
                || fs_type == "pstore"
                || fs_type == "overlay"
                || fs_type == "squashfs"
                || fs_type == "fuse"
                || fs_type.starts_with("fuse.")
            {
                continue;
            }

            let c_path = match std::ffi::CString::new(mount_point.as_bytes()) {
                Ok(p) => p,
                Err(e) => {
                    trace!("Invalid mount point '{}': {}", mount_point, e);
                    continue;
                }
            };

            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };

            let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

            if result != 0 {
                let err = std::io::Error::last_os_error();
                trace!("statvfs failed for '{}': {}", mount_point, err);
                continue;
            }

            let block_size = stat.f_frsize as u64;
            let total_bytes = stat.f_blocks.wrapping_mul(block_size);
            let available_bytes = stat.f_bavail.wrapping_mul(block_size);
            let used_bytes = total_bytes.saturating_sub(available_bytes);

            buffer.push(FilesystemStats {
                device: Cow::Owned(device),
                mount_point: Cow::Owned(mount_point),
                fs_type: Cow::Owned(fs_type),
                total_bytes,
                used_bytes,
                available_bytes,
                total_inodes: stat.f_files,
                used_inodes: stat.f_files.saturating_sub(stat.f_favail),
                available_inodes: stat.f_favail,
            });
        }

        trace!("Collected {} filesystem entries", buffer.len());
        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
register_collector!(FilesystemCollector, "filesystem");

/// Fallback FilesystemStats struct for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemStats<'a> {
    /// Device name (e.g., "/dev/sda1")
    pub device: Cow<'a, str>,
    /// Mount point (e.g., "/", "/home")
    pub mount_point: Cow<'a, str>,
    /// Filesystem type (e.g., "ext4", "xfs", "btrfs")
    pub fs_type: Cow<'a, str>,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Used space in bytes
    pub used_bytes: u64,
    /// Available space in bytes
    pub available_bytes: u64,
    /// Total number of inodes
    pub total_inodes: u64,
    /// Used inodes
    pub used_inodes: u64,
    /// Available inodes
    pub available_inodes: u64,
}

/// Fallback FilesystemBuffer for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilesystemBuffer<'a> {
    stats: Vec<FilesystemStats<'a>>,
}

/// Fallback methods for FilesystemBuffer
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
impl<'a> FilesystemBuffer<'a> {
    pub fn new() -> Self {
        FilesystemBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        FilesystemBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stats: FilesystemStats<'a>) {
        self.stats.push(stats);
    }

    pub fn iter(&self) -> impl Iterator<Item = &FilesystemStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// Fallback FilesystemCollector for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
pub struct FilesystemCollector;

/// Fallback constructor methods for FilesystemCollector
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
impl FilesystemCollector {
    pub fn new() -> Self {
        FilesystemCollector
    }
}

/// Fallback Default trait implementation for FilesystemCollector
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
impl Default for FilesystemCollector {
    fn default() -> Self {
        FilesystemCollector::new()
    }
}

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for FilesystemCollector {
    type Output = FilesystemBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Filesystem collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Filesystem tests
    #[test]
    fn test_filesystem_stats_creation() {
        let fs_stats = FilesystemStats {
            device: Cow::Borrowed("/dev/sda1"),
            mount_point: Cow::Borrowed("/"),
            fs_type: Cow::Borrowed("ext4"),
            total_bytes: 1_000_000_000_000,
            used_bytes: 500_000_000_000,
            available_bytes: 500_000_000_000,
            total_inodes: 10_000_000,
            used_inodes: 5_000_000,
            available_inodes: 5_000_000,
        };

        assert_eq!(fs_stats.device, "/dev/sda1");
        assert_eq!(fs_stats.mount_point, "/");
        assert_eq!(fs_stats.fs_type, "ext4");
        assert_eq!(fs_stats.total_bytes, 1_000_000_000_000);
    }

    #[test]
    fn test_filesystem_buffer() {
        let mut buffer = FilesystemBuffer::new();
        assert!(buffer.is_empty());

        let fs_stats = FilesystemStats {
            device: Cow::Borrowed("/dev/sda1"),
            mount_point: Cow::Borrowed("/"),
            fs_type: Cow::Borrowed("ext4"),
            total_bytes: 1_000_000_000_000,
            used_bytes: 500_000_000_000,
            available_bytes: 500_000_000_000,
            total_inodes: 10_000_000,
            used_inodes: 5_000_000,
            available_inodes: 5_000_000,
        };

        buffer.push(fs_stats);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_filesystem_usage_calculation() {
        let fs_stats = FilesystemStats {
            device: Cow::Borrowed("/dev/sda1"),
            mount_point: Cow::Borrowed("/"),
            fs_type: Cow::Borrowed("ext4"),
            total_bytes: 1_000_000_000,
            used_bytes: 750_000_000,
            available_bytes: 250_000_000,
            total_inodes: 1_000_000,
            used_inodes: 800_000,
            available_inodes: 200_000,
        };

        let usage_percent = (fs_stats.used_bytes as f64 / fs_stats.total_bytes as f64) * 100.0;
        assert_eq!(usage_percent, 75.0);
    }

    #[test]
    fn test_filesystem_stats_serialize() {
        let fs_stats = FilesystemStats {
            device: Cow::Borrowed("/dev/sda1"),
            mount_point: Cow::Borrowed("/"),
            fs_type: Cow::Borrowed("ext4"),
            total_bytes: 1_000_000_000,
            used_bytes: 750_000_000,
            available_bytes: 250_000_000,
            total_inodes: 1_000_000,
            used_inodes: 800_000,
            available_inodes: 200_000,
        };

        let json = serde_json::to_string(&fs_stats).expect("serialization failed");
        assert!(json.contains("/dev/sda1"));
        assert!(json.contains("/"));
        assert!(json.contains("ext4"));
    }

    #[test]
    fn test_filesystem_buffer_multiple() {
        let mut buffer = FilesystemBuffer::with_capacity(3);

        for i in 0..3 {
            let fs_stats = FilesystemStats {
                device: Cow::Owned(format!("/dev/sda{}", i + 1)),
                mount_point: Cow::Owned(format!("/mnt/disk{}", i)),
                fs_type: Cow::Borrowed("ext4"),
                total_bytes: 1_000_000_000,
                used_bytes: 500_000_000,
                available_bytes: 500_000_000,
                total_inodes: 1_000_000,
                used_inodes: 500_000,
                available_inodes: 500_000,
            };
            buffer.push(fs_stats);
        }

        assert_eq!(buffer.len(), 3);
        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_filesystem_collector_creation() {
        let _ = FilesystemCollector::new();
        let _ = FilesystemCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_filesystem_buffer_clone() {
        let mut buffer1 = FilesystemBuffer::new();

        let fs_stats = FilesystemStats {
            device: Cow::Borrowed("/dev/sda1"),
            mount_point: Cow::Borrowed("/"),
            fs_type: Cow::Borrowed("ext4"),
            total_bytes: 1_000_000_000,
            used_bytes: 500_000_000,
            available_bytes: 500_000_000,
            total_inodes: 1_000_000,
            used_inodes: 500_000,
            available_inodes: 500_000,
        };

        buffer1.push(fs_stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
        assert_eq!(buffer1.len(), 1);
    }

    #[test]
    fn test_filesystem_stats_deserialize() {
        let json = r#"{
            "device": "/dev/sdb1",
            "mount_point": "/home",
            "fs_type": "xfs",
            "total_bytes": 2000000000,
            "used_bytes": 1000000000,
            "available_bytes": 1000000000,
            "total_inodes": 2000000,
            "used_inodes": 1000000,
            "available_inodes": 1000000
        }"#;

        let fs_stats: FilesystemStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(fs_stats.device, "/dev/sdb1");
        assert_eq!(fs_stats.mount_point, "/home");
        assert_eq!(fs_stats.fs_type, "xfs");
        assert_eq!(fs_stats.total_bytes, 2_000_000_000);
    }

    #[test]
    fn test_filesystem_zero_capacity() {
        let fs_stats = FilesystemStats {
            device: Cow::Borrowed("/dev/sda1"),
            mount_point: Cow::Borrowed("/"),
            fs_type: Cow::Borrowed("ext4"),
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            total_inodes: 0,
            used_inodes: 0,
            available_inodes: 0,
        };

        assert_eq!(fs_stats.total_bytes, 0);
        assert_eq!(fs_stats.available_bytes, 0);
        // Zero capacity filesystem is unusual but possible
    }
}
