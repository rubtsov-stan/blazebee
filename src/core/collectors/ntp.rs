use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// NTP synchronization statistics.
/// Contains information about time synchronization status and quality.
/// These statistics help monitor system clock accuracy and NTP service health.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtpStats<'a> {
    /// Whether the system time is currently synchronized with NTP servers
    pub synchronized: bool,
    /// NTP server being used for synchronization
    pub ntp_server: Cow<'a, str>,
    /// Stratum level (distance from reference clock, lower is better)
    pub stratum: u8,
    /// Time offset from the reference clock in milliseconds
    pub offset_ms: f64,
    /// Estimated timing jitter in milliseconds (lower is better)
    pub jitter_ms: f64,
    /// Root delay to the reference clock in milliseconds
    pub root_delay_ms: f64,
}

/// Buffer for storing NTP synchronization statistics.
/// Uses Option since typically there's only one NTP status for the system.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NtpBuffer<'a> {
    stats: Option<NtpStats<'a>>,
}

/// Methods for working with the NTP buffer
impl<'a> NtpBuffer<'a> {
    /// Creates a new empty NTP buffer
    pub fn new() -> Self {
        NtpBuffer { stats: None }
    }

    /// Sets the NTP statistics in the buffer
    pub fn set(&mut self, stats: NtpStats<'a>) {
        self.stats = Some(stats);
    }

    /// Returns a reference to the stored NTP statistics, if any
    pub fn get(&self) -> Option<&NtpStats<'a>> {
        self.stats.as_ref()
    }
}

/// The NTP collector that reads time synchronization information.
/// Uses timedatectl command to gather NTP synchronization status and metrics.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
pub struct NtpCollector;

/// Constructor methods for NtpCollector
impl NtpCollector {
    /// Creates a new NTP collector instance
    pub fn new() -> Self {
        NtpCollector
    }
}

/// Default trait implementation for NtpCollector
impl Default for NtpCollector {
    fn default() -> Self {
        NtpCollector::new()
    }
}

/// Data producer implementation for NTP collector.
/// Executes timedatectl command and parses its output to gather NTP synchronization metrics.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for NtpCollector {
    type Output = NtpBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        use tokio::process::Command;

        // Execute timedatectl command to get NTP synchronization status
        let output = Command::new("timedatectl")
            .arg("timesync-status")
            .output()
            .await
            .map_err(|source| CollectorError::CommandExecution {
                command: "timedatectl".to_string(),
                source,
            })?;

        let content = String::from_utf8_lossy(&output.stdout);
        let mut buffer = NtpBuffer::new();

        // Initialize default values for NTP statistics
        let mut synchronized = false;
        let mut ntp_server = String::from("unknown");
        let mut stratum = 0u8;
        let mut offset_ms = 0.0f64;
        let mut jitter_ms = 0.0f64;
        let mut root_delay_ms = 0.0f64;

        // Parse each line of timedatectl output
        for line in content.lines() {
            let line = line.trim();

            if line.contains("synchronized:") {
                // Check if time is synchronized
                synchronized = line.contains("yes");
            } else if line.contains("Server:") {
                // Extract NTP server address
                ntp_server = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("unknown")
                    .trim()
                    .to_string();
            } else if line.contains("Stratum:") {
                // Parse stratum level
                if let Some(value) = line.split(':').nth(1) {
                    stratum = value.trim().parse().unwrap_or(0);
                }
            } else if line.contains("Offset:") {
                // Parse time offset
                if let Some(value) = line.split(':').nth(1) {
                    offset_ms = value.trim().parse().unwrap_or(0.0);
                }
            } else if line.contains("Jitter:") {
                // Parse timing jitter
                if let Some(value) = line.split(':').nth(1) {
                    jitter_ms = value.trim().parse().unwrap_or(0.0);
                }
            } else if line.contains("Root delay:") {
                // Parse root delay
                if let Some(value) = line.split(':').nth(1) {
                    root_delay_ms = value.trim().parse().unwrap_or(0.0);
                }
            }
        }

        // Create and store NTP statistics
        buffer.set(NtpStats {
            synchronized,
            ntp_server: Cow::Owned(ntp_server),
            stratum,
            offset_ms,
            jitter_ms,
            root_delay_ms,
        });

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
register_collector!(NtpCollector, "ntp");

/// Fallback NTP statistics for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtpStats<'a> {
    /// Whether the system time is currently synchronized with NTP servers
    pub synchronized: bool,
    /// NTP server being used for synchronization
    pub ntp_server: Cow<'a, str>,
    /// Stratum level (distance from reference clock, lower is better)
    pub stratum: u8,
    /// Time offset from the reference clock in milliseconds
    pub offset_ms: f64,
    /// Estimated timing jitter in milliseconds (lower is better)
    pub jitter_ms: f64,
    /// Root delay to the reference clock in milliseconds
    pub root_delay_ms: f64,
}

/// Fallback buffer for storing NTP synchronization statistics
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NtpBuffer<'a> {
    stats: Option<NtpStats<'a>>,
}

/// Fallback methods for NTP buffer
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
impl<'a> NtpBuffer<'a> {
    /// Creates a new empty NTP buffer
    pub fn new() -> Self {
        NtpBuffer { stats: None }
    }

    /// Sets the NTP statistics in the buffer
    pub fn set(&mut self, stats: NtpStats<'a>) {
        self.stats = Some(stats);
    }

    /// Returns a reference to the stored NTP statistics, if any
    pub fn get(&self) -> Option<&NtpStats<'a>> {
        self.stats.as_ref()
    }
}

/// Fallback NTP collector for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
pub struct NtpCollector;

/// Fallback constructor methods for NtpCollector
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
impl NtpCollector {
    /// Creates a new NTP collector instance
    pub fn new() -> Self {
        NtpCollector
    }
}

/// Fallback Default trait implementation for NtpCollector
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
impl Default for NtpCollector {
    fn default() -> Self {
        NtpCollector::new()
    }
}

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for NtpCollector {
    type Output = NtpBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "NTP collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NTP tests
    #[test]
    fn test_ntp_stats_creation() {
        let ntp_stats = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("time.google.com"),
            stratum: 2,
            offset_ms: 0.5,
            jitter_ms: 0.1,
            root_delay_ms: 1.2,
        };

        assert!(ntp_stats.synchronized);
        assert_eq!(ntp_stats.stratum, 2);
        assert_eq!(ntp_stats.offset_ms, 0.5);
        assert_eq!(ntp_stats.ntp_server, "time.google.com");
    }

    #[test]
    fn test_ntp_buffer() {
        let mut buffer = NtpBuffer::new();
        // Buffer should be empty initially
        assert!(buffer.get().is_none());

        let ntp_stats = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("ntp.ubuntu.com"),
            stratum: 3,
            offset_ms: 1.5,
            jitter_ms: 0.3,
            root_delay_ms: 2.1,
        };

        buffer.set(ntp_stats);
        // Buffer should now contain the statistics
        assert!(buffer.get().is_some());

        let stored_stats = buffer.get().unwrap();
        assert_eq!(stored_stats.ntp_server, "ntp.ubuntu.com");
        assert_eq!(stored_stats.stratum, 3);
    }

    #[test]
    fn test_ntp_not_synchronized() {
        let ntp_stats = NtpStats {
            synchronized: false,
            ntp_server: Cow::Borrowed("unknown"),
            stratum: 0,
            offset_ms: 0.0,
            jitter_ms: 0.0,
            root_delay_ms: 0.0,
        };

        assert!(!ntp_stats.synchronized);
        assert_eq!(ntp_stats.stratum, 0);
        assert_eq!(ntp_stats.offset_ms, 0.0);
        assert_eq!(ntp_stats.ntp_server, "unknown");
    }

    #[test]
    fn test_ntp_large_offset() {
        let ntp_stats = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("pool.ntp.org"),
            stratum: 1,
            offset_ms: 5000.0, // Large offset of 5 seconds
            jitter_ms: 10.5,
            root_delay_ms: 50.0,
        };

        assert!(ntp_stats.synchronized);
        assert_eq!(ntp_stats.offset_ms, 5000.0);
        // Large offsets may indicate synchronization issues
        assert!(ntp_stats.offset_ms > 100.0);
    }

    #[test]
    fn test_ntp_buffer_clone() {
        let mut buffer1 = NtpBuffer::new();
        let ntp_stats = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("time.windows.com"),
            stratum: 4,
            offset_ms: 2.3,
            jitter_ms: 0.7,
            root_delay_ms: 3.1,
        };
        buffer1.set(ntp_stats);

        let buffer2 = buffer1.clone();
        assert!(buffer2.get().is_some());
        assert_eq!(
            buffer1.get().unwrap().stratum,
            buffer2.get().unwrap().stratum
        );
    }

    #[test]
    fn test_ntp_stats_serialize() {
        let ntp_stats = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("time.apple.com"),
            stratum: 2,
            offset_ms: 0.8,
            jitter_ms: 0.2,
            root_delay_ms: 1.5,
        };

        let json = serde_json::to_string(&ntp_stats).expect("serialization failed");
        assert!(json.contains("time.apple.com"));
        assert!(json.contains("2"));
        assert!(json.contains("0.8"));
        assert!(json.contains("\"synchronized\":true"));
    }

    #[test]
    fn test_ntp_stats_deserialize() {
        let json = r#"{
            "synchronized": false,
            "ntp_server": "local",
            "stratum": 0,
            "offset_ms": 1000.5,
            "jitter_ms": 5.2,
            "root_delay_ms": 10.8
        }"#;

        let ntp_stats: NtpStats = serde_json::from_str(json).expect("deserialization failed");
        assert!(!ntp_stats.synchronized);
        assert_eq!(ntp_stats.ntp_server, "local");
        assert_eq!(ntp_stats.stratum, 0);
        assert_eq!(ntp_stats.offset_ms, 1000.5);
    }

    #[test]
    fn test_ntp_buffer_empty() {
        let buffer = NtpBuffer::new();
        // Empty buffer should return None
        assert!(buffer.get().is_none());
    }

    #[test]
    fn test_ntp_collector_creation() {
        let _ = NtpCollector::new();
        let _ = NtpCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_ntp_stratum_quality() {
        let good_ntp = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("stratum1.example.com"),
            stratum: 1, // Best possible stratum
            offset_ms: 0.1,
            jitter_ms: 0.05,
            root_delay_ms: 0.5,
        };

        let poor_ntp = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("stratum10.example.com"),
            stratum: 10, // Poor quality
            offset_ms: 100.0,
            jitter_ms: 50.0,
            root_delay_ms: 200.0,
        };

        // Lower stratum indicates better time source
        assert!(good_ntp.stratum < poor_ntp.stratum);
        // Lower offset and jitter are better
        assert!(good_ntp.offset_ms < poor_ntp.offset_ms);
        assert!(good_ntp.jitter_ms < poor_ntp.jitter_ms);
    }

    #[test]
    fn test_ntp_buffer_replace() {
        let mut buffer = NtpBuffer::new();

        let stats1 = NtpStats {
            synchronized: false,
            ntp_server: Cow::Borrowed("old-server"),
            stratum: 0,
            offset_ms: 0.0,
            jitter_ms: 0.0,
            root_delay_ms: 0.0,
        };

        buffer.set(stats1);
        assert_eq!(buffer.get().unwrap().ntp_server, "old-server");

        let stats2 = NtpStats {
            synchronized: true,
            ntp_server: Cow::Borrowed("new-server"),
            stratum: 2,
            offset_ms: 0.5,
            jitter_ms: 0.1,
            root_delay_ms: 1.2,
        };

        buffer.set(stats2);
        // Buffer should now contain the new stats
        assert_eq!(buffer.get().unwrap().ntp_server, "new-server");
        assert!(buffer.get().unwrap().synchronized);
    }
}
