//! Allowlisted metadata-only producer event data and negotiation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AdapterVersion, EventId, HarnessSessionId, ProducerGeneration, ProducerId, ProtocolError,
    RunId, Seq, TurnId, Version, strict,
};

/// Built-in harness vocabulary; not lifecycle or capability evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessKind {
    Hermes,
    Claude,
    Codex,
}

/// Exact event vocabulary. Health signals can only name these kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventKind {
    #[serde(rename = "run.started")]
    RunStarted,
    #[serde(rename = "run.exited")]
    RunExited,
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "turn.awaiting_input")]
    TurnAwaitingInput,
    #[serde(rename = "turn.awaiting_approval")]
    TurnAwaitingApproval,
    #[serde(rename = "turn.completed")]
    TurnCompleted,
    #[serde(rename = "turn.failed")]
    TurnFailed,
    #[serde(rename = "turn.interrupted")]
    TurnInterrupted,
    #[serde(rename = "producer.health")]
    ProducerHealth,
}

impl EventKind {
    pub const ALL: [Self; 9] = [
        Self::RunStarted,
        Self::RunExited,
        Self::TurnStarted,
        Self::TurnAwaitingInput,
        Self::TurnAwaitingApproval,
        Self::TurnCompleted,
        Self::TurnFailed,
        Self::TurnInterrupted,
        Self::ProducerHealth,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStartedPayload {
    pub harness_session_id: Option<HarnessSessionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    Signaled,
    Lost,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum RunExitedPayload {
    ExitCode { exit_code: i32 },
    Terminated { reason: TerminationReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnPayload {
    pub turn_id: TurnId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    HarnessError,
    Cancelled,
    TimedOut,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnFailedPayload {
    pub turn_id: TurnId,
    pub reason: FailureReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthPayload {
    pub adapter_version: AdapterVersion,
    pub signals: Vec<EventKind>,
}

/// The tag determines the payload structure; arbitrary output is never accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum EventPayload {
    #[serde(rename = "run.started")]
    RunStarted(RunStartedPayload),
    #[serde(rename = "run.exited")]
    RunExited(RunExitedPayload),
    #[serde(rename = "turn.started")]
    TurnStarted(TurnPayload),
    #[serde(rename = "turn.awaiting_input")]
    TurnAwaitingInput(TurnPayload),
    #[serde(rename = "turn.awaiting_approval")]
    TurnAwaitingApproval(TurnPayload),
    #[serde(rename = "turn.completed")]
    TurnCompleted(TurnPayload),
    #[serde(rename = "turn.failed")]
    TurnFailed(TurnFailedPayload),
    #[serde(rename = "turn.interrupted")]
    TurnInterrupted(TurnPayload),
    #[serde(rename = "producer.health")]
    ProducerHealth(HealthPayload),
}

impl EventPayload {
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::RunStarted(_) => EventKind::RunStarted,
            Self::RunExited(_) => EventKind::RunExited,
            Self::TurnStarted(_) => EventKind::TurnStarted,
            Self::TurnAwaitingInput(_) => EventKind::TurnAwaitingInput,
            Self::TurnAwaitingApproval(_) => EventKind::TurnAwaitingApproval,
            Self::TurnCompleted(_) => EventKind::TurnCompleted,
            Self::TurnFailed(_) => EventKind::TurnFailed,
            Self::TurnInterrupted(_) => EventKind::TurnInterrupted,
            Self::ProducerHealth(_) => EventKind::ProducerHealth,
        }
    }
    fn decode(kind: EventKind, payload: Value) -> Result<Self, ProtocolError> {
        Ok(match kind {
            EventKind::RunStarted => Self::RunStarted(strict::typed(payload)?),
            EventKind::RunExited => Self::RunExited(strict::typed(payload)?),
            EventKind::TurnStarted => Self::TurnStarted(strict::typed(payload)?),
            EventKind::TurnAwaitingInput => Self::TurnAwaitingInput(strict::typed(payload)?),
            EventKind::TurnAwaitingApproval => Self::TurnAwaitingApproval(strict::typed(payload)?),
            EventKind::TurnCompleted => Self::TurnCompleted(strict::typed(payload)?),
            EventKind::TurnFailed => Self::TurnFailed(strict::typed(payload)?),
            EventKind::TurnInterrupted => Self::TurnInterrupted(strict::typed(payload)?),
            EventKind::ProducerHealth => {
                let health: HealthPayload = strict::typed(payload)?;
                unique_bounded(&health.signals, EventKind::ALL.len())?;
                Self::ProducerHealth(health)
            }
        })
    }
}

#[derive(Serialize, Deserialize)]
enum EventChannel {
    #[serde(rename = "event")]
    Event,
}

/// A validated wire observation; admission/persistence must authorize it separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub producer_id: ProducerId,
    pub generation: ProducerGeneration,
    pub seq: Seq,
    pub run_id: RunId,
    pub event: EventPayload,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    protocol: Version,
    channel: EventChannel,
    event_id: EventId,
    producer_id: ProducerId,
    generation: ProducerGeneration,
    seq: Seq,
    run_id: RunId,
    kind: EventKind,
    payload: Value,
}

impl Serialize for EventEnvelope {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            protocol: Version,
            channel: EventChannel,
            event_id: &'a EventId,
            producer_id: &'a ProducerId,
            generation: &'a ProducerGeneration,
            seq: Seq,
            run_id: &'a RunId,
            #[serde(flatten)]
            event: &'a EventPayload,
        }
        if let EventPayload::ProducerHealth(health) = &self.event {
            unique_bounded(&health.signals, EventKind::ALL.len())
                .map_err(serde::ser::Error::custom)?;
        }
        Wire {
            protocol: Version,
            channel: EventChannel::Event,
            event_id: &self.event_id,
            producer_id: &self.producer_id,
            generation: &self.generation,
            seq: self.seq,
            run_id: &self.run_id,
            event: &self.event,
        }
        .serialize(serializer)
    }
}

/// Parse exactly one event frame, without registry lookup or side effects.
/// # Errors
/// Rejects invalid/version-mismatched framing, structure, identities or payloads.
pub fn parse_event(frame: &[u8]) -> Result<EventEnvelope, ProtocolError> {
    let raw: RawEvent = strict::typed(strict::decode(frame)?)?;
    let _ = (raw.protocol, raw.channel);
    Ok(EventEnvelope {
        event_id: raw.event_id,
        producer_id: raw.producer_id,
        generation: raw.generation,
        seq: raw.seq,
        run_id: raw.run_id,
        event: EventPayload::decode(raw.kind, raw.payload)?,
    })
}

pub(super) fn unique_bounded<T: Ord>(values: &[T], maximum: usize) -> Result<(), ProtocolError> {
    if values.len() > maximum
        || values
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != values.len()
    {
        Err(ProtocolError::InvalidFrame)
    } else {
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
enum Negotiate {
    #[serde(rename = "negotiate")]
    Negotiate,
}

/// Claims to an existing trusted registration; parsing never creates registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandshake {
    pub producer_id: ProducerId,
    pub generation: ProducerGeneration,
    pub run_id: RunId,
    pub harness: HarnessKind,
    pub requested_signals: Vec<EventKind>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireHandshake {
    protocol: Version,
    channel: EventChannel,
    operation: Negotiate,
    producer_id: ProducerId,
    generation: ProducerGeneration,
    run_id: RunId,
    harness: HarnessKind,
    requested_signals: Vec<EventKind>,
}

impl Serialize for EventHandshake {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        unique_bounded(&self.requested_signals, EventKind::ALL.len())
            .map_err(serde::ser::Error::custom)?;
        WireHandshake {
            protocol: Version,
            channel: EventChannel::Event,
            operation: Negotiate::Negotiate,
            producer_id: self.producer_id.clone(),
            generation: self.generation.clone(),
            run_id: self.run_id.clone(),
            harness: self.harness,
            requested_signals: self.requested_signals.clone(),
        }
        .serialize(serializer)
    }
}

/// Decode bounded negotiation claims for comparison against trusted registration.
/// # Errors
/// Rejects unknown/duplicate signals and malformed or unsupported frames.
pub fn parse_event_handshake(frame: &[u8]) -> Result<EventHandshake, ProtocolError> {
    let raw: WireHandshake = strict::typed(strict::decode(frame)?)?;
    unique_bounded(&raw.requested_signals, EventKind::ALL.len())?;
    Ok(EventHandshake {
        producer_id: raw.producer_id,
        generation: raw.generation,
        run_id: raw.run_id,
        harness: raw.harness,
        requested_signals: raw.requested_signals,
    })
}
