use std::{
    process,
    sync::{Arc, OnceLock},
};

use blazebee::{
    config::Config,
    core::{collectors::registry::Collectors, executor::Executor, readiness::Readiness},
    logger::LoggerManager,
    print_error,
};
use tracing::{debug, error, info};

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        Config::new().unwrap_or_else(|e| {
            print_error!("{}", e);
            process::exit(1);
        })
    })
}

fn log_collectors_table(enabled: Vec<&str>, available: Vec<&'static str>) {
    use std::collections::BTreeSet;

    let enabled_set: BTreeSet<&str> = enabled.into_iter().collect();
    let available_set: BTreeSet<&str> = available.into_iter().collect();

    // Union of both sets to show *everything* explicitly
    let all_names: BTreeSet<&str> = enabled_set
        .iter()
        .copied()
        .chain(available_set.iter().copied())
        .collect();

    let name_width = all_names
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(10)
        .max("Collector".len());

    let header = format!("{:<width$} | Status", "Collector", width = name_width);
    let sep = format!("{}-+-{}", "-".repeat(name_width), "-".repeat(12));

    info!("{}", header);
    info!("{}", sep);

    for name in all_names {
        let status = match (enabled_set.contains(name), available_set.contains(name)) {
            // Configured and present in registry — normal case
            (true, true) => "ENABLED",

            // Enabled in config but missing in registry — configuration error
            (true, false) => "ENABLED (missing)",

            // Present in registry but not enabled
            (false, true) => "DISABLED",

            // Theoretically impossible, but kept for completeness
            (false, false) => "UNKNOWN",
        };

        info!("{:<width$} | {}", name, status, width = name_width);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = config();
    let mut logger_manager = LoggerManager::new(cfg.logger.clone()).unwrap_or_else(|e| {
        print_error!("Failed to setup Log Manager: {}", e);
        process::exit(1);
    });
    info!("Starting blazebee version {}...", env!("CARGO_PKG_VERSION"));
    logger_manager.init().unwrap_or_else(|e| {
        print_error!("Failed to init Log Manager: {}", e);
        process::exit(1);
    });
    debug!("{:#?}", cfg.transport);
    info!("Log level: {}", cfg.logger.level);

    log_collectors_table(cfg.metrics.collectors.enabled_names(), Collectors::list());
    let readiness = Readiness::default();

    let (publisher, instance): (Arc<dyn blazebee::core::executor::Publisher>, _) = {
        #[cfg(feature = "blazebee-mqtt-v4")]
        {
            info!("MQTT transport enabled");
            info!("Starting MQTT client...");

            let manager = blazebee_mqtt_v4::MqttManager::from_config(cfg.transport.clone())
                .unwrap_or_else(|e| {
                    error!("Failed to create MqttManager: {}", e);
                    process::exit(1);
                });

            let instance = manager.build_and_start().await.unwrap_or_else(|e| {
                error!("Failed to create build and start MQTT kernel: {}", e);
                process::exit(1);
            });

            info!("MQTT client started");

            instance.start_monitoring().await.unwrap_or_else(|e| {
                error!("Failed to start monitoring: {}", e);
                process::exit(1);
            });

            info!("State monitoring started");

            readiness
                .start_listening(instance.supervisor().state_receiver().clone())
                .await;

            for collector in &cfg.metrics.collectors.enabled {
                use blazebee::core::collectors::registry::Collectors;

                let topic = &collector.metadata.topic;
                match Collectors::get(&collector.name) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Collector '{}' not found in registry", &collector.name);
                        debug!(
                            "Collector '{}' not found in registry: details: {:#?}",
                            &collector.name, e
                        );
                        instance.cancel_token().cancel();
                        process::exit(1);
                    }
                }
                if let Err(e) = instance.subscribe(topic).await {
                    error!("Failed to subscribe to metric topic '{}': {}", topic, e);
                } else {
                    debug!("Subscribed to retain topic: {}", topic);
                }
            }

            let ser_config = cfg.transport.serialization.clone().unwrap().into_config();

            info!(
                "MQTT Publisher configured: {} format + {} compression (threshold: {} bytes)",
                ser_config.format, ser_config.compression, ser_config.compression_threshold
            );

            let mqtt_publisher = blazebee_mqtt_v4::Publisher::with_config(
                Arc::new(instance.clone()),
                &ser_config,
                &cfg.transport.base_topic,
            );

            (
                Arc::new(mqtt_publisher) as Arc<dyn blazebee::core::executor::Publisher>,
                instance,
            )
        }

        #[cfg(not(feature = "blazebee-mqtt-v4"))]
        {
            info!("Running without MQTT (noop publisher)");
            struct NoopPublisher;
            #[async_trait::async_trait]
            impl blazebee::core::executor::Publisher for NoopPublisher {
                async fn publish(
                    &self,
                    _data: &(dyn erased_serde::Serialize + Send + Sync),
                    _meta: &blazebee::config::metrics::CollectorMetadata,
                ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    Ok(())
                }
            }
            let noop = Arc::new(NoopPublisher) as Arc<dyn blazebee::core::executor::Publisher>;
            (noop, std::mem::MaybeUninit::uninit().assume_init())
        }
    };

    let executor = Executor::new(publisher, Arc::from(cfg.metrics.clone()), readiness);

    info!("Starting metrics collection executor...");

    tokio::select! {
        _ = executor.run() => {
            error!("Executor unexpectedly finished");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C — initiating graceful shutdown...");

            #[cfg(feature = "blazebee-mqtt-v4")]
            {
                instance.cancel_token().cancel();
                debug!("Cancellation token triggered — MQTT disconnecting...");
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            }

            info!("Shutdown complete");
        }
    }
    Ok(())
}
