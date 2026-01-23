use blazebee_mqtt_v3::{
    config::{CompressionType, Config, FramingConfig, SerializationConfig, SerializationFormat},
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

    info!("MQTT publish_framed Example");

    let mut cfg = Config::default();
    cfg.host = "localhost".to_string();
    cfg.port = 1883;
    cfg.base_topic = "base_topic".to_string();
    cfg.max_packet_size = Some(65_535);
    cfg.framing = FramingConfig { enabled: true };

    info!(
        "Config: host={}, port={}, base_topic={}, max_packet_size={}, framing.enabled={}",
        cfg.host,
        cfg.port,
        cfg.base_topic,
        cfg.max_packet_size.unwrap_or(0),
        cfg.framing.enabled
    );

    let manager = MqttManager::from_config(cfg)?;
    info!("Building and starting MQTT instance...");
    let instance = std::sync::Arc::new(manager.build_and_start().await?);

    info!("Starting monitoring (supervisor)...");
    instance.start_monitoring().await?;
    info!("Monitoring started");

    {
        info!("Preparing publisher config...");
        let config = SerializationConfig {
            format: SerializationFormat::MessagePack,
            compression: CompressionType::Zstd,
            compression_threshold: 0,
            ..Default::default()
        };

        info!(
            "Publisher SerializationConfig: format={:?}, compression={:?}, compression_threshold={}",
            config.format, config.compression, config.compression_threshold
        );

        let publisher = Publisher::with_config(instance.clone(), &config, instance.base_topic());
        info!("Publisher created (base_topic={})", instance.base_topic());

        info!("Preparing payload...");
        let values_count = 200_000usize;
        let data = DataPacket {
            id: 100,
            values: (0..values_count).map(|i| (i as f32) * 0.001).collect(),
            metadata: "large payload -> framed".into(),
        };

        info!(
            "DataPacket prepared: id={}, values_count={}, metadata_len={}",
            data.id,
            data.values.len(),
            data.metadata.len()
        );

        let meta = EndpointMetadata {
            qos: 1,
            topic: "data/framed".into(),
            retain: false,
        };

        info!(
            "EndpointMetadata: topic='{}', qos={}, retain={}",
            meta.topic, meta.qos, meta.retain
        );

        info!("Calling publish_framed()...");
        publisher.publish_framed(&data, &meta).await?;
        info!("publish_framed() completed successfully");
    }

    info!("Shutting down instance...");
    instance.shutdown().await?;
    info!("Shutdown completed");

    Ok(())
}
