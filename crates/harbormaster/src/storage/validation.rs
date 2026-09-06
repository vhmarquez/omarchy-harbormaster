use super::{PAGE_LIMIT, Request, StorageError, WriteSet};
use crate::protocol::encode_frame;
use std::collections::BTreeSet;

impl Request {
    /// Validate bounded shape before queueing; database-dependent authorization,
    /// scope, sequence and revision checks still occur within the transaction.
    /// # Errors
    /// Invalid timestamps, page cursors, oversized or inconsistent write sets.
    pub fn validate(&self) -> Result<(), StorageError> {
        match self {
            Self::Catalog(request) => request.validate().map_err(StorageError::from),
            Self::Commit(set) => validate_set(set),
            Self::Context(fact) => encode_frame(fact)
                .map(|_| ())
                .map_err(|_| StorageError::InvalidRequest),
            Self::Apply(set) => super::reducer_validation::apply(set),
            Self::Reconcile(request) => super::reducer_validation::reconciliation(request),
            Self::CleanupPreview { now } if *now < 0 => Err(StorageError::InvalidRequest),
            Self::Snapshot(query)
                if query.limit == 0
                    || query.limit > PAGE_LIMIT
                    || (query.after_run.is_some() && query.expected_revision.is_none()) =>
            {
                Err(StorageError::InvalidRequest)
            }
            Self::Maintain { now, .. } if *now < 0 => Err(StorageError::InvalidRequest),
            Self::SetDelivery(update)
                if update.at < 0 || matches!(update.state, super::DeliveryState::Pending) =>
            {
                Err(StorageError::InvalidRequest)
            }
            _ => Ok(()),
        }
    }
}

fn validate_set(set: &WriteSet) -> Result<(), StorageError> {
    if set.accepted_at < 0
        || set.attention.len() > 8
        || set.projection.run_id != set.fact.run_id
        || set
            .attention
            .iter()
            .any(|item| item.outcome_id != set.fact.event_id)
        || set
            .outbox
            .as_ref()
            .is_some_and(|item| item.outcome_id != set.fact.event_id)
        || set.tombstone.as_ref().is_some_and(|item| {
            item.outcome_id != set.fact.event_id
                || set.projection.turn_id.as_ref() != Some(&item.turn_id)
        })
        || set
            .attention
            .iter()
            .map(|item| format!("{:?}", item.reason))
            .collect::<BTreeSet<_>>()
            .len()
            != set.attention.len()
    {
        return Err(StorageError::InvalidRequest);
    }
    encode_frame(&set.fact).map_err(|_| StorageError::InvalidRequest)?;
    let bytes = serde_json::to_vec(set).map_err(|_| StorageError::InvalidRequest)?;
    if bytes.len() > 24_576 {
        return Err(StorageError::InvalidRequest);
    }
    Ok(())
}

pub(super) fn number(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}
pub(super) fn decode_number(value: Vec<u8>) -> Result<u64, rusqlite::Error> {
    let bytes: [u8; 8] = value
        .try_into()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(u64::from_be_bytes(bytes))
}
