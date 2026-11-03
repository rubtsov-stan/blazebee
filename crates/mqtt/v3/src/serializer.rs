//! Payload encoding: serialization followed by optional compression.
//!
//! Compression runs synchronously on the in-memory buffer. The payloads are
//! small and already complete, so streaming through an async writer would
//! only add overhead.

use std::io::Write;

use serde::Serialize;

use crate::{
    config::{CompressionType, SerializationConfig, SerializationFormat},
    error::TransferError,
};

const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Clone)]
pub struct Serializer {
    format: SerializationFormat,
    compression: CompressionType,
    threshold: usize,
}

impl Serializer {
    pub fn new(config: &SerializationConfig) -> Self {
        Self {
            format: config.format,
            compression: config.compression,
            threshold: config.compression_threshold as usize,
        }
    }

    pub fn format(&self) -> SerializationFormat {
        self.format
    }

    pub fn compression(&self) -> CompressionType {
        self.compression
    }

    /// Encodes `value` into a new buffer. `capacity_hint` sizes the buffer up
    /// front (use the size of the previous payload for the same topic).
    pub fn encode<T: Serialize + ?Sized>(
        &self,
        value: &T,
        capacity_hint: usize,
    ) -> Result<Vec<u8>, TransferError> {
        let mut buf = Vec::with_capacity(capacity_hint);
        match self.format {
            SerializationFormat::Json => serde_json::to_writer(&mut buf, value)
                .map_err(|e| TransferError::Serialization(e.to_string()))?,
            SerializationFormat::MessagePack => rmp_serde::encode::write(&mut buf, value)
                .map_err(|e| TransferError::Serialization(e.to_string()))?,
            SerializationFormat::Cbor => ciborium::ser::into_writer(value, &mut buf)
                .map_err(|e| TransferError::Serialization(e.to_string()))?,
        }
        if self.compression == CompressionType::None || buf.len() < self.threshold {
            return Ok(buf);
        }
        self.compress(&buf)
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, TransferError> {
        match self.compression {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Gzip => {
                let out = Vec::with_capacity(data.len() / 2 + 32);
                let mut enc = flate2::write::GzEncoder::new(out, flate2::Compression::default());
                enc.write_all(data).map_err(TransferError::Compression)?;
                enc.finish().map_err(TransferError::Compression)
            }
            CompressionType::Zstd => {
                zstd::bulk::compress(data, ZSTD_LEVEL).map_err(TransferError::Compression)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sample {
        name: String,
        values: Vec<u64>,
    }

    fn sample() -> Sample {
        Sample {
            name: "cpu".into(),
            values: (0..400).map(|v| v * 1_000_003).collect(),
        }
    }

    fn serializer(
        format: SerializationFormat,
        compression: CompressionType,
        threshold: u32,
    ) -> Serializer {
        Serializer::new(&SerializationConfig {
            format,
            compression,
            compression_threshold: threshold,
        })
    }

    #[test]
    fn json_round_trip() {
        let bytes = serializer(SerializationFormat::Json, CompressionType::None, 0)
            .encode(&sample(), 0)
            .unwrap();
        assert_eq!(serde_json::from_slice::<Sample>(&bytes).unwrap(), sample());
    }

    #[test]
    fn msgpack_round_trip_is_positional() {
        let bytes = serializer(SerializationFormat::MessagePack, CompressionType::None, 0)
            .encode(&sample(), 0)
            .unwrap();
        assert_eq!(rmp_serde::from_slice::<Sample>(&bytes).unwrap(), sample());
        // Positional encoding: a 2-field struct is a fixarray of 2 elements.
        assert_eq!(bytes[0], 0x92);
    }

    #[test]
    fn cbor_round_trip() {
        let bytes = serializer(SerializationFormat::Cbor, CompressionType::None, 0)
            .encode(&sample(), 0)
            .unwrap();
        assert_eq!(
            ciborium::de::from_reader::<Sample, _>(&bytes[..]).unwrap(),
            sample()
        );
    }

    #[test]
    fn gzip_applied_above_threshold_only() {
        let s = serializer(SerializationFormat::Json, CompressionType::Gzip, 1024);
        let small = s
            .encode(
                &Sample {
                    name: "x".into(),
                    values: vec![],
                },
                0,
            )
            .unwrap();
        assert_eq!(small[0], b'{');
        let big = s.encode(&sample(), 0).unwrap();
        assert_eq!(&big[..2], &[0x1f, 0x8b], "gzip magic");
        let mut dec = flate2::read::GzDecoder::new(&big[..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        assert_eq!(serde_json::from_slice::<Sample>(&out).unwrap(), sample());
    }

    #[test]
    fn zstd_round_trip() {
        let s = serializer(SerializationFormat::Json, CompressionType::Zstd, 0);
        let bytes = s.encode(&sample(), 0).unwrap();
        assert_eq!(&bytes[..4], &[0x28, 0xb5, 0x2f, 0xfd], "zstd magic");
        let out = zstd::bulk::decompress(&bytes, 1 << 20).unwrap();
        assert_eq!(serde_json::from_slice::<Sample>(&out).unwrap(), sample());
    }

    #[test]
    fn serialization_errors_are_reported() {
        let s = serializer(SerializationFormat::Json, CompressionType::None, 0);
        let mut m = std::collections::HashMap::new();
        m.insert(vec![1u8], 1u8); // JSON requires string keys
        assert!(matches!(
            s.encode(&m, 0),
            Err(TransferError::Serialization(_))
        ));
    }
}
