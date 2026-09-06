//! Explicit capability/freshness inputs; no global timeout or inferred health.
use super::ObservationState;
use crate::protocol::EventKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    InvalidEvidence,
    InvalidScope,
    UnsupportedSignal,
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEvidence => "invalid_evidence",
            Self::InvalidScope => "invalid_scope",
            Self::UnsupportedSignal => "unsupported_signal",
        })
    }
}

impl std::error::Error for DomainError {}

/// Trusted observation policy, never widened by a producer health claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationPolicy {
    supported: [bool; 9],
    freshness: ObservationState,
}

impl ObservationPolicy {
    /// # Errors
    /// Rejects duplicate or excessive signals; absence means unsupported.
    pub fn new(signals: &[EventKind], freshness: ObservationState) -> Result<Self, DomainError> {
        let mut supported = [false; 9];
        if signals.len() > supported.len() {
            return Err(DomainError::InvalidEvidence);
        }
        for signal in signals {
            let position = EventKind::ALL
                .iter()
                .position(|value| value == signal)
                .ok_or(DomainError::InvalidEvidence)?;
            if std::mem::replace(&mut supported[position], true) {
                return Err(DomainError::InvalidEvidence);
            }
        }
        Ok(Self {
            supported,
            freshness,
        })
    }

    #[must_use]
    pub fn supports(&self, signal: EventKind) -> bool {
        EventKind::ALL
            .iter()
            .position(|value| *value == signal)
            .is_some_and(|position| self.supported[position])
    }

    #[must_use]
    pub const fn freshness(&self) -> ObservationState {
        self.freshness
    }
}
