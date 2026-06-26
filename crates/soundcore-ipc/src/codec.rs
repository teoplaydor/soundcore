//! Length-delimited protobuf codec for the SoundCore IPC frame stream.

use bytes::{Buf, BytesMut};
use prost::Message;
use std::io;
use tokio_util::codec::{Decoder, Encoder};

use crate::proto::Frame;
use crate::MAX_FRAME_BYTES;

#[derive(Debug, Default, Clone, Copy)]
pub struct FrameCodec;

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Frame>> {
        // Peek a varint length without consuming bytes from `src`.
        let snapshot = &src[..];
        let payload_len = match prost::decode_length_delimiter(snapshot) {
            Ok(len) => len,
            // A varint length delimiter is at most 10 bytes. If decode fails
            // with that many bytes already buffered, the prefix is genuinely
            // malformed (not merely incomplete) — surface a protocol error
            // instead of buffering forever / hanging the connection.
            Err(_) if src.len() >= 10 => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "malformed frame length varint",
                ));
            }
            Err(_) => return Ok(None), // not enough bytes for a complete varint yet
        };

        if payload_len > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large: {payload_len} bytes"),
            ));
        }

        let header_len = prost::length_delimiter_len(payload_len);
        let total = header_len + payload_len;
        if src.len() < total {
            // Hint the buffer the amount we still need so it can reserve.
            src.reserve(total - src.len());
            return Ok(None);
        }

        src.advance(header_len);
        let payload = src.split_to(payload_len);
        let frame = Frame::decode(payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(frame))
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Frame, dst: &mut BytesMut) -> io::Result<()> {
        let needed = item.encoded_len();
        if needed > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large to encode: {needed} bytes"),
            ));
        }
        dst.reserve(prost::length_delimiter_len(needed) + needed);
        item.encode_length_delimited(dst)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl Encoder<&Frame> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, item: &Frame, dst: &mut BytesMut) -> io::Result<()> {
        let needed = item.encoded_len();
        if needed > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large to encode: {needed} bytes"),
            ));
        }
        dst.reserve(prost::length_delimiter_len(needed) + needed);
        item.encode_length_delimited(dst)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
