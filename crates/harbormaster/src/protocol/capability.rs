//! Capability metadata and a control negotiation claim, independent of grants.

use serde::{Deserialize, Serialize};

use super::{
    AdapterVersion, Operation, ProtocolError, Version, control::ControlChannel,
    event::unique_bounded, strict,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Unsupported,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    Manager,
    Adapter,
    NativeHarness,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationFreshness {
    Fresh,
    Stale,
    Disconnected,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityReason {
    Available,
    NotImplemented,
    NotGranted,
    VersionUntested,
    StaleIdentity,
    UnsupportedHarness,
}

/// Bounded metadata only; supported alone is not permission or target proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRecord {
    pub operation: Operation,
    pub status: CapabilityStatus,
    pub source: CapabilitySource,
    pub tested_version: Option<AdapterVersion>,
    pub reason: CapabilityReason,
    pub freshness: ObservationFreshness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlHandshake {
    pub requested_operations: Vec<Operation>,
}

#[derive(Serialize, Deserialize)]
enum Negotiate {
    #[serde(rename = "negotiate")]
    Negotiate,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireHandshake {
    protocol: Version,
    channel: ControlChannel,
    operation: Negotiate,
    requested_operations: Vec<Operation>,
}

impl Serialize for ControlHandshake {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        unique_bounded(&self.requested_operations, Operation::ALL.len())
            .map_err(serde::ser::Error::custom)?;
        WireHandshake {
            protocol: Version,
            channel: ControlChannel::Control,
            operation: Negotiate::Negotiate,
            requested_operations: self.requested_operations.clone(),
        }
        .serialize(serializer)
    }
}

/// Parse requested capabilities; the manager must intersect them with trusted grants.
/// # Errors
/// Rejects unknown/duplicate operations, invalid frames and unsupported versions.
pub fn parse_control_handshake(frame: &[u8]) -> Result<ControlHandshake, ProtocolError> {
    let raw: WireHandshake = strict::typed(strict::decode(frame)?)?;
    unique_bounded(&raw.requested_operations, Operation::ALL.len())?;
    Ok(ControlHandshake {
        requested_operations: raw.requested_operations,
    })
}
