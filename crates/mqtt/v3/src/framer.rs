//! Splits an encoded payload into frames that each fit into one MQTT packet.
//!
//! Frame layout (big-endian):
//!
//! | offset | size | field                                  |
//! |--------|------|----------------------------------------|
//! | 0      | 2    | magic `"FR"`                           |
//! | 2      | 1    | version (1)                            |
//! | 3      | 1    | serialization format (`SerializationFormat as u8`) |
//! | 4      | 1    | QoS used for the frames                |
//! | 5      | 8    | stream id, unique per payload          |
//! | 13     | 4    | frame index, 0-based                   |
//! | 17     | 4    | total number of frames                 |
//! | 21     | n    | payload chunk                          |
//!
//! An empty payload produces exactly one frame with an empty chunk.

use crate::{config::SerializationFormat, error::TransferError};

const FRAME_MAGIC: [u8; 2] = *b"FR";
const FRAME_VERSION: u8 = 1;

/// Size of the fixed header that precedes every chunk.
pub const FRAME_HEADER_LEN: usize = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub format: SerializationFormat,
    pub qos: u8,
    pub stream_id: u64,
    pub frame_index: u32,
    pub frame_total: u32,
}

impl FrameHeader {
    pub fn encode(self) -> [u8; FRAME_HEADER_LEN] {
        let mut out = [0u8; FRAME_HEADER_LEN];
        out[..2].copy_from_slice(&FRAME_MAGIC);
        out[2] = FRAME_VERSION;
        out[3] = self.format as u8;
        out[4] = self.qos;
        out[5..13].copy_from_slice(&self.stream_id.to_be_bytes());
        out[13..17].copy_from_slice(&self.frame_index.to_be_bytes());
        out[17..21].copy_from_slice(&self.frame_total.to_be_bytes());
        out
    }

    /// Parses a header; used by consumers and tests.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < FRAME_HEADER_LEN || bytes[..2] != FRAME_MAGIC || bytes[2] != FRAME_VERSION
        {
            return None;
        }
        let format = match bytes[3] {
            0 => SerializationFormat::Json,
            1 => SerializationFormat::MessagePack,
            2 => SerializationFormat::Cbor,
            _ => return None,
        };
        Some(Self {
            format,
            qos: bytes[4],
            stream_id: u64::from_be_bytes(bytes[5..13].try_into().ok()?),
            frame_index: u32::from_be_bytes(bytes[13..17].try_into().ok()?),
            frame_total: u32::from_be_bytes(bytes[17..21].try_into().ok()?),
        })
    }
}

/// Returns the frames for `payload`. Each frame is `FRAME_HEADER_LEN + chunk`
/// bytes and never longer than `max_frame_len`.
pub fn frames<'a>(
    payload: &'a [u8],
    format: SerializationFormat,
    qos: u8,
    stream_id: u64,
    max_frame_len: usize,
) -> Result<impl Iterator<Item = Vec<u8>> + 'a, TransferError> {
    let chunk_max = max_frame_len
        .checked_sub(FRAME_HEADER_LEN)
        .filter(|n| *n > 0 || payload.is_empty())
        .ok_or_else(|| {
            TransferError::Framing(format!(
                "max_packet_size leaves no room for payload after the {FRAME_HEADER_LEN}-byte frame header"
            ))
        })?;
    let total = if payload.is_empty() {
        1
    } else {
        payload.len().div_ceil(chunk_max)
    };
    let frame_total = u32::try_from(total)
        .map_err(|_| TransferError::Framing("payload needs more than u32::MAX frames".into()))?;

    let chunks: Box<dyn Iterator<Item = &'a [u8]> + 'a> = if payload.is_empty() {
        Box::new(std::iter::once(&payload[..0]))
    } else {
        Box::new(payload.chunks(chunk_max))
    };
    Ok(chunks.enumerate().map(move |(index, chunk)| {
        let header = FrameHeader {
            format,
            qos,
            stream_id,
            frame_index: index as u32,
            frame_total,
        }
        .encode();
        let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + chunk.len());
        frame.extend_from_slice(&header);
        frame.extend_from_slice(chunk);
        frame
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(payload: &[u8], max: usize) -> Vec<Vec<u8>> {
        frames(payload, SerializationFormat::Cbor, 2, 7, max)
            .unwrap()
            .collect()
    }

    #[test]
    fn empty_payload_is_one_frame() {
        let out = collect(b"", FRAME_HEADER_LEN);
        assert_eq!(out.len(), 1);
        let h = FrameHeader::decode(&out[0]).unwrap();
        assert_eq!(
            (h.frame_index, h.frame_total, h.qos, h.stream_id),
            (0, 1, 2, 7)
        );
        assert_eq!(h.format, SerializationFormat::Cbor);
        assert_eq!(out[0].len(), FRAME_HEADER_LEN);
    }

    #[test]
    fn chunks_are_ordered_bounded_and_reassemble() {
        let payload = b"0123456789";
        let out = collect(payload, FRAME_HEADER_LEN + 4);
        assert_eq!(out.len(), 3);
        let mut joined = Vec::new();
        for (i, frame) in out.iter().enumerate() {
            assert!(frame.len() <= FRAME_HEADER_LEN + 4);
            let h = FrameHeader::decode(frame).unwrap();
            assert_eq!(h.frame_index as usize, i);
            assert_eq!(h.frame_total, 3);
            joined.extend_from_slice(&frame[FRAME_HEADER_LEN..]);
        }
        assert_eq!(joined, payload);
    }

    #[test]
    fn rejects_frames_without_room() {
        assert!(frames(b"abc", SerializationFormat::Json, 0, 1, FRAME_HEADER_LEN).is_err());
        assert!(frames(
            b"abc",
            SerializationFormat::Json,
            0,
            1,
            FRAME_HEADER_LEN - 1
        )
        .is_err());
    }

    #[test]
    fn header_round_trip_and_rejects_garbage() {
        let h = FrameHeader {
            format: SerializationFormat::MessagePack,
            qos: 1,
            stream_id: u64::MAX,
            frame_index: 5,
            frame_total: 9,
        };
        assert_eq!(FrameHeader::decode(&h.encode()), Some(h));
        assert!(FrameHeader::decode(b"XX").is_none());
    }
}
