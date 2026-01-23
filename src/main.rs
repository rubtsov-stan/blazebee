use std::{
    collections::HashSet,
    process,
    sync::{Arc, OnceLock},
};

use blazebee::{
    config::Config,
    core::{collectors::registry::Collectors, executor::Executor, readiness::Readiness},
    logger::LoggerManager,
    print_error, print_info,
};
use console::{style, Term};
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

fn print_banner() {
    const LOGO: &[&str] = &[
        "██████╗ ██╗      █████╗ ███████╗███████╗██████╗ ███████╗███████╗",
        "██╔══██╗██║     ██╔══██╗╚══███╔╝██╔════╝██╔══██╗██╔════╝██╔════╝",
        "██████╔╝██║     ███████║  ███╔╝ █████╗  ██████╔╝█████╗  █████╗  ",
        "██╔══██╗██║     ██╔══██║ ███╔╝  ██╔══╝  ██╔══██╗██╔══╝  ██╔══╝  ",
        "██████╔╝███████╗██║  ██║███████╗███████╗██████╔╝███████╗███████╗",
        "╚═════╝ ╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚═════╝ ╚══════╝╚══════╝",
    ];

    let term = Term::stdout();
    let term_width = term.size().1 as usize;

    let logo_width = LOGO.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    let box_width = logo_width;
    let total_width = box_width + 4;

    let outer_pad = term_width.saturating_sub(total_width) / 2;

    let pad = " ".repeat(outer_pad);
    let horizontal = "═".repeat(box_width + 2);

    term.write_line(&format!("{pad}╔{horizontal}╗")).ok();

    for line in LOGO {
        let line_width = line.chars().count();
        let inner_pad = box_width.saturating_sub(line_width);
        let left = inner_pad / 2;
        let right = inner_pad - left;

        term.write_line(&format!(
            "{pad}║ {}{}{} ║",
            " ".repeat(left),
            style(line).bold(),
            " ".repeat(right),
        ))
        .ok();
    }

    term.write_line(&format!("{pad}╚{horizontal}╝")).ok();
    term.write_line("").ok();
}

fn print_collectors_table(enabled: &[&str], available: &[&'static str]) {
    use std::collections::HashSet;

    let enabled_set: HashSet<&str> = enabled.iter().copied().collect();

    let table: String = available
        .iter()
        .map(|&collector| {
            let status = if enabled_set.contains(collector) {
                format!("{} {}", style("+").green(), style(collector).bold())
            } else {
                format!("{} {}", style("-").dim(), style(collector).dim())
            };
            format!("    {}", status)
        })
        .collect::<Vec<_>>()
        .join("\n");

    print_info!("Available collectors:\n{}", table);
}

pub fn print_config_toml<T>(config: &T)
where
    T: serde::Serialize,
{
    let toml = toml::to_string_pretty(config)
        .unwrap_or_else(|_| "<failed to serialize config>".to_string());

    println!("{}", format_toml(&toml));
}

#[inline]
fn ui_debug<F: FnOnce()>(f: F) {
    if cfg!(debug_assertions) && tracing::level_enabled!(tracing::Level::DEBUG) {
        f();
    }
}

fn format_toml(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            if line.starts_with('[') {
                console::style(line).cyan().bold().to_string()
            } else if let Some((k, v)) = line.split_once('=') {
                format!(
                    "{} = {}",
                    console::style(k.trim()).yellow().bold(),
                    console::style(v.trim()).green()
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_banner();
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
    ui_debug(|| {
        print_config_toml(cfg);
    });
    info!("Log level: {}", cfg.logger.level);

    let enabled_names = cfg.metrics.collectors.enabled_names();
    let available_list = Collectors::list();
    let enabled_refs: Vec<&str> = enabled_names.iter().map(|s| s.as_ref()).collect();
    let available_set: HashSet<&str> = available_list.iter().copied().collect();
    let missing: Vec<&str> = enabled_refs
        .iter()
        .filter(|&&name| !available_set.contains(name))
        .copied()
        .collect();

    if !missing.is_empty() {
        print_error!(
            "{}",
            style("Required collector(s) not available!").red().bold()
        );
        for name in missing {
            println!("  -> {}", style(name).red().bold());
        }
        print_collectors_table(&[], &available_list[..]);
        std::process::exit(1);
    }

    let readiness = Readiness::default();

    let (publisher, instance): (Arc<dyn blazebee::core::executor::Publisher>, _) = {
        #[cfg(feature = "blazebee-mqtt-v3")]
        {
            info!("MQTT transport enabled");
            info!("Starting MQTT client...");

            let manager = blazebee_mqtt_v3::MqttManager::from_config(cfg.transport.clone())
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

            let mqtt_publisher = blazebee_mqtt_v3::Publisher::with_config(
                Arc::new(instance.clone()),
                &ser_config,
                &cfg.transport.base_topic,
            );

            (
                Arc::new(mqtt_publisher) as Arc<dyn blazebee::core::executor::Publisher>,
                instance,
            )
        }

        #[cfg(not(feature = "blazebee-mqtt-v3"))]
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

            #[cfg(feature = "blazebee-mqtt-v3")]
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
