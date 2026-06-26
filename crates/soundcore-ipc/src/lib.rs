//! SoundCore IPC.
//!
//! Wire format: varint-length-prefixed protobuf `Frame` messages over a
//! bidirectional Windows named pipe. The default pipe path is
//! [`PIPE_NAME`]; both server and client are async and exposed as
//! Tokio [`tokio_util::codec::Framed`] streams.

pub mod proto {
    #![allow(clippy::all)]
    #![allow(missing_docs)]
    include!(concat!(env!("OUT_DIR"), "/soundcore.v1.rs"));
}

pub mod codec;
pub mod transport;

/// Canonical pipe path. The service creates it, the UI client connects to it.
pub const PIPE_NAME: &str = r"\\.\pipe\SoundCore.Service";

/// Hard ceiling on a single message size, matched on both ends.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

pub use codec::FrameCodec;
pub use proto::{
    frame::Body as FrameBody, request::Payload as RequestPayload,
    response::Payload as ResponsePayload, Event, Frame, Request, Response,
};
