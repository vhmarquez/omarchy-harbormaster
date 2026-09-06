//! An empty production registry is intentional: wire string types prove no origin.
use super::{MAX_RECORD_BYTES, RecoveryError};
use crate::protocol::{EventEnvelope, EventKind, HarnessKind, parse_event};
use serde_json::{Value, json};

/// Borrowed, bounded source input. Never formatted, logged or persisted verbatim.
#[derive(Clone, Copy)]
pub struct AdapterRecord<'a> {
    pub harness: HarnessKind,
    pub version: &'a str,
    pub source: &'a [u8],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    Metadata,
    #[cfg(test)]
    Unproven,
    #[cfg(test)]
    Prompt,
    #[cfg(test)]
    Environment,
}

pub(super) struct Mapping {
    pub harness: HarnessKind,
    pub version: &'static str,
    pub turn: Source,
    pub session: Source,
    pub health_version: Source,
}

// Adding a real entry requires independently reviewed, version-specific adapter
// source evidence and canaries. Historical M0 fixtures do not authorize an entry.
pub(super) const REVIEWED: &[Mapping] = &[];

pub(super) struct EligibleMetadata {
    event: EventEnvelope,
    harness: HarnessKind,
    version: &'static str,
}
impl EligibleMetadata {
    pub fn into_parts(self) -> (EventEnvelope, HarnessKind, &'static str) {
        (self.event, self.harness, self.version)
    }
}

pub(super) fn mapping<'a>(
    harness: HarnessKind,
    version: &str,
    reviewed: &'a [Mapping],
) -> Result<&'a Mapping, RecoveryError> {
    reviewed
        .iter()
        .find(|item| item.harness == harness && item.version == version)
        .ok_or(RecoveryError::UnsupportedAdapter)
}

pub(super) fn eligible(
    input: AdapterRecord<'_>,
    reviewed: &[Mapping],
) -> Result<EligibleMetadata, RecoveryError> {
    if input.version.is_empty() || input.version.len() > 64 || input.source.len() > MAX_RECORD_BYTES
    {
        return Err(RecoveryError::BoundExceeded);
    }
    let mapping = mapping(input.harness, input.version, reviewed)?;
    let raw: Value =
        serde_json::from_slice(input.source).map_err(|_| RecoveryError::InvalidRecord)?;
    let metadata = raw.get("metadata").ok_or(RecoveryError::InvalidRecord)?;
    let kind: EventKind = serde_json::from_value(metadata["kind"].clone())
        .map_err(|_| RecoveryError::InvalidRecord)?;
    let payload = payload(metadata, kind, mapping)?;
    let event = json!({"protocol":0,"channel":"event", "event_id":metadata["event_id"],
        "producer_id":metadata["producer_id"], "generation":metadata["generation"],
        "run_id":metadata["run_id"], "seq":metadata["seq"], "kind":kind,"payload":payload});
    let mut frame = serde_json::to_vec(&event).map_err(|_| RecoveryError::InvalidRecord)?;
    frame.push(b'\n');
    Ok(EligibleMetadata {
        event: parse_event(&frame).map_err(|_| RecoveryError::InvalidRecord)?,
        harness: input.harness,
        version: mapping.version,
    })
}

fn payload(metadata: &Value, kind: EventKind, mapping: &Mapping) -> Result<Value, RecoveryError> {
    let source = &metadata["payload"];
    match kind {
        EventKind::RunStarted => Ok(
            json!({"harness_session_id": if mapping.session == Source::Metadata { source["harness_session_id"].clone() } else { Value::Null }}),
        ),
        EventKind::RunExited => {
            if source.get("exit_code").is_some() {
                Ok(json!({"exit_code":source["exit_code"]}))
            } else {
                Ok(json!({"reason":source["reason"]}))
            }
        }
        EventKind::ProducerHealth => {
            if mapping.health_version != Source::Metadata {
                return Err(RecoveryError::UnprovenSource);
            }
            Ok(json!({"adapter_version":mapping.version,"signals":source["signals"]}))
        }
        _ => {
            if mapping.turn != Source::Metadata {
                return Err(RecoveryError::UnprovenSource);
            }
            if kind == EventKind::TurnFailed {
                Ok(json!({"turn_id":source["turn_id"],"reason":source["reason"]}))
            } else {
                Ok(json!({"turn_id":source["turn_id"]}))
            }
        }
    }
}

pub(super) fn validate_replay(
    event: &EventEnvelope,
    mapping: &Mapping,
) -> Result<(), RecoveryError> {
    match &event.event {
        crate::protocol::EventPayload::RunStarted(payload) => {
            if payload.harness_session_id.is_some() && mapping.session != Source::Metadata {
                return Err(RecoveryError::UnprovenSource);
            }
        }
        crate::protocol::EventPayload::RunExited(_) => (),
        crate::protocol::EventPayload::ProducerHealth(payload) => {
            if mapping.health_version != Source::Metadata
                || payload.adapter_version.as_str() != mapping.version
            {
                return Err(RecoveryError::UnprovenSource);
            }
        }
        _ if mapping.turn != Source::Metadata => return Err(RecoveryError::UnprovenSource),
        _ => (),
    }
    Ok(())
}
