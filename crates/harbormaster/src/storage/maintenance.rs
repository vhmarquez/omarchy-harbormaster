use super::{DeliveryState, DeliveryUpdate, Response, ReviewUpdate, StorageError, queries};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) use super::policy::maintain;

pub(super) fn set_delivery(
    conn: &mut Connection,
    update: &DeliveryUpdate,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, update.expected_revision)?;
    let key = &update.outcome;
    let current: Option<(u8, i64)> = tx
        .query_row(
            "SELECT state,changed FROM outbox WHERE producer=?1 AND generation=?2 AND event=?3",
            params![
                key.producer_id.as_str(),
                key.generation.as_str(),
                key.event_id.as_str()
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((state, changed)) = current else {
        return Err(StorageError::Conflict);
    };
    if update.at < changed || state >= 2 || matches!(update.state, DeliveryState::Pending) {
        return Err(StorageError::Conflict);
    }
    tx.execute(
        "UPDATE outbox SET state=?1,changed=?2 WHERE producer=?3 AND generation=?4 AND event=?5",
        params![
            update.state as u8,
            update.at,
            key.producer_id.as_str(),
            key.generation.as_str(),
            key.event_id.as_str()
        ],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::DeliveryUpdated { revision: next })
}

pub(super) fn mark_reviewed(
    conn: &mut Connection,
    update: &ReviewUpdate,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, update.expected_revision)?;
    let key = &update.outcome;
    let changed = tx.execute(
        "UPDATE attention SET reviewed=1 WHERE producer=?1 AND generation=?2 AND event=?3",
        params![
            key.producer_id.as_str(),
            key.generation.as_str(),
            key.event_id.as_str()
        ],
    )?;
    if changed == 0 {
        return Err(StorageError::Conflict);
    }
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Committed {
        revision: next,
        duplicate: false,
    })
}
