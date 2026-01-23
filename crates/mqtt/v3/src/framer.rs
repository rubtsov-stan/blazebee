// framer.rs

//! Framing for already serialized + (optionally) compressed payload bytes.
//!
//! Requirements implemented:
//! 1) Input is already prepared bytes (no compression inside Framer).
//! 2) One frame payload (header + chunk) MUST be <= max_package_size, else error.
//! 3) Chunking uses zero-copy slices of the original Bytes (`Bytes::slice`).

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use async_stream::try_stream;
use bytes::Bytes;
use futures_core::Stream;

use super::{config::SerializationFormat, TransferError};

const FRAME_MAGIC: [u8; 2] = *b"FR";
const FRAME_VERSION: u8 = 1;

/// Fixed-size frame header used for reassembly and ordering.
///
/// Encoding (big-endian):
/// - magic: [u8;2]      = "FR"
/// - version: u8        = 1
/// - format: u8         = SerializationFormat as u8
/// - qos: u8            = MQTT QoS used for frames (0/1/2)
/// - stream_id: u64
/// - frame_idx: u32     = 0..frame_total-1
/// - frame_total: u32   = total number of frames in stream
#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    pub format: SerializationFormat,
    pub qos: u8,
    pub stream_id: u64,
    pub frame_idx: u32,
    pub frame_total: u32,
}

impl FrameHeader {
    pub const LEN: usize = 2 + 1 + 1 + 1 + 8 + 4 + 4;

    pub fn encode(self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];

        out[0] = FRAME_MAGIC[0];
        out[1] = FRAME_MAGIC[1];
        out[2] = FRAME_VERSION;
        out[3] = self.format as u8;
        out[4] = self.qos;

        out[5..13].copy_from_slice(&self.stream_id.to_be_bytes());
        out[13..17].copy_from_slice(&self.frame_idx.to_be_bytes());
        out[17..21].copy_from_slice(&self.frame_total.to_be_bytes());

        out
    }
}

/// Output item: header (owned small array) + zero-copy slice of the input payload.
#[derive(Debug, Clone)]
pub struct FrameParts {
    pub header: [u8; FrameHeader::LEN],
    pub chunk: Bytes,
}

#[derive(Debug, Clone)]
pub struct Framer {
    next_stream_id: Arc<AtomicU64>,
    max_package_size: usize,
}

impl Framer {
    pub fn new(max_package_size: usize) -> Self {
        Self {
            next_stream_id: Arc::new(AtomicU64::new(1)),
            max_package_size,
        }
    }

    pub fn max_package_size(&self) -> usize {
        self.max_package_size
    }

    /// Splits `payload` into ordered frames.
    ///
    /// Errors if:
    /// - max_package_size < FrameHeader::LEN
    /// - any produced frame would exceed max_package_size
    pub fn frames(
        &self,
        payload: Bytes,
        format: SerializationFormat,
        qos: u8,
        max_frame_payload: usize,
    ) -> impl Stream<Item = Result<FrameParts, TransferError>> + '_ {
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::Relaxed);

        try_stream! {
            if max_frame_payload < FrameHeader::LEN {
                Err(TransferError::InvalidMetadata(
                    "max_frame_payload is smaller than FrameHeader::LEN".to_string(),
                ))?;
            }

            let chunk_max = max_frame_payload - FrameHeader::LEN;

            if !payload.is_empty() && chunk_max == 0 {
                Err(TransferError::InvalidMetadata(
                    "max_frame_payload leaves no room for payload".to_string(),
                ))?;
            }

            let total_frames: u32 = if payload.is_empty() {
                1
            } else {
                ((payload.len() + chunk_max - 1) / chunk_max) as u32
            };

            if payload.is_empty() {
                let header = FrameHeader {
                    format,
                    qos,
                    stream_id,
                    frame_idx: 0,
                    frame_total: 1,
                }.encode();

                yield FrameParts { header, chunk: Bytes::new() };
                return;
            }

            for idx in 0..total_frames {
                let start = (idx as usize) * chunk_max;
                let end = std::cmp::min(start + chunk_max, payload.len());
                let chunk = payload.slice(start..end);

                let header = FrameHeader {
                    format,
                    qos,
                    stream_id,
                    frame_idx: idx,
                    frame_total: total_frames,
                }.encode();

                let frame_len = FrameHeader::LEN + chunk.len();
                if frame_len > max_frame_payload {
                    Err(TransferError::InvalidMetadata(
                        "Frame size exceeds max_frame_payload".to_string(),
                    ))?;
                }

                yield FrameParts { header, chunk };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_util::{pin_mut, StreamExt};

    use super::*;

    fn parse_u64_be(b: &[u8]) -> u64 {
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        u64::from_be_bytes(arr)
    }

    fn parse_u32_be(b: &[u8]) -> u32 {
        let mut arr = [0u8; 4];
        arr.copy_from_slice(b);
        u32::from_be_bytes(arr)
    }

    #[tokio::test]
    async fn frames_empty_payload_single_frame() {
        let framer = Framer::new(65_535);
        let payload = Bytes::new();

        let s = framer.frames(payload, SerializationFormat::Json, 1, FrameHeader::LEN);
        pin_mut!(s);

        let mut out = Vec::new();
        while let Some(item) = s.next().await {
            out.push(item.unwrap());
        }

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].chunk.len(), 0);

        // header sanity
        let h = out[0].header;
        assert_eq!(&h[0..2], b"FR");
        assert_eq!(h[2], 1); // version
        assert_eq!(h[3], SerializationFormat::Json as u8);
        assert_eq!(h[4], 1); // qos
        assert_eq!(parse_u32_be(&h[13..17]), 0); // idx
        assert_eq!(parse_u32_be(&h[17..21]), 1); // total
    }

    #[tokio::test]
    async fn frames_errors_when_max_smaller_than_header() {
        let framer = Framer::new(65_535);
        let payload = Bytes::from_static(b"abc");

        let s = framer.frames(payload, SerializationFormat::Json, 0, FrameHeader::LEN - 1);
        pin_mut!(s);

        let err = s.next().await.unwrap().unwrap_err();
        match err {
            TransferError::InvalidMetadata(_) => {}
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[tokio::test]
    async fn frames_errors_when_no_room_for_payload() {
        let framer = Framer::new(65_535);
        let payload = Bytes::from_static(b"abc");

        // max_frame_payload == header => chunk_max == 0 while payload non-empty
        let s = framer.frames(payload, SerializationFormat::Json, 0, FrameHeader::LEN);
        pin_mut!(s);

        let err = s.next().await.unwrap().unwrap_err();
        match err {
            TransferError::InvalidMetadata(_) => {}
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[tokio::test]
    async fn frames_chunking_and_indices_are_correct() {
        let framer = Framer::new(65_535);

        // payload 10 bytes, chunk_max 4 => total 3 frames (4+4+2)
        let payload = Bytes::from_static(b"0123456789");
        let max_frame_payload = FrameHeader::LEN + 4;

        let s = framer.frames(
            payload.clone(),
            SerializationFormat::Cbor,
            2,
            max_frame_payload,
        );
        pin_mut!(s);

        let mut frames = Vec::new();
        while let Some(item) = s.next().await {
            frames.push(item.unwrap());
        }

        assert_eq!(frames.len(), 3);

        // each frame payload <= max_frame_payload
        for f in &frames {
            assert!(FrameHeader::LEN + f.chunk.len() <= max_frame_payload);
        }

        // verify totals/indices
        for (i, f) in frames.iter().enumerate() {
            let h = f.header;
            assert_eq!(&h[0..2], b"FR");
            assert_eq!(h[2], 1);
            assert_eq!(h[3], SerializationFormat::Cbor as u8);
            assert_eq!(h[4], 2);

            let idx = parse_u32_be(&h[13..17]);
            let total = parse_u32_be(&h[17..21]);

            assert_eq!(idx, i as u32);
            assert_eq!(total, 3);
        }

        // verify chunk content order
        let joined: Vec<u8> = frames
            .iter()
            .flat_map(|f| f.chunk.iter().copied())
            .collect();

        assert_eq!(joined, payload.to_vec());
    }

    #[tokio::test]
    async fn frames_are_zero_copy_slices() {
        let framer = Framer::new(65_535);

        let payload = Bytes::from_static(b"abcdefghijklmnopqrstuvwxyz");
        let base_ptr = payload.as_ptr() as usize;
        let base_len = payload.len();

        let max_frame_payload = FrameHeader::LEN + 5;
        let s = framer.frames(
            payload.clone(),
            SerializationFormat::Json,
            1,
            max_frame_payload,
        );
        pin_mut!(s);

        while let Some(item) = s.next().await {
            let f = item.unwrap();

            // chunk must point inside original payload allocation
            let p = f.chunk.as_ptr() as usize;
            assert!(p >= base_ptr);
            assert!(p < base_ptr + base_len);

            // chunk must not exceed original payload range
            assert!(p + f.chunk.len() <= base_ptr + base_len);
        }
    }

    #[tokio::test]
    async fn stream_id_increments_between_calls() {
        let framer = Framer::new(65_535);

        let p1 = Bytes::from_static(b"111111");
        let p2 = Bytes::from_static(b"222222");

        let s1 = framer.frames(p1, SerializationFormat::Json, 0, FrameHeader::LEN + 3);
        pin_mut!(s1);
        let f1 = s1.next().await.unwrap().unwrap();
        let stream_id_1 = parse_u64_be(&f1.header[5..13]);

        let s2 = framer.frames(p2, SerializationFormat::Json, 0, FrameHeader::LEN + 3);
        pin_mut!(s2);
        let f2 = s2.next().await.unwrap().unwrap();
        let stream_id_2 = parse_u64_be(&f2.header[5..13]);

        assert!(stream_id_2 > stream_id_1);
    }
}
