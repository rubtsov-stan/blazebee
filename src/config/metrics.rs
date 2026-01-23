//! Configuration structures for system metrics collection and publishing.
//!
//! This module defines the configuration types that control which metrics
//! are collected, how frequently they are gathered, and the metadata used
//! for publishing with different transport type.

#[cfg(feature = "blazebee-mqtt-v3")]
use blazebee_mqtt_v3::EndpointMetadata;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Represents a single enabled metric collector with its publishing metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct Collector {
    /// Unique name of the collector. Must not be empty.
    ///
    /// This name is used to look up the registered collector implementation.
    #[validate(length(min = 1, message = "Collector name must not be empty"))]
    pub name: String,

    /// Metadata defining how collected data should be published.
    pub metadata: CollectorMetadata,
}

#[cfg(feature = "blazebee-mqtt-v3")]
/// Type alias for collector metadata when using MQTT transport.
pub type CollectorMetadata = EndpointMetadata;

impl Default for Collector {
    fn default() -> Self {
        Self {
            name: "unknown".into(),
            metadata: CollectorMetadata::default(),
        }
    }
}

/// Configuration for the set of enabled collectors and collection timing.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct CollectorsConfig {
    /// List of collectors that are enabled and should be executed periodically.
    ///
    /// At least one collector must be specified.
    #[validate(length(
        min = 1,
        message = "At least one collector must be enabled, possible values: cpu, memory, disk, network, process, system, temperature"
    ))]
    pub enabled: Vec<Collector>,

    /// Interval (in seconds) at which collected data is refreshed in memory.
    ///
    /// Must be at least 1 second.
    #[validate(range(min = 1, message = "Refresh interval must be at least 1 second"))]
    pub refresh_interval: u64,

    /// Interval (in seconds) at which metrics are collected and published.
    ///
    /// Must be at least 1 second.
    #[validate(range(min = 1, message = "Collection must be at least 1 second"))]
    pub collection_interval: u64,
}

impl Default for CollectorsConfig {
    fn default() -> Self {
        let default_collectors = vec!["cpu", "avg", "disk", "network", "processes", "uptime"]
            .into_iter()
            .map(|name| Collector {
                name: name.into(),
                metadata: CollectorMetadata {
                    qos: 1,
                    topic: format!("metrics/{}", name),
                    retain: false,
                },
            })
            .collect();

        Self {
            enabled: default_collectors,
            refresh_interval: 60,
            collection_interval: 5,
        }
    }
}

impl CollectorsConfig {
    /// Return enabled collector names from CollectorConfig struct
    pub fn enabled_names(&self) -> Vec<&str> {
        self.enabled.iter().map(|c| c.name.as_str()).collect()
    }
}

/// Top-level metrics configuration container.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct MetricsConfig {
    /// Configuration for collectors and their execution schedule.
    #[serde(default)]
    pub collectors: CollectorsConfig,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collectors: CollectorsConfig::default(),
        }
    }
}
