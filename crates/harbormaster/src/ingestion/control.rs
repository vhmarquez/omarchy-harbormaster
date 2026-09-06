//! Explicit control grants; producer handshake data never creates these grants.
use super::AdmissionError;
use crate::ipc::{Channel, Connection};
use crate::protocol::{
    CapabilityReason, CapabilityRecord, CapabilitySource, CapabilityStatus, ControlHandshake,
    ControlRequest, ObservationFreshness, Operation, Revision,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Constructed only from a real control channel, then held by that connection.
pub struct ControlSession {
    authority: Rc<()>,
    uid: u32,
    requested: BTreeSet<Operation>,
}

/// Trusted manager-owned grants and current capability evidence.
/// Successful authorization is only an operation permission check. The eventual
/// executor still owes current ownership/process identity and durable semantics.
pub struct ControlAuthorizer {
    authority: Rc<()>,
    uid: u32,
    granted: BTreeSet<Operation>,
    capabilities: BTreeMap<Operation, CapabilityRecord>,
}

impl ControlAuthorizer {
    /// Empty capabilities deny all operations, even for the correct UID.
    #[must_use]
    pub fn new(uid: u32, granted: impl IntoIterator<Item = Operation>) -> Self {
        Self {
            authority: Rc::new(()),
            uid,
            granted: granted.into_iter().collect(),
            capabilities: BTreeMap::new(),
        }
    }

    /// Replace one trusted capability observation; this is not producer input.
    pub fn set_capability(&mut self, capability: CapabilityRecord) {
        self.capabilities.insert(capability.operation, capability);
    }

    /// Read the negotiated results without copying authorization rules to a UI.
    /// # Errors
    /// Rejects a session created by another authorizer.
    pub fn negotiated_capabilities(
        &self,
        session: &ControlSession,
    ) -> Result<Vec<CapabilityRecord>, AdmissionError> {
        if !Rc::ptr_eq(&session.authority, &self.authority) {
            return Err(AdmissionError::PermissionDenied);
        }
        Ok(session
            .requested
            .iter()
            .map(|operation| {
                if self.granted.contains(operation)
                    && let Some(record) = self.capabilities.get(operation)
                {
                    return record.clone();
                }
                CapabilityRecord {
                    operation: *operation,
                    status: CapabilityStatus::Unsupported,
                    source: CapabilitySource::Manager,
                    tested_version: None,
                    reason: if self.granted.contains(operation) {
                        CapabilityReason::NotImplemented
                    } else {
                        CapabilityReason::NotGranted
                    },
                    freshness: ObservationFreshness::Unsupported,
                }
            })
            .collect())
    }

    /// Negotiate a bounded set without granting any operation by negotiation.
    /// # Errors
    /// Rejects the event channel, wrong peer, or duplicate/invalid requests.
    pub fn connect(
        &self,
        peer: &Connection,
        handshake: &ControlHandshake,
    ) -> Result<ControlSession, AdmissionError> {
        if peer.channel() != Channel::Control || peer.peer().uid() != self.uid {
            return Err(AdmissionError::PermissionDenied);
        }
        let requested: BTreeSet<_> = handshake.requested_operations.iter().copied().collect();
        if requested.len() != handshake.requested_operations.len()
            || requested.len() > Operation::ALL.len()
        {
            return Err(AdmissionError::InvalidHandshake);
        }
        Ok(ControlSession {
            authority: self.authority.clone(),
            uid: self.uid,
            requested,
        })
    }

    /// Read-only authorization; never launch, mutate policy, or acknowledge data.
    /// # Errors
    /// Requires an explicit grant, negotiated operation, fresh supported evidence,
    /// and an equal expected revision for stateful requests.
    pub fn authorize(
        &self,
        session: &ControlSession,
        request: &ControlRequest,
        current: Revision,
    ) -> Result<(), AdmissionError> {
        let operation = request.operation();
        if !Rc::ptr_eq(&session.authority, &self.authority)
            || session.uid != self.uid
            || !session.requested.contains(&operation)
            || !self.granted.contains(&operation)
        {
            return Err(AdmissionError::PermissionDenied);
        }
        let capability = self
            .capabilities
            .get(&operation)
            .ok_or(AdmissionError::CapabilityUnavailable)?;
        if !usable(capability) {
            return Err(AdmissionError::CapabilityUnavailable);
        }
        if request
            .expected_revision()
            .is_some_and(|expected| expected != current)
        {
            return Err(AdmissionError::StaleRevision);
        }
        Ok(())
    }
}

fn usable(capability: &CapabilityRecord) -> bool {
    capability.status == CapabilityStatus::Supported
        && capability.freshness == ObservationFreshness::Fresh
        && capability.reason == CapabilityReason::Available
        && (capability.source == CapabilitySource::Manager || capability.tested_version.is_some())
}
