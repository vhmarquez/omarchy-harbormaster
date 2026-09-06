//! Explicit control grants; producer handshake data never creates these grants.
use super::AdmissionError;
use crate::ipc::{Channel, Connection};
use crate::protocol::{CapabilityRecord, CapabilityStatus, ControlHandshake, ControlRequest,
    ObservationFreshness, Operation, Revision};
use std::collections::{BTreeMap, BTreeSet};

/// Constructed only from a real control channel, then held by that connection.
pub struct ControlSession {
    uid: u32,
    requested: BTreeSet<Operation>,
}

/// Trusted manager-owned grants and current capability evidence.
/// Successful authorization is only an operation permission check. The eventual
/// executor still owes current ownership/process identity and durable semantics.
pub struct ControlAuthorizer {
    uid: u32,
    granted: BTreeSet<Operation>,
    capabilities: BTreeMap<Operation, CapabilityRecord>,
}

impl ControlAuthorizer {
    /// Empty capabilities deny all operations, even for the correct UID.
    #[must_use]
    pub fn new(uid: u32, granted: impl IntoIterator<Item = Operation>) -> Self {
        Self { uid, granted: granted.into_iter().collect(), capabilities: BTreeMap::new() }
    }

    /// Replace one trusted capability observation; this is not producer input.
    pub fn set_capability(&mut self, capability: CapabilityRecord) {
        self.capabilities.insert(capability.operation, capability);
    }

    /// Negotiate a bounded set without granting any operation by negotiation.
    /// # Errors
    /// Rejects the event channel, wrong peer, or duplicate/invalid requests.
    pub fn connect(&self, peer: &Connection, handshake: &ControlHandshake) -> Result<ControlSession, AdmissionError> {
        if peer.channel() != Channel::Control || peer.peer().uid() != self.uid {
            return Err(AdmissionError::PermissionDenied);
        }
        let requested: BTreeSet<_> = handshake.requested_operations.iter().copied().collect();
        if requested.len() != handshake.requested_operations.len() || requested.len() > Operation::ALL.len() {
            return Err(AdmissionError::InvalidHandshake);
        }
        Ok(ControlSession { uid: self.uid, requested })
    }

    /// Read-only authorization; never launch, mutate policy, or acknowledge data.
    /// # Errors
    /// Requires an explicit grant, negotiated operation, fresh supported evidence,
    /// and an equal expected revision for stateful requests.
    pub fn authorize(&self, session: &ControlSession, request: &ControlRequest, current: Revision) -> Result<(), AdmissionError> {
        let operation = request.operation();
        if session.uid != self.uid || !session.requested.contains(&operation) || !self.granted.contains(&operation) {
            return Err(AdmissionError::PermissionDenied);
        }
        let capability = self.capabilities.get(&operation).ok_or(AdmissionError::CapabilityUnavailable)?;
        if capability.status != CapabilityStatus::Supported || capability.freshness != ObservationFreshness::Fresh {
            return Err(AdmissionError::CapabilityUnavailable);
        }
        if request.expected_revision().is_some_and(|expected| expected != current) {
            return Err(AdmissionError::StaleRevision);
        }
        Ok(())
    }
}
