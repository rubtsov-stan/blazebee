use blazebee_mqtt_v3::{
    config::{CompressionType, SerializationConfig, SerializationFormat},
    EndpointMetadata, MqttManager, Publisher,
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Serialize, Deserialize, Debug)]
struct DataPacket {
    id: u32,
    values: Vec<f32>,
    metadata: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("MQTT Serialization Formats Example");

    let manager = MqttManager::new("localhost", 1883)?;
    let instance = std::sync::Arc::new(manager.build_and_start().await?);

    instance.start_monitoring().await?;

    // Example 1: JSON (readable, larger)
    {
        info!("\n=== JSON Format (Default) ===");
        let config = SerializationConfig::default();
        let publisher = Publisher::with_config(instance.clone(), &config, "base_topic");

        let data = DataPacket {
            id: 1,
            values: vec![1.0, 2.0, 3.0, 4.0, 5.0],
            metadata: "test data".into(),
        };

        let metadata = EndpointMetadata {
            qos: 1,
            topic: "data/json".into(),
            retain: false,
        };

        publisher.publish(&data, &metadata).await?;
        info!("Published with JSON format");
    }

    // Example 2: MessagePack (compact binary)
    {
        info!("\n=== MessagePack Format (Compact) ===");
        let config = SerializationConfig {
            format: SerializationFormat::MessagePack,
            compression: CompressionType::None,
            ..Default::default()
        };
        let publisher = Publisher::with_config(instance.clone(), &config, "base_topic");

        let data = DataPacket {
            id: 2,
            values: vec![10.0, 20.0, 30.0, 40.0, 50.0],
            metadata: "messagepack data".into(),
        };

        let metadata = EndpointMetadata {
            qos: 1,
            topic: "data/msgpack".into(),
            retain: false,
        };

        publisher.publish(&data, &metadata).await?;
        info!("Published with MessagePack format");
    }

    // Example 3: CBOR (self-describing)
    {
        info!("\n=== CBOR Format (Self-Describing) ===");
        let config = SerializationConfig {
            format: SerializationFormat::Cbor,
            compression: CompressionType::None,
            ..Default::default()
        };
        let publisher = Publisher::with_config(instance.clone(), &config, "base_topic");

        let data = DataPacket {
            id: 3,
            values: vec![100.0, 200.0, 300.0],
            metadata: "cbor format".into(),
        };

        let metadata = EndpointMetadata {
            qos: 1,
            topic: "data/cbor".into(),
            retain: false,
        };

        publisher.publish(&data, &metadata).await?;
        info!("Published with CBOR format");
    }

    // Example 4: JSON with Gzip compression
    {
        info!("\n=== JSON with Gzip Compression ===");
        let config = SerializationConfig {
            preset: Some("compact".to_string()),
            format: SerializationFormat::Json,
            compression: CompressionType::Gzip,
            compression_threshold: 256,
        };
        let publisher = Publisher::with_config(instance.clone(), &config, "base_topic");

        let data = DataPacket {
            id: 4,
            values: vec![1.1, 2.2, 3.3, 4.4, 5.5, 6.6, 7.7, 8.8, 9.9],
            metadata: "gzip compressed data for bandwidth optimization".into(),
        };

        let metadata = EndpointMetadata {
            qos: 1,
            topic: "data/gzip".into(),
            retain: false,
        };

        publisher.publish(&data, &metadata).await?;
        info!("Published with JSON + Gzip compression");
    }

    // Example 5: MessagePack with Zstd (high performance)
    {
        info!("\n=== MessagePack with Zstd (High Performance) ===");
        let config = SerializationConfig {
            preset: Some("high_performance".to_string()),
            format: SerializationFormat::MessagePack,
            compression: CompressionType::Zstd,
            compression_threshold: 512,
        };
        let publisher = Publisher::with_config(instance.clone(), &config, "base_topic");

        let data = DataPacket {
            id: 5,
            values: (1..=100).map(|i| i as f32).collect(),
            metadata: "high performance configuration for real-time data".into(),
        };

        let metadata = EndpointMetadata {
            qos: 1,
            topic: "data/zstd".into(),
            retain: false,
        };

        publisher.publish(&data, &metadata).await?;
        info!("Published with MessagePack + Zstd compression");
    }

    // Example 6: Using Publisher presets
    {
        info!("\n=== Using Publisher Presets ===");

        // High performance preset
        let publisher_hp = blazebee_mqtt_v3::publisher::presets::high_performance(instance.clone());
        let data = DataPacket {
            id: 6,
            values: vec![42.0],
            metadata: "preset".into(),
        };

        let metadata = EndpointMetadata {
            qos: 1,
            topic: "data/preset".into(),
            retain: false,
        };

        publisher_hp.publish(&data, &metadata).await?;
        info!("Published using high_performance preset");
    }

    info!("\n=== All examples completed ===");

    instance.shutdown().await?;

    Ok(())
}
