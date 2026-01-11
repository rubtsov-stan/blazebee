//! Message publishing with serialization, compression, and base_topic support.
//!
//! The `Publisher` provides a high-level interface for sending messages to MQTT topics.
//! It handles:
//! - Serialization (JSON, MessagePack, CBOR)
//! - Compression (Gzip, Zstd)
//! - Base topic prefix application
//! - Error handling
//! - Metadata validation
//!
//! # Base Topic Integration
//!
//! The publisher automatically applies the `base_topic` from the MqttInstance
//! configuration to all publication topics. For example, with `base_topic = "myapp"`:
//! - Topic "sensor/temperature" becomes "myapp/sensor/temperature"
//! - Topic "/status" becomes "myapp/status"
//!
//! # Examples
//!
//! ```ignore
//! use mqtt_manager::{Publisher, config::EndpointMetadata};
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Sensor { temperature: f32 }
//!
//! let publisher = Publisher::new(instance);
//! let sensor = Sensor { temperature: 22.5 };
//! let metadata = EndpointMetadata {
//!     qos: 1,
//!     topic: "sensors/living_room".into(),
//!     retain: true,
//! };
//!
//! ```

use std::sync::Arc;

use bytes::Bytes;
use futures_util::StreamExt;
use rumqttc::QoS;
use serde::Serialize;
use tracing::debug;

use super::{
    config::{CompressionType, EndpointMetadata, SerializationConfig, SerializationFormat},
    framer::{FrameHeader, FrameParts, Framer},
    manager::{MqttInstance, PublishDrain},
    message::ConfigurableSerializationService,
    TransferError,
};

/// High-level interface for publishing MQTT messages with base_topic support.
///
/// The publisher serializes data, applies optional compression, adds base_topic
/// prefix, and sends to the broker. It can be cloned and shared across tasks
/// (all fields are wrapped in Arc for thread-safety).
#[derive(Clone)]
pub struct Publisher {
    /// Reference to the MQTT instance
    instance: Arc<MqttInstance>,

    /// Serialization and compression service
    serialization: Arc<ConfigurableSerializationService>,

    /// Base topic prefix (cached from instance)
    base_topic: String,

    /// Optional framer (present only if enabled in config)
    framer: Option<Framer>,

    /// Publish drain barrier (delays connection shutdown until publishes complete)
    publish_drain: Arc<PublishDrain>,
}

impl Publisher {
    /// Creates a publisher with default serialization settings.
    ///
    /// Default: JSON format, Gzip compression, uses base_topic from instance.
    ///
    /// # Arguments
    /// - `instance`: Reference to the active MQTT instance
    ///
    /// # Examples
    /// ```ignore
    /// let publisher = Publisher::new(instance);
    /// ```
    pub fn new(instance: Arc<MqttInstance>) -> Self {
        let base_topic = instance.base_topic().to_string();
        Self::with_config(instance, &SerializationConfig::default(), &base_topic)
    }

    /// Creates a publisher with custom serialization configuration.
    ///
    /// # Arguments
    /// - `instance`: Reference to the active MQTT instance
    /// - `config`: Serialization and compression settings
    /// - `base_topic`: Base topic prefix (usually from instance.config())
    ///
    /// # Examples
    /// ```ignore
    /// let config = SerializationConfig {
    ///     format: SerializationFormat::MessagePack,
    ///     compression: CompressionType::Zstd,
    ///     compression_threshold: 512,
    /// };
    /// let publisher = Publisher::with_config(instance, &config, "myapp");
    /// ```
    pub fn with_config(
        instance: Arc<MqttInstance>,
        config: &SerializationConfig,
        base_topic: &str,
    ) -> Self {
        let serialization = Arc::new(ConfigurableSerializationService::new(config));
        let publish_drain = instance.publish_drain();
        let framer = if instance.framing_enabled() {
            Some(Framer::new(instance.max_packet_size()))
        } else {
            None
        };
        Self {
            instance,
            serialization,
            base_topic: base_topic.to_string(),
            framer,
            publish_drain,
        }
    }

    /// Gets the serialization configuration.
    pub fn config(&self) -> &SerializationConfig {
        self.serialization.config()
    }

    /// Gets a reference to the serialization service.
    ///
    /// Useful for manual serialization operations outside of publishing.
    pub fn serialization(&self) -> &ConfigurableSerializationService {
        &self.serialization
    }

    /// Gets the base topic prefix.
    ///
    /// # Returns
    /// - `&str`: The base topic prefix
    pub fn base_topic(&self) -> &str {
        &self.base_topic
    }

    /// Applies base_topic to a given topic.
    ///
    /// # Arguments
    /// - `topic`: Topic without base_topic prefix
    ///
    /// # Returns
    /// - `String`: Full topic with base_topic prefix
    fn with_base_topic(&self, topic: &str) -> String {
        if self.base_topic.is_empty() {
            topic.to_string()
        } else {
            format!(
                "{}/{}",
                self.base_topic.trim_end_matches('/'),
                topic.trim_start_matches('/')
            )
        }
    }

    /// Publishes a message to the specified topic (with base_topic applied).
    ///
    /// # Arguments
    /// - `data`: Message to serialize and send
    /// - `metadata`: Topic (without base_topic), QoS, retain settings
    ///
    /// # Returns
    /// - `Ok(())`: Message sent successfully
    /// - `Err(TransferError)`: Serialization or network error
    ///
    /// # Serialization Process
    /// 1. Serialize to format (JSON/MessagePack/CBOR)
    /// 2. If size >= threshold and compression enabled: compress
    /// 3. Apply base_topic prefix
    /// 4. Send to broker with QoS and retain settings
    ///
    /// # Examples
    /// ```ignore
    /// let metadata = EndpointMetadata {
    ///     qos: 1,
    ///     topic: "device/status".into(), // Without base_topic
    ///     retain: true,
    /// };
    /// // With base_topic = "myapp", publishes to "myapp/device/status"
    /// publisher.publish(&my_data, &metadata).await?;
    /// ```
    pub async fn publish<T: Serialize + Send + Sync>(
        &self,
        data: T,
        metadata: &EndpointMetadata,
    ) -> Result<(), TransferError> {
        let _guard = self.publish_drain.enter();
        // Serialize and optionally compress
        let payload = self
            .serialization
            .serialize_and_compress(&data)
            .await
            .map_err(|e| TransferError::Serialization(e.to_string()))?;

        // Apply base_topic to topic
        let full_topic = self.with_base_topic(&metadata.topic);

        debug!(
            "Publishing to topic '{}' (full: '{}'): {} bytes (compressed: {})",
            metadata.topic,
            full_topic,
            payload.len(),
            self.serialization.config().should_compress(payload.len()),
        );

        // Convert u8 QoS to rumqttc QoS enum
        let qos = match metadata.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => {
                return Err(TransferError::InvalidMetadata(
                    "Invalid QoS value".to_string(),
                ));
            }
        };

        // Send to broker with full topic
        self.instance
            .client()
            .publish(full_topic, qos, metadata.retain, payload)
            .await?;

        Ok(())
    }

    /// Publishes a message to a specific topic without using EndpointMetadata.
    ///
    /// Convenience method for simple publishing scenarios.
    ///
    /// # Arguments
    /// - `data`: Message to serialize and send
    /// - `topic`: Topic without base_topic prefix
    /// - `qos`: Quality of Service level
    /// - `retain`: Whether broker should retain the message
    ///
    /// # Returns
    /// - `Ok(())`: Message sent successfully
    /// - `Err(TransferError)`: Serialization or network error
    ///
    /// # Examples
    /// ```ignore
    /// // With base_topic = "myapp"
    /// publisher.publish_to("sensor/temp", &data, 1, false).await?;
    /// // Publishes to "myapp/sensor/temp"
    /// ```
    pub async fn publish_to<T: Serialize + Send + Sync>(
        &self,
        topic: &str,
        data: T,
        qos: u8,
        retain: bool,
    ) -> Result<(), TransferError> {
        let metadata = EndpointMetadata {
            topic: topic.to_string(),
            qos,
            retain,
        };
        self.publish(data, &metadata).await
    }

    fn mqtt_publish_overhead(full_topic: &str, qos: QoS) -> usize {
        // MQTT PUBLISH (v3.1.1) overhead (worst-case Remaining Length encoding = 4 bytes):
        // fixed header: 1 byte (packet type/flags) + up to 4 bytes (remaining length)
        // topic name: 2 bytes length + topic bytes
        // packet identifier: 2 bytes if QoS>0
        let fixed_header = 1 + 4;
        let topic = 2 + full_topic.as_bytes().len();
        let pid = match qos {
            QoS::AtMostOnce => 0,
            QoS::AtLeastOnce | QoS::ExactlyOnce => 2,
        };
        fixed_header + topic + pid
    }

    pub async fn publish_framed<T: Serialize + Send + Sync>(
        &self,
        data: T,
        metadata: &EndpointMetadata,
    ) -> Result<(), TransferError> {
        let Some(framer) = &self.framer else {
            return self.publish(data, metadata).await;
        };

        let _guard = self.publish_drain.enter();

        let payload = self.serialization.serialize_and_compress(&data).await?;
        let payload = Bytes::from(payload);

        let full_topic = self.with_base_topic(&metadata.topic);

        let qos = match metadata.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => {
                return Err(TransferError::InvalidMetadata(
                    "Invalid QoS value".to_string(),
                ));
            }
        };

        let broker_max_packet = self.instance.max_packet_size();

        let overhead = Self::mqtt_publish_overhead(&full_topic, qos);
        if broker_max_packet <= overhead {
            return Err(TransferError::InvalidMetadata(
                "max_packet_size is too small for MQTT overhead".to_string(),
            ));
        }
        let max_mqtt_payload = broker_max_packet - overhead;

        if max_mqtt_payload < FrameHeader::LEN {
            return Err(TransferError::InvalidMetadata(
                "max_packet_size is too small for framing header".to_string(),
            ));
        }

        let stream = framer.frames(
            payload,
            self.serialization.config().format,
            metadata.qos,
            max_mqtt_payload,
        );
        tokio::pin!(stream);

        while let Some(item) = stream.next().await {
            let FrameParts { header, chunk } = item?;

            let mut out = Vec::with_capacity(header.len() + chunk.len());
            out.extend_from_slice(&header);
            out.extend_from_slice(&chunk);

            self.instance
                .client()
                .publish(full_topic.clone(), qos, metadata.retain, out)
                .await?;
        }

        Ok(())
    }
}

/// Preset publisher configurations for common scenarios.
///
/// All presets automatically use the base_topic from the MqttInstance.
pub mod presets {
    use super::*;

    /// Minimal setup: JSON, no compression.
    ///
    /// Use for:
    /// - Development and debugging
    /// - Small messages
    /// - Human-readable output
    ///
    /// # Examples
    /// ```ignore
    /// let publisher = presets::minimal(instance);
    /// ```
    pub fn minimal(instance: Arc<MqttInstance>) -> Publisher {
        let base_topic = instance.base_topic().to_string();
        Publisher::with_config(
            instance,
            &SerializationConfig {
                format: SerializationFormat::Json,
                compression: CompressionType::None,
                compression_threshold: 1024,
                ..Default::default()
            },
            &base_topic,
        )
    }

    /// Compact setup: MessagePack, no compression.
    ///
    /// Use for:
    /// - Bandwidth-constrained scenarios
    /// - When size matters but not debugging
    /// - Mobile/cellular networks
    ///
    /// # Examples
    /// ```ignore
    /// let publisher = presets::compact(instance);
    /// ```
    pub fn compact(instance: Arc<MqttInstance>) -> Publisher {
        let base_topic = instance.base_topic().to_string();
        Publisher::with_config(
            instance,
            &SerializationConfig {
                format: SerializationFormat::MessagePack,
                compression: CompressionType::None,
                compression_threshold: 1024,
                ..Default::default()
            },
            &base_topic,
        )
    }

    /// Efficient setup: JSON with Gzip compression.
    ///
    /// Use for:
    /// - Balancing readability and size
    /// - Good compression-to-speed ratio
    /// - When debuggability matters somewhat
    ///
    /// # Examples
    /// ```ignore
    /// let publisher = presets::efficient(instance);
    /// ```
    pub fn efficient(instance: Arc<MqttInstance>) -> Publisher {
        let base_topic = instance.base_topic().to_string();
        Publisher::with_config(
            instance,
            &SerializationConfig {
                format: SerializationFormat::Json,
                compression: CompressionType::Gzip,
                compression_threshold: 1024,
                ..Default::default()
            },
            &base_topic,
        )
    }

    /// High-performance setup: MessagePack with Zstd.
    ///
    /// Use for:
    /// - High-throughput scenarios
    /// - CPU resources available
    /// - Bandwidth-critical scenarios
    ///
    /// # Examples
    /// ```ignore
    /// let publisher = presets::high_performance(instance);
    /// ```
    pub fn high_performance(instance: Arc<MqttInstance>) -> Publisher {
        let base_topic = instance.base_topic().to_string();
        Publisher::with_config(
            instance,
            &SerializationConfig {
                format: SerializationFormat::MessagePack,
                compression: CompressionType::Zstd,
                compression_threshold: 512,
                ..Default::default()
            },
            &base_topic,
        )
    }

    /// Ultra-compact setup: CBOR with Zstd.
    ///
    /// Use for:
    /// - Maximum compression
    /// - Interoperability with CBOR systems
    /// - Extreme bandwidth constraints
    ///
    /// # Examples
    /// ```ignore
    /// let publisher = presets::ultra_compact(instance);
    /// ```
    pub fn ultra_compact(instance: Arc<MqttInstance>) -> Publisher {
        let base_topic = instance.base_topic().to_string();
        Publisher::with_config(
            instance,
            &SerializationConfig {
                format: SerializationFormat::Cbor,
                compression: CompressionType::Zstd,
                compression_threshold: 256,
                ..Default::default()
            },
            &base_topic,
        )
    }
}
