//! Collector selection and sampling interval.

use std::collections::HashSet;

use blazebee_mqtt_v3::EndpointMetadata;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    /// Seconds between two sampling cycles; at least 1.
    pub collection_interval: u64,
    pub collectors: CollectorsConfig,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval: 5,
            collectors: CollectorsConfig::default(),
        }
    }
}

impl MetricsConfig {
    pub fn enabled_names(&self) -> Vec<&str> {
        self.collectors
            .enabled
            .iter()
            .map(|c| c.name.as_str())
            .collect()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.collection_interval == 0 {
            return Err("metrics.collection_interval must be at least 1 second".to_string());
        }
        if self.collectors.enabled.is_empty() {
            return Err("metrics: at least one collector must be enabled".to_string());
        }
        let mut seen = HashSet::new();
        for entry in &self.collectors.enabled {
            if entry.name.is_empty() {
                return Err("metrics: collector name must not be empty".to_string());
            }
            if !seen.insert(entry.name.as_str()) {
                return Err(format!(
                    "metrics: collector '{}' is enabled twice",
                    entry.name
                ));
            }
            entry
                .metadata
                .validate()
                .map_err(|e| format!("metrics: collector '{}': {e}", entry.name))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CollectorsConfig {
    /// One entry per `[[metrics.collectors.enabled]]` table.
    pub enabled: Vec<CollectorConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CollectorConfig {
    /// Registered collector name, e.g. `cpu`.
    pub name: String,
    pub metadata: EndpointMetadata,
}
