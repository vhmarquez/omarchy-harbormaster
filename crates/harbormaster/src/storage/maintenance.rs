use super::{queries, *};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn maintain(
    conn: &mut Connection,
    expected: crate::protocol::Revision,
    now: i64,
    history: HistoryRetention,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, expected)?;
    let facts_removed = tx.execute("DELETE FROM facts WHERE rowid IN(SELECT rowid FROM facts WHERE accepted<=?1 ORDER BY accepted,rowid LIMIT 100)", [now.saturating_sub(history.days() * DAY)])?;
    let (tombstones_removed, generations_retired) = tombstones(&tx, now)?;
    let deliveries_expired = tx.execute("UPDATE outbox SET state=3,changed=?1 WHERE rowid IN(SELECT rowid FROM outbox WHERE state IN(0,1) AND created<=?2 ORDER BY created,rowid LIMIT 100)", params![now, now.saturating_sub(7 * DAY)])?;
    tx.execute(
        "UPDATE metadata SET history_days=?1 WHERE id=1",
        [history.days()],
    )?;
    tx.execute("UPDATE maintenance SET facts_removed=facts_removed+?1,tombstones_removed=tombstones_removed+?2,generations_retired=generations_retired+?3,deliveries_expired=deliveries_expired+?4 WHERE id=1", params![bounded_count(facts_removed)?, bounded_count(tombstones_removed)?, bounded_count(generations_retired)?, bounded_count(deliveries_expired)?])?;
    let result = MaintenanceResult {
        facts_removed: bounded_count(facts_removed)?,
        tombstones_removed: bounded_count(tombstones_removed)?,
        generations_retired: bounded_count(generations_retired)?,
        deliveries_expired: bounded_count(deliveries_expired)?,
        more: facts_removed == 100 || tombstones_removed == 100 || deliveries_expired == 100,
    };
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Maintained {
        revision: next,
        result,
    })
}

fn tombstones(conn: &Connection, now: i64) -> Result<(usize, usize), StorageError> {
    let count: u32 = conn.query_row("SELECT count(*) FROM tombstones", [], |row| row.get(0))?;
    let mut stmt = conn.prepare("SELECT rowid,producer,generation FROM tombstones WHERE created<=?1 OR ?2 ORDER BY created,rowid LIMIT 100")?;
    let candidates = stmt
        .query_map(
            params![
                now.saturating_sub(30 * DAY),
                u64::from(count) >= MAX_TOMBSTONES as u64
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut retired = 0;
    for (rowid, producer, generation) in &candidates {
        retired += conn.execute(
            "UPDATE producers SET active=0 WHERE producer=?1 AND generation=?2 AND active=1",
            params![producer, generation],
        )?;
        conn.execute("DELETE FROM tombstones WHERE rowid=?1", [rowid])?;
    }
    Ok((candidates.len(), retired))
}

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

fn bounded_count(count: usize) -> Result<u32, StorageError> {
    u32::try_from(count).map_err(|_| StorageError::ResourceExhausted)
}
