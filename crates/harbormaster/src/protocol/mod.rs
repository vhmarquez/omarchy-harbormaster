//! Strict, bounded protocol v0 data. Parsing and identity strings confer no authority.

mod capability;
mod control;
mod event;
mod identity;
mod response;
mod strict;

pub use capability::*;
pub use control::*;
pub use event::*;
pub use identity::*;
pub use response::*;
pub use strict::encode_frame;

/// Maximum NDJSON frame bytes, including the terminating LF.
pub const MAX_FRAME_BYTES: usize = 16_384;
/// Maximum number of nested object/array containers, counting the root as one.
pub const MAX_JSON_DEPTH: usize = 8;

/// Fixed protocol errors never include caller-controlled strings or exceptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolError {
    UnsupportedVersion,
    InvalidFrame,
    UnknownProducer,
    StaleGeneration,
    SequenceGap,
    CapabilityUnavailable,
    StaleTarget,
    PermissionDenied,
    Conflict,
    ResourceExhausted,
    PersistenceUnavailable,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidFrame => "invalid_frame",
            Self::UnknownProducer => "unknown_producer",
            Self::StaleGeneration => "stale_generation",
            Self::SequenceGap => "sequence_gap",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::StaleTarget => "stale_target",
            Self::PermissionDenied => "permission_denied",
            Self::Conflict => "conflict",
            Self::ResourceExhausted => "resource_exhausted",
            Self::PersistenceUnavailable => "persistence_unavailable",
        })
    }
}

impl std::error::Error for ProtocolError {}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(try_from = "u8", into = "u8")]
#[derive(Clone, Copy)]
pub(super) struct Version;
impl TryFrom<u8> for Version {
    type Error = ProtocolError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 0 {
            Ok(Self)
        } else {
            Err(ProtocolError::UnsupportedVersion)
        }
    }
}
impl From<Version> for u8 {
    fn from(_: Version) -> Self {
        0
    }
}
