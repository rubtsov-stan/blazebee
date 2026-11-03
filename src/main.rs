use std::{process::ExitCode, time::Duration};

use blazebee::{
    config::Config,
    core::{collectors, scheduler::Scheduler},
    logger,
};
use blazebee_mqtt_v3::Transport;
use tracing::{error, info};

/// Time allowed for the final `offline` status and DISCONNECT to reach the broker.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("blazebee: {err}");
            return ExitCode::from(2);
        }
    };
    if let Err(err) = logger::init(&config.logger) {
        eprintln!("blazebee: {err}");
        return ExitCode::from(2);
    }

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            error!(%err, "failed to start runtime");
            return ExitCode::from(1);
        }
    };
    runtime.block_on(run(config))
}

async fn run(config: Config) -> ExitCode {
    info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %config.source().display(),
        collectors = ?config.metrics.enabled_names(),
        interval_s = config.metrics.collection_interval,
        "starting blazebee"
    );

    let transport = match Transport::connect(&config.transport) {
        Ok(transport) => transport,
        Err(err) => {
            error!(%err, "cannot start MQTT transport");
            return ExitCode::from(1);
        }
    };

    let entries = config
        .metrics
        .collectors
        .enabled
        .iter()
        .map(|entry| {
            // Names were validated against the registry while loading the configuration.
            let descriptor = collectors::find(&entry.name).expect("validated collector name");
            (
                descriptor.name,
                (descriptor.build)(),
                entry.metadata.clone(),
            )
        })
        .collect();
    let scheduler = Scheduler::new(
        Duration::from_secs(config.metrics.collection_interval),
        entries,
        transport.publisher(),
        transport.state(),
    );

    let exit = tokio::select! {
        biased;
        signal = shutdown_signal() => {
            info!(signal, "shutting down");
            ExitCode::SUCCESS
        }
        _ = scheduler.run() => {
            error!("collection stopped because the broker connection failed permanently");
            ExitCode::from(1)
        }
    };

    if let Err(err) = transport.shutdown(SHUTDOWN_TIMEOUT).await {
        if exit == ExitCode::SUCCESS {
            error!(%err, "MQTT shutdown failed");
        }
    }
    info!("stopped");
    exit
}

#[cfg(unix)]
async fn shutdown_signal() -> &'static str {
    use tokio::signal::unix::{signal, SignalKind};

    let mut term = match signal(SignalKind::terminate()) {
        Ok(term) => term,
        Err(err) => {
            error!(%err, "cannot install SIGTERM handler; only SIGINT will stop the process");
            tokio::signal::ctrl_c().await.ok();
            return "SIGINT";
        }
    };
    tokio::select! {
        _ = term.recv() => "SIGTERM",
        _ = tokio::signal::ctrl_c() => "SIGINT",
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> &'static str {
    tokio::signal::ctrl_c().await.ok();
    "SIGINT"
}
