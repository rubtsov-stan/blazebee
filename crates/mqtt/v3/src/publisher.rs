//! Encodes values and hands them to the event loop without blocking.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use rumqttc::{AsyncClient, QoS};
use serde::Serialize;
use tracing::trace;

use crate::{
    config::{Config, EndpointMetadata, SerializationFormat},
    error::TransferError,
    framer::{self, FRAME_HEADER_LEN},
    serializer::Serializer,
};

pub struct Publisher {
    client: AsyncClient,
    base_topic: String,
    serializer: Serializer,
    framing: bool,
    max_packet_size: usize,
    next_stream_id: AtomicU64,
    /// Largest payload produced so far; sizes the encode buffer.
    size_hint: AtomicUsize,
}

impl Publisher {
    pub(crate) fn new(client: AsyncClient, config: &Config) -> Self {
        Self {
            client,
            base_topic: config.base_topic.clone(),
            serializer: Serializer::new(&config.serialization),
            framing: config.framing.enabled,
            max_packet_size: config.max_packet_size as usize,
            next_stream_id: AtomicU64::new(1),
            size_hint: AtomicUsize::new(256),
        }
    }

    pub fn base_topic(&self) -> &str {
        &self.base_topic
    }

    pub fn format(&self) -> SerializationFormat {
        self.serializer.format()
    }

    pub fn serializer(&self) -> &Serializer {
        &self.serializer
    }

    /// Encodes `value` and queues it for `base_topic/meta.topic`.
    /// Returns the number of payload bytes queued. Never waits: a full queue
    /// is reported as [`TransferError::QueueFull`].
    pub fn publish<T: Serialize + ?Sized>(
        &self,
        value: &T,
        meta: &EndpointMetadata,
    ) -> Result<usize, TransferError> {
        let qos = match meta.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            other => return Err(TransferError::InvalidQos(other)),
        };
        let topic = join_topic(&self.base_topic, &meta.topic);
        let payload = self
            .serializer
            .encode(value, self.size_hint.load(Ordering::Relaxed))?;
        self.size_hint.fetch_max(payload.len(), Ordering::Relaxed);
        let max_payload = self
            .max_packet_size
            .checked_sub(publish_overhead(&topic, qos))
            .ok_or(TransferError::PayloadTooLarge {
                size: payload.len(),
                max: self.max_packet_size,
            })?;
        trace!(topic = %topic, bytes = payload.len(), "publishing");

        if !self.framing {
            if payload.len() > max_payload {
                return Err(TransferError::PayloadTooLarge {
                    size: payload.len(),
                    max: self.max_packet_size,
                });
            }
            let len = payload.len();
            self.client.try_publish(topic, qos, meta.retain, payload)?;
            return Ok(len);
        }

        if max_payload < FRAME_HEADER_LEN {
            return Err(TransferError::PayloadTooLarge {
                size: payload.len(),
                max: self.max_packet_size,
            });
        }
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::Relaxed);
        let frames = framer::frames(
            &payload,
            self.serializer.format(),
            meta.qos,
            stream_id,
            max_payload,
        )?;
        for frame in frames {
            self.client
                .try_publish(topic.clone(), qos, meta.retain, frame)?;
        }
        Ok(payload.len())
    }
}

/// `base/topic` with duplicate slashes at the join removed; an empty base
/// yields the topic unchanged (minus a leading slash).
pub(crate) fn join_topic(base: &str, topic: &str) -> String {
    let topic = topic.trim_start_matches('/');
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        topic.to_string()
    } else {
        let mut out = String::with_capacity(base.len() + 1 + topic.len());
        out.push_str(base);
        out.push('/');
        out.push_str(topic);
        out
    }
}

/// Worst-case MQTT 3.1.1 PUBLISH overhead: fixed header (1 + up to 4 bytes),
/// topic length prefix and topic, packet identifier for QoS 1/2.
fn publish_overhead(topic: &str, qos: QoS) -> usize {
    let pid = match qos {
        QoS::AtMostOnce => 0,
        QoS::AtLeastOnce | QoS::ExactlyOnce => 2,
    };
    1 + 4 + 2 + topic.len() + pid
}

#[cfg(test)]
mod tests {
    use rumqttc::Request;
    use serde::Serialize;

    use super::*;
    use crate::config::{CompressionType, FramingConfig, SerializationConfig};

    #[derive(Serialize)]
    struct Sample {
        v: Vec<u8>,
    }

    fn config(max_packet_size: u32, framing: bool) -> Config {
        Config {
            client_id: "t".into(),
            base_topic: "base".into(),
            max_packet_size,
            request_channel_capacity: 4,
            framing: FramingConfig { enabled: framing },
            serialization: SerializationConfig {
                format: SerializationFormat::Json,
                compression: CompressionType::None,
                compression_threshold: 0,
            },
            ..Config::default()
        }
    }

    fn publisher(config: &Config) -> (Publisher, flume::Receiver<Request>) {
        let (tx, rx) = flume::bounded(config.request_channel_capacity as usize);
        (Publisher::new(AsyncClient::from_senders(tx), config), rx)
    }

    fn drain(rx: &flume::Receiver<Request>) -> Vec<rumqttc::Publish> {
        let mut out = Vec::new();
        while let Ok(req) = rx.try_recv() {
            if let Request::Publish(p) = req {
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn join_topic_rules() {
        assert_eq!(join_topic("base", "metrics/cpu"), "base/metrics/cpu");
        assert_eq!(join_topic("base/", "/metrics/cpu"), "base/metrics/cpu");
        assert_eq!(join_topic("", "/metrics/cpu"), "metrics/cpu");
        assert_eq!(join_topic("", "metrics/cpu"), "metrics/cpu");
    }

    #[test]
    fn publishes_with_base_topic_qos_and_retain() {
        let cfg = config(1024, false);
        let (publisher, eventloop) = publisher(&cfg);
        let meta = EndpointMetadata {
            topic: "metrics/cpu".into(),
            qos: 1,
            retain: true,
        };
        let n = publisher
            .publish(&Sample { v: vec![1, 2, 3] }, &meta)
            .unwrap();
        let sent = drain(&eventloop);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].topic, "base/metrics/cpu");
        assert_eq!(sent[0].qos, QoS::AtLeastOnce);
        assert!(sent[0].retain);
        assert_eq!(&sent[0].payload[..], br#"{"v":[1,2,3]}"#);
        assert_eq!(n, sent[0].payload.len());
    }

    #[test]
    fn oversized_payload_is_rejected_not_queued() {
        let cfg = config(64, false);
        let (publisher, eventloop) = publisher(&cfg);
        let meta = EndpointMetadata {
            topic: "metrics/cpu".into(),
            qos: 0,
            retain: false,
        };
        let err = publisher
            .publish(&Sample { v: vec![7; 100] }, &meta)
            .unwrap_err();
        assert!(
            matches!(err, TransferError::PayloadTooLarge { max: 64, .. }),
            "{err}"
        );
        assert!(drain(&eventloop).is_empty());
    }

    #[test]
    fn full_queue_is_reported() {
        let cfg = config(1024, false);
        let (publisher, eventloop) = publisher(&cfg);
        let meta = EndpointMetadata {
            topic: "t".into(),
            qos: 0,
            retain: false,
        };
        for _ in 0..4 {
            publisher.publish(&Sample { v: vec![] }, &meta).unwrap();
        }
        assert!(matches!(
            publisher.publish(&Sample { v: vec![] }, &meta),
            Err(TransferError::QueueFull)
        ));
        assert_eq!(drain(&eventloop).len(), 4);
    }

    #[test]
    fn invalid_qos_is_rejected() {
        let cfg = config(1024, false);
        let (publisher, _eventloop) = publisher(&cfg);
        let meta = EndpointMetadata {
            topic: "t".into(),
            qos: 3,
            retain: false,
        };
        assert!(matches!(
            publisher.publish(&Sample { v: vec![] }, &meta),
            Err(TransferError::InvalidQos(3))
        ));
    }

    #[test]
    fn framing_splits_large_payloads() {
        // overhead for topic "base/t" at QoS 0 is 1+4+2+6 = 13; frames of 64-13 = 51 bytes
        let cfg = config(64, true);
        let (publisher, eventloop) = publisher(&cfg);
        let meta = EndpointMetadata {
            topic: "t".into(),
            qos: 0,
            retain: false,
        };
        let sample = Sample { v: vec![9; 40] };
        let expected = serde_json::to_vec(&sample).unwrap();
        publisher.publish(&sample, &meta).unwrap();
        let sent = drain(&eventloop);
        assert!(sent.len() > 1);
        let mut joined = Vec::new();
        for (i, p) in sent.iter().enumerate() {
            assert!(p.payload.len() <= 51);
            let header = crate::framer::FrameHeader::decode(&p.payload).unwrap();
            assert_eq!(header.frame_index as usize, i);
            assert_eq!(header.frame_total as usize, sent.len());
            joined.extend_from_slice(&p.payload[FRAME_HEADER_LEN..]);
        }
        assert_eq!(joined, expected);
    }
}
