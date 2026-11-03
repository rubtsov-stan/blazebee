//! Connects to a broker, publishes one JSON message and disconnects cleanly.
//!
//! ```bash
//! cargo run -p blazebee-mqtt-v3 --example publish -- localhost 1883
//! ```

use std::time::Duration;

use blazebee_mqtt_v3::{Config, EndpointMetadata, Transport};
use serde::Serialize;

#[derive(Serialize)]
struct Reading {
    temperature: f32,
    humidity: f32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();

    let mut args = std::env::args().skip(1);
    let config = Config {
        host: args.next().unwrap_or_else(|| "localhost".to_string()),
        port: args.next().and_then(|p| p.parse().ok()).unwrap_or(1883),
        client_id: "blazebee-example".to_string(),
        base_topic: "example".to_string(),
        clean_session: true,
        ..Config::default()
    };

    let transport = Transport::connect(&config)?;
    let mut state = transport.state();
    while !state.borrow().is_connected() {
        if let blazebee_mqtt_v3::ConnectionState::FatalExit(reason) = &*state.borrow() {
            return Err(reason.clone().into());
        }
        state.changed().await?;
    }

    let publisher = transport.publisher();
    let meta = EndpointMetadata {
        topic: "sensor/living_room".to_string(),
        qos: 1,
        retain: false,
    };
    let bytes = publisher.publish(
        &Reading {
            temperature: 22.5,
            humidity: 45.0,
        },
        &meta,
    )?;
    println!("queued {bytes} bytes for example/sensor/living_room");

    // Give the event loop a moment to flush the publish and its acknowledgement.
    tokio::time::sleep(Duration::from_millis(300)).await;
    transport.shutdown(Duration::from_secs(3)).await?;
    Ok(())
}
