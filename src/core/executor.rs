//! Metrics collection and publishing executor.
//!
//! The `Executor` is responsible for periodically collecting system metrics
//! from registered collectors and publishing them via the configured publisher.
//! It waits for system readiness before starting collection and runs indefinitely.

use std::sync::Arc;

use erased_serde::Serialize;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, trace, warn};

use super::{collectors::registry::Collectors, readiness::Readiness};
use crate::config::metrics::{CollectorMetadata, MetricsConfig};

/// Trait for publishers that can send metric data to an external system.
#[async_trait::async_trait]
pub trait Publisher: Send + Sync {
    /// Publishes serialized data to the destination defined in metadata.
    async fn publish(
        &self,
        data: &(dyn Serialize + Send + Sync),
        meta: &CollectorMetadata,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Executor that manages periodic collection and publishing of metrics.
pub struct Executor {
    publisher: Arc<dyn Publisher>,
    config: Arc<MetricsConfig>,
    readiness: Readiness,
}

impl Executor {
    /// Creates a new Executor instance.
    ///
    /// # Arguments
    /// * `publisher` - Arc-wrapped publisher implementation
    /// * `config` - Metrics configuration
    /// * `readiness` - Readiness state monitor
    pub fn new(
        publisher: Arc<dyn Publisher>,
        config: Arc<MetricsConfig>,
        readiness: Readiness,
    ) -> Self {
        Self {
            publisher,
            config,
            readiness,
        }
    }

    /// Runs the executor loop indefinitely.
    ///
    /// Waits for system readiness, then collects and publishes metrics
    /// at the configured interval.
    pub async fn run(self) -> ! {
        // Wait for readiness before starting collection
        {
            let mut rx = self.readiness.subscribe();
            if rx.borrow().is_ready() {
                info!("System is already ready — starting metrics collection");
            } else {
                warn!("Waiting for system readiness... Current: {}", *rx.borrow());
                loop {
                    tokio::select! {
                        _ = rx.changed() => {
                            let state = rx.borrow().clone();
                            if state.is_ready() {
                                info!("System is READY! Starting metrics collection");
                                break;
                            } else {
                                warn!("Still not ready: {}", state);
                            }
                        }
                        _ = sleep(Duration::from_secs(30)) => {
                            warn!("Still waiting for readiness... Current: {}", *rx.borrow());
                        }
                    }
                }
            }
        }

        let interval = Duration::from_secs(self.config.collectors.collection_interval);
        info!(
            "Metrics collection started (interval: {}s)",
            self.config.collectors.collection_interval
        );

        loop {
            let start = Instant::now();

            // Spawn tasks for each enabled collector
            let tasks: Vec<_> = self
                .config
                .collectors
                .enabled
                .iter()
                .cloned()
                .map(|col| {
                    let publisher = self.publisher.clone();
                    let name = col.name.clone();
                    let meta = col.metadata.clone();
                    tokio::spawn(async move {
                        let collector = match Collectors::get(&name) {
                            Ok(c) => c,
                            Err(e) => {
                                error!("Collector '{}' not found: {:?}", name, e);
                                return;
                            }
                        };

                        let data = match collector.produce_dyn().await {
                            Ok(d) => d,
                            Err(e) => {
                                error!("Failed to collect from '{}': {}", name, e);
                                return;
                            }
                        };

                        log_memory_stats().await;

                        debug!("Collected data from '{}'", name);

                        if let Err(e) = publisher.publish(&*data, &meta).await {
                            error!("Publish failed for '{}': {:?}", name, e);
                        }
                    })
                })
                .collect();

            // Wait for all collection tasks to complete
            for task in tasks {
                let _ = task.await;
            }

            let elapsed = start.elapsed();
            if elapsed < interval {
                sleep(interval - elapsed).await;
            }
        }
    }
}

/// Logs current resident memory usage
#[cfg(target_os = "linux")]
pub async fn log_memory_stats() {
    let path = std::path::Path::new("/proc/self/statm");

    match tokio::fs::read_to_string(path).await {
        Ok(content) => {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(resident_pages) = parts[1].parse::<u64>() {
                    let page_size = 4096u64;
                    let resident_bytes = resident_pages * page_size;
                    let resident_mib = resident_bytes / 1024 / 1024;
                    trace!("Resident memory: {} MiB", resident_mib);
                } else {
                    debug!("Failed to parse resident pages from statm");
                }
            } else {
                debug!("Invalid format in /proc/self/statm");
            }
        }
        Err(e) => {
            trace!("Could not read /proc/self/statm: {}", e);
        }
    }
}

/// MQTT-specific publisher implementation.
#[cfg(feature = "blazebee-mqtt-v4")]
#[async_trait::async_trait]
impl Publisher for blazebee_mqtt_v4::publisher::Publisher {
    async fn publish(
        &self,
        data: &(dyn Serialize + Send + Sync),
        meta: &CollectorMetadata,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.publish_framed(data, meta)
            .await
            .map_err(|e| Box::new(e) as _)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use erased_serde::Serialize;
    use serde::Deserialize;
    use tokio::time::{sleep, Duration};
    use tracing_test::traced_test;

    use super::*;

    #[derive(Default)]
    struct MockPublisher {
        publish_count: std::sync::Mutex<usize>,
        last_data: std::sync::Mutex<Option<String>>,
    }

    #[async_trait::async_trait]
    impl Publisher for MockPublisher {
        async fn publish(
            &self,
            data: &(dyn Serialize + Send + Sync),
            _meta: &CollectorMetadata,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut count = self.publish_count.lock().unwrap();
            *count += 1;

            let json = serde_json::to_string(data).unwrap();
            *self.last_data.lock().unwrap() = Some(json);

            Ok(())
        }
    }

    impl MockPublisher {
        fn publish_count(&self) -> usize {
            *self.publish_count.lock().unwrap()
        }

        fn last_data(&self) -> String {
            self.last_data.lock().unwrap().clone().unwrap_or_default()
        }
    }

    use crate::core::{
        collectors::{traits::DataProducer, types::CollectorResult},
        readiness::ReadinessState,
    };

    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    struct DummyOutput {
        value: u32,
    }

    #[derive(Debug, Clone, serde::Serialize, Deserialize)]
    struct DummyCollector;

    #[async_trait::async_trait]
    impl DataProducer for DummyCollector {
        type Output = DummyOutput;

        async fn produce(&self) -> CollectorResult<Self::Output> {
            Ok(DummyOutput { value: 42 })
        }
    }
    impl Default for DummyCollector {
        fn default() -> Self {
            DummyCollector
        }
    }

    static INIT: std::sync::Once = std::sync::Once::new();

    fn init_registry() {
        INIT.call_once(|| {
            use crate::register_collector;
            register_collector!(DummyCollector, "dummy");
        });
    }

    #[tokio::test]
    #[traced_test]
    async fn executor_waits_for_ready_state() {
        init_registry();

        let readiness = Readiness::new();
        readiness.set_state(ReadinessState::NotReadyYet("test".into()));

        let publisher = Arc::new(MockPublisher::default());
        let config = Arc::new(MetricsConfig {
            collectors: crate::config::metrics::CollectorsConfig {
                enabled: vec![crate::config::metrics::Collector {
                    name: "dummy".into(),
                    metadata: CollectorMetadata {
                        topic: "test/dummy".into(),
                        qos: 1,
                        retain: false,
                    },
                }],
                collection_interval: 1,
                ..Default::default()
            },
            ..Default::default()
        });

        let executor = Executor::new(publisher.clone(), config, readiness.clone());

        let handle = tokio::spawn(async move {
            executor.run().await;
        });

        sleep(Duration::from_millis(300)).await;
        assert_eq!(publisher.publish_count(), 0);

        readiness.set_state(ReadinessState::Ready);

        sleep(Duration::from_millis(500)).await;

        assert!(publisher.publish_count() > 0);
        assert!(publisher.last_data().contains("\"value\":42"));

        handle.abort();
    }

    #[tokio::test]
    #[traced_test]
    async fn executor_starts_immediately_if_already_ready() {
        init_registry();

        let readiness = Readiness::new();
        readiness.set_state(ReadinessState::Ready);

        let publisher = Arc::new(MockPublisher::default());

        let config = Arc::new(MetricsConfig {
            collectors: crate::config::metrics::CollectorsConfig {
                enabled: vec![crate::config::metrics::Collector {
                    name: "dummy".into(),
                    metadata: CollectorMetadata::default(),
                }],
                collection_interval: 1,
                ..Default::default()
            },
            ..Default::default()
        });

        let executor = Executor::new(publisher.clone(), config, readiness);

        let handle = tokio::spawn(async move {
            executor.run().await;
        });

        sleep(Duration::from_millis(800)).await;

        assert!(
            publisher.publish_count() >= 1,
            "Should have published at least once"
        );

        handle.abort();
    }

    #[tokio::test]
    #[traced_test]
    async fn executor_handles_collector_not_found() {
        let readiness = Readiness::new();
        readiness.set_state(ReadinessState::Ready);

        let publisher = Arc::new(MockPublisher::default());

        let config = Arc::new(MetricsConfig {
            collectors: crate::config::metrics::CollectorsConfig {
                enabled: vec![crate::config::metrics::Collector {
                    name: "non_existent_collector_123".into(),
                    metadata: CollectorMetadata::default(),
                }],
                collection_interval: 1,
                ..Default::default()
            },
            ..Default::default()
        });

        let executor = Executor::new(publisher.clone(), config, readiness);

        let handle = tokio::spawn(async move {
            executor.run().await;
        });

        sleep(Duration::from_millis(500)).await;

        assert_eq!(publisher.publish_count(), 0);
        assert!(logs_contain(
            "Collector 'non_existent_collector_123' not found"
        ));

        handle.abort();
    }

    #[tokio::test]
    #[traced_test]
    async fn executor_keeps_running_after_readiness() {
        init_registry();

        let readiness = Readiness::new();
        readiness.set_state(ReadinessState::NotReadyYet("initial".into()));

        let publisher = Arc::new(MockPublisher::default());
        let config = Arc::new(MetricsConfig {
            collectors: crate::config::metrics::CollectorsConfig {
                enabled: vec![crate::config::metrics::Collector {
                    name: "dummy".into(),
                    metadata: CollectorMetadata::default(),
                }],
                collection_interval: 1,
                ..Default::default()
            },
            ..Default::default()
        });

        let executor = Executor::new(publisher.clone(), config, readiness.clone());

        let handle = tokio::spawn(async move {
            executor.run().await;
        });

        sleep(Duration::from_millis(200)).await;
        readiness.set_state(ReadinessState::Ready);
        sleep(Duration::from_secs(2)).await;

        let count = publisher.publish_count();
        assert!(
            count >= 2,
            "Should have published multiple times after Ready"
        );

        handle.abort();
    }
}
