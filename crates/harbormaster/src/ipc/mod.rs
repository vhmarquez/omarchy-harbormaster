//! Bounded Linux Unix IPC. Peer/channel evidence is not operation authorization.
//!
//! Private directories exclude other UIDs. They do not isolate malicious code
//! running as this UID. No listener is activated by the executable.

mod connection;
mod framing;
mod paths;
mod peer;

pub use connection::{Connection, ConnectionBudget};
pub use framing::{FrameDecoder, OutboundQueue};
pub use paths::PrivateSockets;
pub use peer::PeerCredentials;

use std::fmt;

/// The accepting socket determines the channel, never a JSON field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Event,
    Control,
}

/// Sanitized failures never expose paths, source bytes, or arbitrary OS output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    UnsafePath,
    PermissionDenied,
    ResourceExhausted,
    FrameTooLarge,
    InvalidFrame,
    TruncatedFrame,
    DeadlineExceeded,
    Closed,
    Io,
}

impl fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafePath => "unsafe private IPC path",
            Self::PermissionDenied => "IPC peer permission denied",
            Self::ResourceExhausted => "IPC resource limit reached",
            Self::FrameTooLarge => "IPC frame exceeds byte limit",
            Self::InvalidFrame => "invalid IPC frame",
            Self::TruncatedFrame => "incomplete IPC frame",
            Self::DeadlineExceeded => "IPC deadline exceeded",
            Self::Closed => "IPC connection closed",
            Self::Io => "IPC operation failed",
        })
    }
}

impl std::error::Error for IpcError {}

impl From<std::io::Error> for IpcError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

impl From<rustix::io::Errno> for IpcError {
    fn from(_: rustix::io::Errno) -> Self {
        Self::Io
    }
}
