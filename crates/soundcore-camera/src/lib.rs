//! Camera multiplex proxy.
//!
//! Architecture:
//!
//!   real webcam ──► [single MF SourceReader, owned by core] ──► [ring of
//!   shared-memory frame slots, GPU-friendly layout] ──► N consumers, each
//!   a process that opened our virtual camera filter.
//!
//! The native virtual-camera DirectShow Source Filter (in `native/virtual-camera`)
//! is a thin client that maps the shared memory and copies frames into the
//! sample buffer it hands to downstream DirectShow consumers. The same
//! design is reused on Windows 11 by the optional Media Foundation
//! Virtual Camera shim.
//!
//! This crate hosts the *producer* side: enumeration, the MF capture loop,
//! the shared-memory ring management. The consumer side is C++.

use thiserror::Error;

pub mod broadcaster;
pub mod capture;
pub mod init;
pub mod source;

pub use source::{CameraFormat, CameraSource};

#[derive(Debug, Error)]
pub enum CameraError {
    #[error("COM error: {0}")]
    Com(#[from] windows::core::Error),

    #[error("camera not found: {0}")]
    NotFound(String),

    #[error("no compatible format")]
    NoCompatibleFormat,

    #[error("camera in use by another process")]
    Busy,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, CameraError>;
