//! Message serialization, deserialization, and compression services.
//!
//! Handles encoding messages to bytes (serialization) and decoding bytes
//! back to messages (deserialization), with optional compression.
//!
//! Supports multiple formats:
//! - JSON (human-readable, larger)
//! - MessagePack (binary, compact)
//! - CBOR (binary, self-describing)
//!
//! # Examples
//!
//! ```ignore
//! let service = ConfigurableSerializationService::new(&config);
//!
//! // Serialize and optionally compress
//! let bytes = service.serialize_and_compress(&my_struct).await?;
//!
//! // Send via MQTT...
//!
//! // Later, deserialize
//! let value: MyStruct = service.deserialize(&bytes)?;
//! ```

use async_compression::tokio::write::{GzipEncoder, ZstdEncoder};
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::AsyncWriteExt;

use super::{
    config::{CompressionType, SerializationConfig, SerializationFormat},
    error::TransferError,
};

/// Trait for message serialization across multiple formats.
#[async_trait::async_trait]
pub trait MessageSerializer: Send + Sync {
    /// Serializes data to bytes.
    fn to_bytes<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, TransferError>;

    /// Deserializes bytes back to data.
    fn deserialize_bytes<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, TransferError>;

    /// Compresses data asynchronously.
    async fn compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError>;
}

/// JSON serializer using serde_json.
#[derive(Copy, Clone)]
pub struct JsonSerializer;

#[async_trait::async_trait]
impl MessageSerializer for JsonSerializer {
    fn to_bytes<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, TransferError> {
        serde_json::to_vec(data).map_err(|e| TransferError::Serialization(e.to_string()))
    }

    fn deserialize_bytes<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, TransferError> {
        serde_json::from_slice(data).map_err(|e| TransferError::Deserialization(e.to_string()))
    }

    async fn compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        let mut encoder = GzipEncoder::new(Vec::new());
        encoder.write_all(data).await?;
        encoder.shutdown().await?;
        Ok(encoder.into_inner())
    }
}

/// MessagePack serializer using rmp_serde.
#[derive(Copy, Clone)]
pub struct MessagePackSerializer;

#[async_trait::async_trait]
impl MessageSerializer for MessagePackSerializer {
    fn to_bytes<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, TransferError> {
        rmp_serde::to_vec(data).map_err(|e| TransferError::Serialization(e.to_string()))
    }

    fn deserialize_bytes<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, TransferError> {
        rmp_serde::from_slice(data).map_err(|e| TransferError::Deserialization(e.to_string()))
    }

    async fn compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        let mut encoder = ZstdEncoder::new(Vec::new());
        encoder.write_all(data).await?;
        encoder.shutdown().await?;
        Ok(encoder.into_inner())
    }
}

/// CBOR serializer using serde_cbor.
#[derive(Copy, Clone)]
pub struct CborSerializer;

#[async_trait::async_trait]
impl MessageSerializer for CborSerializer {
    fn to_bytes<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, TransferError> {
        serde_cbor::to_vec(data).map_err(|e| TransferError::Serialization(e.to_string()))
    }

    fn deserialize_bytes<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, TransferError> {
        serde_cbor::from_slice(data).map_err(|e| TransferError::Deserialization(e.to_string()))
    }

    async fn compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        let mut encoder = ZstdEncoder::new(Vec::new());
        encoder.write_all(data).await?;
        encoder.shutdown().await?;
        Ok(encoder.into_inner())
    }
}

/// Compression service for reducing message size.
#[derive(Clone)]
pub struct CompressionService {
    config: SerializationConfig,
}

impl CompressionService {
    pub fn new(config: &SerializationConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub async fn compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        if !self.config.should_compress(data.len()) {
            return Ok(data.to_vec());
        }

        match self.config.compression {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Gzip => self.gzip_compress(data).await,
            CompressionType::Zstd => self.zstd_compress(data).await,
        }
    }

    async fn gzip_compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        let mut encoder = GzipEncoder::new(Vec::new());
        encoder.write_all(data).await?;
        encoder.shutdown().await?;
        Ok(encoder.into_inner())
    }

    async fn zstd_compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        let mut encoder = ZstdEncoder::new(Vec::new());
        encoder.write_all(data).await?;
        encoder.shutdown().await?;
        Ok(encoder.into_inner())
    }
}

/// Type-erased serializer enum (object-safe alternative to trait objects).
#[derive(Clone)]
pub enum SerializerImpl {
    Json(JsonSerializer),
    MessagePack(MessagePackSerializer),
    Cbor(CborSerializer),
}

impl SerializerImpl {
    pub fn to_bytes<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, TransferError> {
        match self {
            SerializerImpl::Json(s) => s.to_bytes(data),
            SerializerImpl::MessagePack(s) => s.to_bytes(data),
            SerializerImpl::Cbor(s) => s.to_bytes(data),
        }
    }

    pub fn deserialize_bytes<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, TransferError> {
        match self {
            SerializerImpl::Json(s) => s.deserialize_bytes(data),
            SerializerImpl::MessagePack(s) => s.deserialize_bytes(data),
            SerializerImpl::Cbor(s) => s.deserialize_bytes(data),
        }
    }
}

pub struct SerializerFactory;

impl SerializerFactory {
    pub fn create(format: SerializationFormat) -> SerializerImpl {
        match format {
            SerializationFormat::Json => SerializerImpl::Json(JsonSerializer),
            SerializationFormat::MessagePack => SerializerImpl::MessagePack(MessagePackSerializer),
            SerializationFormat::Cbor => SerializerImpl::Cbor(CborSerializer),
        }
    }
}

/// Complete serialization service with format and compression.
#[derive(Clone)]
pub struct ConfigurableSerializationService {
    serializer: SerializerImpl,
    compression: CompressionService,
    config: SerializationConfig,
}

impl ConfigurableSerializationService {
    pub fn new(config: &SerializationConfig) -> Self {
        let serializer = SerializerFactory::create(config.format);
        let compression = CompressionService::new(config);

        Self {
            serializer,
            compression,
            config: config.clone(),
        }
    }

    pub fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, TransferError> {
        self.serializer.to_bytes(data)
    }

    pub async fn serialize_and_compress<T: Serialize>(
        &self,
        data: &T,
    ) -> Result<Vec<u8>, TransferError> {
        let serialized = self.serialize(data)?;
        self.compression.compress(&serialized).await
    }

    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, TransferError> {
        self.serializer.deserialize_bytes(data)
    }

    pub fn config(&self) -> &SerializationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_serializer() {
        let serializer = JsonSerializer;
        let data = serde_json::json!({"key": "value"});

        let bytes = serializer.to_bytes(&data).unwrap();
        assert!(!bytes.is_empty());

        let deserialized: serde_json::Value = serializer.deserialize_bytes(&bytes).unwrap();
        assert_eq!(deserialized["key"], "value");
    }

    #[tokio::test]
    async fn test_compression_service() {
        let config = SerializationConfig {
            compression: CompressionType::Gzip,
            compression_threshold: 100,
            ..Default::default()
        };
        let service = CompressionService::new(&config);

        let large_data = vec![b'a'; 1000];
        let compressed = service.compress(&large_data).await.unwrap();

        assert!(compressed.len() < large_data.len());
    }
}
