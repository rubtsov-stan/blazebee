use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// Systemd unit state information.
/// Contains details about systemd services, sockets, timers, and other unit types.
/// These statistics help monitor service health and system state management.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdUnitStats<'a> {
    /// Unit name with type suffix (e.g., "nginx.service", "docker.socket", "daily-backup.timer")
    pub unit_name: Cow<'a, str>,
    /// Load state indicates if unit files were successfully loaded (e.g., "loaded", "not-found", "error")
    pub load_state: Cow<'a, str>,
    /// Active state indicates if unit is currently active (e.g., "active", "inactive", "failed", "activating")
    pub active_state: Cow<'a, str>,
    /// Sub state provides more detailed information about the active state (e.g., "running", "dead", "exited", "listening")
    pub sub_state: Cow<'a, str>,
    /// Human-readable description of the unit's purpose
    pub description: Cow<'a, str>,
}

/// Buffer for storing systemd unit statistics from multiple systemd units.
/// Uses Vec internally for efficient storage and iteration.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemdBuffer<'a> {
    stats: Vec<SystemdUnitStats<'a>>,
}

/// Methods for working with the systemd buffer
impl<'a> SystemdBuffer<'a> {
    /// Creates a new empty systemd buffer
    pub fn new() -> Self {
        SystemdBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory for the specified number of systemd units.
    /// Typical capacity might be 50-100 units for most systems.
    pub fn with_capacity(capacity: usize) -> Self {
        SystemdBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds systemd unit statistics entry to the buffer
    pub fn push(&mut self, stats: SystemdUnitStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the systemd unit statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &SystemdUnitStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of systemd unit entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// The Systemd collector that queries systemd via systemctl command.
/// Collects information about all systemd units and their current states.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
pub struct SystemdCollector;

/// Constructor methods for SystemdCollector
impl SystemdCollector {
    /// Creates a new systemd collector instance
    pub fn new() -> Self {
        SystemdCollector
    }
}

/// Default trait implementation for SystemdCollector
impl Default for SystemdCollector {
    fn default() -> Self {
        SystemdCollector::new()
    }
}

/// Data producer implementation for Systemd collector.
/// Executes systemctl command and parses its output to gather systemd unit information.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for SystemdCollector {
    type Output = SystemdBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        use tokio::process::Command;

        // Execute systemctl command to list all units
        let output = Command::new("systemctl")
            .args(&["list-units", "--all", "--no-pager", "--no-legend"])
            .output()
            .await
            .map_err(|source| CollectorError::CommandExecution {
                command: "systemctl".to_string(),
                source,
            })?;

        let content = String::from_utf8_lossy(&output.stdout);

        // Initialize buffer with capacity for typical number of units
        let mut buffer = SystemdBuffer::with_capacity(100);

        // Parse each line of systemctl output
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Each unit line should have at least unit name, load state, active state, and sub state
            if parts.len() < 4 {
                continue;
            }

            let unit_name = parts[0];
            let load_state = parts[1];
            let active_state = parts[2];
            let sub_state = parts[3];
            // Remaining parts form the description
            let description = parts[4..].join(" ");

            // Create systemd unit statistics entry
            buffer.push(SystemdUnitStats {
                unit_name: Cow::Owned(unit_name.to_string()),
                load_state: Cow::Owned(load_state.to_string()),
                active_state: Cow::Owned(active_state.to_string()),
                sub_state: Cow::Owned(sub_state.to_string()),
                description: Cow::Owned(description),
            });
        }

        Ok(buffer)
    }
}

#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
register_collector!(SystemdCollector, "systemd");

/// Fallback systemd unit statistics for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdUnitStats<'a> {
    /// Unit name with type suffix (e.g., "nginx.service", "docker.socket", "daily-backup.timer")
    pub unit_name: Cow<'a, str>,
    /// Load state indicates if unit files were successfully loaded (e.g., "loaded", "not-found", "error")
    pub load_state: Cow<'a, str>,
    /// Active state indicates if unit is currently active (e.g., "active", "inactive", "failed", "activating")
    pub active_state: Cow<'a, str>,
    /// Sub state provides more detailed information about the active state (e.g., "running", "dead", "exited", "listening")
    pub sub_state: Cow<'a, str>,
    /// Human-readable description of the unit's purpose
    pub description: Cow<'a, str>,
}

/// Fallback buffer for storing systemd unit statistics
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemdBuffer<'a> {
    stats: Vec<SystemdUnitStats<'a>>,
}

/// Fallback methods for systemd buffer
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
impl<'a> SystemdBuffer<'a> {
    /// Creates a new empty systemd buffer
    pub fn new() -> Self {
        SystemdBuffer { stats: Vec::new() }
    }

    /// Creates a buffer with pre-allocated memory
    pub fn with_capacity(capacity: usize) -> Self {
        SystemdBuffer {
            stats: Vec::with_capacity(capacity),
        }
    }

    /// Adds systemd unit statistics entry to the buffer
    pub fn push(&mut self, stats: SystemdUnitStats<'a>) {
        self.stats.push(stats);
    }

    /// Returns an iterator over the systemd unit statistics entries
    pub fn iter(&self) -> impl Iterator<Item = &SystemdUnitStats<'a>> {
        self.stats.iter()
    }

    /// Returns the number of systemd unit entries in the buffer
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }
}

/// Fallback systemd collector for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
pub struct SystemdCollector;

/// Fallback constructor methods for SystemdCollector
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
impl SystemdCollector {
    /// Creates a new systemd collector instance
    pub fn new() -> Self {
        SystemdCollector
    }
}

/// Fallback Default trait implementation for SystemdCollector
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
impl Default for SystemdCollector {
    fn default() -> Self {
        SystemdCollector::new()
    }
}

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for SystemdCollector {
    type Output = SystemdBuffer<'static>;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Systemd collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Systemd tests
    #[test]
    fn test_systemd_unit_stats_creation() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("nginx.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("active"),
            sub_state: Cow::Borrowed("running"),
            description: Cow::Borrowed("A high performance web server"),
        };

        assert_eq!(unit_stats.unit_name, "nginx.service");
        assert_eq!(unit_stats.active_state, "active");
        assert_eq!(unit_stats.sub_state, "running");
        assert_eq!(unit_stats.load_state, "loaded");
    }

    #[test]
    fn test_systemd_buffer() {
        let mut buffer = SystemdBuffer::with_capacity(10);

        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("docker.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("active"),
            sub_state: Cow::Borrowed("running"),
            description: Cow::Borrowed("Docker Application Container Engine"),
        };

        buffer.push(unit_stats);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_systemd_failed_unit() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("failed.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("failed"),
            sub_state: Cow::Borrowed("dead"),
            description: Cow::Borrowed("A failed service"),
        };

        assert_eq!(unit_stats.active_state, "failed");
        assert_eq!(unit_stats.sub_state, "dead");
        assert_eq!(unit_stats.load_state, "loaded"); // Unit file is loaded but service failed
    }

    #[test]
    fn test_systemd_inactive_unit() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("stopped.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("inactive"),
            sub_state: Cow::Borrowed("dead"),
            description: Cow::Borrowed("Stopped service"),
        };

        assert_eq!(unit_stats.active_state, "inactive");
        assert_eq!(unit_stats.sub_state, "dead");
        // Inactive services are not running but could be started
    }

    #[test]
    fn test_systemd_socket_unit() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("docker.socket"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("active"),
            sub_state: Cow::Borrowed("listening"),
            description: Cow::Borrowed("Docker Socket for the API"),
        };

        assert_eq!(unit_stats.unit_name, "docker.socket");
        assert_eq!(unit_stats.sub_state, "listening"); // Sockets typically have "listening" substate
    }

    #[test]
    fn test_systemd_buffer_multiple_units() {
        let mut buffer = SystemdBuffer::with_capacity(5);

        let units = vec![
            SystemdUnitStats {
                unit_name: Cow::Borrowed("nginx.service"),
                load_state: Cow::Borrowed("loaded"),
                active_state: Cow::Borrowed("active"),
                sub_state: Cow::Borrowed("running"),
                description: Cow::Borrowed("nginx web server"),
            },
            SystemdUnitStats {
                unit_name: Cow::Borrowed("postgresql.service"),
                load_state: Cow::Borrowed("loaded"),
                active_state: Cow::Borrowed("active"),
                sub_state: Cow::Borrowed("running"),
                description: Cow::Borrowed("PostgreSQL database server"),
            },
            SystemdUnitStats {
                unit_name: Cow::Borrowed("ssh.service"),
                load_state: Cow::Borrowed("loaded"),
                active_state: Cow::Borrowed("active"),
                sub_state: Cow::Borrowed("running"),
                description: Cow::Borrowed("OpenSSH server daemon"),
            },
        ];

        for unit in units {
            buffer.push(unit);
        }

        assert_eq!(buffer.len(), 3);
        let count = buffer.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_systemd_stats_serialize() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("cron.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("active"),
            sub_state: Cow::Borrowed("running"),
            description: Cow::Borrowed("Regular background program processing daemon"),
        };

        let json = serde_json::to_string(&unit_stats).expect("serialization failed");
        assert!(json.contains("cron.service"));
        assert!(json.contains("loaded"));
        assert!(json.contains("active"));
        assert!(json.contains("running"));
        assert!(json.contains("Regular background program processing daemon"));
    }

    #[test]
    fn test_systemd_stats_deserialize() {
        let json = r#"{
            "unit_name": "sshd.service",
            "load_state": "loaded",
            "active_state": "active",
            "sub_state": "running",
            "description": "OpenBSD Secure Shell server"
        }"#;

        let unit_stats: SystemdUnitStats =
            serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(unit_stats.unit_name, "sshd.service");
        assert_eq!(unit_stats.load_state, "loaded");
        assert_eq!(unit_stats.active_state, "active");
        assert_eq!(unit_stats.sub_state, "running");
        assert_eq!(unit_stats.description, "OpenBSD Secure Shell server");
    }

    #[test]
    fn test_systemd_buffer_empty() {
        let buffer = SystemdBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_systemd_buffer_clone() {
        let mut buffer1 = SystemdBuffer::new();

        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("network.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("active"),
            sub_state: Cow::Borrowed("running"),
            description: Cow::Borrowed("Network service"),
        };

        buffer1.push(unit_stats);
        let buffer2 = buffer1.clone();

        assert_eq!(buffer1.len(), buffer2.len());
        assert_eq!(buffer1.len(), 1);
    }

    #[test]
    fn test_systemd_unit_not_found() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("nonexistent.service"),
            load_state: Cow::Borrowed("not-found"),
            active_state: Cow::Borrowed("inactive"),
            sub_state: Cow::Borrowed("dead"),
            description: Cow::Borrowed(""),
        };

        assert_eq!(unit_stats.load_state, "not-found");
        // Unit file not found - cannot be loaded
    }

    #[test]
    fn test_systemd_collector_creation() {
        let _ = SystemdCollector::new();
        let _ = SystemdCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_systemd_timer_unit() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("systemd-tmpfiles-clean.timer"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("active"),
            sub_state: Cow::Borrowed("waiting"),
            description: Cow::Borrowed("Daily Cleanup of Temporary Directories"),
        };

        assert_eq!(unit_stats.unit_name, "systemd-tmpfiles-clean.timer");
        assert_eq!(unit_stats.sub_state, "waiting"); // Timers typically have "waiting" substate
    }

    #[test]
    fn test_systemd_activating_state() {
        let unit_stats = SystemdUnitStats {
            unit_name: Cow::Borrowed("starting.service"),
            load_state: Cow::Borrowed("loaded"),
            active_state: Cow::Borrowed("activating"),
            sub_state: Cow::Borrowed("start-pre"),
            description: Cow::Borrowed("Service currently starting"),
        };

        assert_eq!(unit_stats.active_state, "activating");
        assert_eq!(unit_stats.sub_state, "start-pre");
        // Service is in the process of starting up
    }
}
