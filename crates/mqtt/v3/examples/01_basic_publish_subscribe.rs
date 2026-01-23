use blazebee_mqtt_v3::{EndpointMetadata, MqttManager, Publisher};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tracing::{error, info};

#[derive(Serialize, Deserialize, Debug)]
struct SensorReading {
    temperature: f32,
    humidity: f32,
    timestamp: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting MQTT example application");

    // Create MQTT manager with minimal config
    let manager = MqttManager::new("localhost", 1883)?;
    info!("Manager created, building MQTT infrastructure...");

    // Build and start the MQTT connection
    let instance = std::sync::Arc::new(manager.build_and_start().await?);

    info!("MQTT instance started");

    // Create a publisher
    let publisher = Publisher::new(instance.clone());

    // Subscribe to a topic
    instance.subscribe("sensors/").await?;
    info!("Subscribed to sensors/");

    // Start state monitoring
    instance.start_monitoring().await?;
    info!("State monitoring started");

    // Spawn a task to publish sensor readings
    let publisher_task = {
        let publisher = publisher.clone();
        tokio::spawn(async move {
            let mut counter = 0u64;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                let reading = SensorReading {
                    temperature: 20.0 + (counter as f32 * 0.5),
                    humidity: 50.0 + ((counter as f32) % 20.0),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                let metadata = EndpointMetadata {
                    qos: 1,
                    topic: format!("sensors/living_room/data"),
                    retain: false,
                };

                match publisher.publish(&reading, &metadata).await {
                    Ok(()) => {
                        info!("Published sensor reading: {:?}", reading);
                        counter += 1;
                    }
                    Err(e) => {
                        error!("Failed to publish: {}", e);
                    }
                }
            }
        })
    };

    // Wait for Ctrl+C signal
    signal::ctrl_c().await?;
    info!("Received shutdown signal");

    // Graceful shutdown
    instance.shutdown().await?;
    publisher_task.abort();

    info!("Application shut down successfully");
    Ok(())
}
