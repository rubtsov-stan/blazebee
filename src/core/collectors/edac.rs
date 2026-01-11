use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

// EDAC (ECC MEMORY ERRORS) COLLECTOR
/// EDAC memory error statistics.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdacStats<'a> {
    /// Memory controller name (e.g., "mc0", "mc1")
    pub controller: Cow<'a, str>,
    /// Number of correctable errors (CE)
    pub correctable_errors: u64,
    /// Number of uncorrectable errors (UE)
    pub uncorrectable_errors: u64,
    /// DIMM label or location
    pub dimm_label: Cow<'a, str>,
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EdacBuffer<'a> {
    stats: Vec<EdacStats<'a>>,
}

impl<'a> EdacBuffer<'a> {
    pub fn new() -> Self {
        EdacBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        EdacBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stats: EdacStats<'a>) {
        self.stats.push(stats);
    }

    pub fn iter(&self) -> impl Iterator<Item = &EdacStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// The EDAC collector that reads from /sys/devices/system/edac.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
))]
pub struct EdacCollector;

impl EdacCollector {
    pub fn new() -> Self {
        EdacCollector
    }
}

impl Default for EdacCollector {
    fn default() -> Self {
        EdacCollector::new()
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for EdacCollector {
    type Output = EdacBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        use tokio::fs;

        let mut buffer = EdacBuffer::with_capacity(8);
        let edac_path = "/sys/devices/system/edac/mc";

        let mut entries =
            fs::read_dir(edac_path)
                .await
                .map_err(|source| CollectorError::FileRead {
                    path: edac_path.to_string(),
                    source,
                })?;

        while let Some(entry) =
            entries
                .next_entry()
                .await
                .map_err(|source| CollectorError::FileRead {
                    path: edac_path.to_string(),
                    source,
                })?
        {
            let path = entry.path();
            let controller_name = entry.file_name().to_string_lossy().to_string();

            if !controller_name.starts_with("mc") {
                continue;
            }

            let ce_path = path.join("ce_count");
            let ue_path = path.join("ue_count");

            let ce_count = fs::read_to_string(&ce_path)
                .await
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);

            let ue_count = fs::read_to_string(&ue_path)
                .await
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);

            buffer.push(EdacStats {
                controller: Cow::Owned(controller_name),
                correctable_errors: ce_count,
                uncorrectable_errors: ue_count,
                dimm_label: Cow::Borrowed("unknown"),
            });
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
))]
register_collector!(EdacCollector, "edac");

/// Fallback EdacStats struct for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdacStats<'a> {
    /// Memory controller name (e.g., "mc0", "mc1")
    pub controller: Cow<'a, str>,
    /// Number of correctable errors (CE)
    pub correctable_errors: u64,
    /// Number of uncorrectable errors (UE)
    pub uncorrectable_errors: u64,
    /// DIMM label or location
    pub dimm_label: Cow<'a, str>,
}

/// Fallback EdacBuffer for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EdacBuffer<'a> {
    stats: Vec<EdacStats<'a>>,
}

/// Fallback methods for EdacBuffer
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
impl<'a> EdacBuffer<'a> {
    pub fn new() -> Self {
        EdacBuffer { stats: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        EdacBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, stats: EdacStats<'a>) {
        self.stats.push(stats);
    }

    pub fn iter(&self) -> impl Iterator<Item = &EdacStats<'a>> {
        self.stats.iter()
    }

    pub fn len(&self) -> usize {
        self.stats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// Fallback EdacCollector for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
pub struct EdacCollector;

/// Fallback constructor methods for EdacCollector
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
impl EdacCollector {
    pub fn new() -> Self {
        EdacCollector
    }
}

/// Fallback Default trait implementation for EdacCollector
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
impl Default for EdacCollector {
    fn default() -> Self {
        EdacCollector::new()
    }
}

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for EdacCollector {
    type Output = EdacBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "EDAC collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EDAC tests
    #[test]
    fn test_edac_stats_creation() {
        let edac_stats = EdacStats {
            controller: Cow::Borrowed("mc0"),
            correctable_errors: 10,
            uncorrectable_errors: 0,
            dimm_label: Cow::Borrowed("DIMM_A1"),
        };

        assert_eq!(edac_stats.controller, "mc0");
        assert_eq!(edac_stats.correctable_errors, 10);
        assert_eq!(edac_stats.uncorrectable_errors, 0);
    }

    #[test]
    fn test_edac_buffer() {
        let mut buffer = EdacBuffer::new();

        let edac_stats = EdacStats {
            controller: Cow::Borrowed("mc0"),
            correctable_errors: 5,
            uncorrectable_errors: 1,
            dimm_label: Cow::Borrowed("DIMM_B1"),
        };

        buffer.push(edac_stats);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_edac_no_errors() {
        let edac_stats = EdacStats {
            controller: Cow::Borrowed("mc0"),
            correctable_errors: 0,
            uncorrectable_errors: 0,
            dimm_label: Cow::Borrowed("unknown"),
        };

        assert_eq!(edac_stats.correctable_errors, 0);
        assert_eq!(edac_stats.uncorrectable_errors, 0);
    }

    #[test]
    fn test_edac_critical_errors() {
        let edac_stats = EdacStats {
            controller: Cow::Borrowed("mc1"),
            correctable_errors: 100,
            uncorrectable_errors: 5,
            dimm_label: Cow::Borrowed("DIMM_C2"),
        };

        // Uncorrectable errors are critical
        assert!(edac_stats.uncorrectable_errors > 0);
        assert_eq!(edac_stats.uncorrectable_errors, 5);
    }

    #[test]
    fn test_edac_collector_creation() {
        let _ = EdacCollector::new();
        let _ = EdacCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_edac_buffer_iterator() {
        let mut buffer = EdacBuffer::new();

        for i in 0..3 {
            let edac_stats = EdacStats {
                controller: Cow::Owned(format!("mc{}", i)),
                correctable_errors: i as u64 * 10,
                uncorrectable_errors: i as u64,
                dimm_label: Cow::Borrowed("DIMM_X"),
            };
            buffer.push(edac_stats);
        }

        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_edac_stats_serialize() {
        let edac_stats = EdacStats {
            controller: Cow::Borrowed("mc0"),
            correctable_errors: 25,
            uncorrectable_errors: 2,
            dimm_label: Cow::Borrowed("DIMM_0"),
        };

        let json = serde_json::to_string(&edac_stats).expect("serialization failed");
        assert!(json.contains("mc0"));
        assert!(json.contains("25"));
        assert!(json.contains("2"));
    }

    #[test]
    fn test_edac_stats_deserialize() {
        let json = r#"{
            "controller": "mc1",
            "correctable_errors": 50,
            "uncorrectable_errors": 1,
            "dimm_label": "DIMM_1"
        }"#;

        let edac_stats: EdacStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(edac_stats.controller, "mc1");
        assert_eq!(edac_stats.correctable_errors, 50);
        assert_eq!(edac_stats.uncorrectable_errors, 1);
    }

    #[test]
    fn test_edac_buffer_empty() {
        let buffer = EdacBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_edac_buffer_with_capacity() {
        let buffer = EdacBuffer::with_capacity(10);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_edac_buffer_clone() {
        let mut buffer1 = EdacBuffer::new();

        let edac_stats = EdacStats {
            controller: Cow::Borrowed("mc0"),
            correctable_errors: 10,
            uncorrectable_errors: 1,
            dimm_label: Cow::Borrowed("DIMM_A"),
        };

        buffer1.push(edac_stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
        assert_eq!(buffer1.len(), 1);
    }
}
