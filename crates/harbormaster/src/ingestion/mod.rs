//! Volatile admission and authorization. No durable acknowledgment or reducer.
mod control;
mod generation;
mod queue;
mod registry;

pub use control::{ControlAuthorizer, ControlSession};
pub use registry::{EventSession, ProducerRegistration, Registry};

use crate::protocol::ProtocolError;
use std::fmt;

pub const MAX_PRODUCERS: usize = 10_000;
pub const MAX_QUEUED: usize = 1_024;
pub const MAX_PRODUCER_QUEUED: usize = 128;
pub const MAX_PRODUCER_RECEIPTS: usize = 1_024;
pub const MAX_RECEIPTS: usize = 8_192;
pub const MAX_RECEIPT_BYTES: usize = 8 * 1_024 * 1_024;

/// Neither result acknowledges storage, a lifecycle transition, or delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Queued,
    AlreadyAdmitted,
}

/// Fixed metadata-only errors; input bytes are never retained in errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    Protocol(ProtocolError),
    PermissionDenied,
    UnknownProducer,
    StaleGeneration,
    InvalidHandshake,
    Conflict,
    SequenceGap,
    ReplayRejected,
    ReconciliationRequired,
    QueueFull,
    ResourceExhausted,
    CapabilityUnavailable,
    StaleRevision,
    EntropyUnavailable,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Protocol(_) => "invalid_frame",
            Self::PermissionDenied => "permission_denied",
            Self::UnknownProducer => "unknown_producer",
            Self::StaleGeneration => "stale_generation",
            Self::InvalidHandshake => "invalid_handshake",
            Self::Conflict => "conflict",
            Self::SequenceGap => "sequence_gap",
            Self::ReplayRejected => "replay_rejected",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::QueueFull | Self::ResourceExhausted => "resource_exhausted",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::StaleRevision => "stale_target",
            Self::EntropyUnavailable => "entropy_unavailable",
        })
    }
}

impl std::error::Error for AdmissionError {}
