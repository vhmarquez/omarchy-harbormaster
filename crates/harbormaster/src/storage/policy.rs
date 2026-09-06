//! Read-only bounded plans; explicit policy and cleanup mutations share revisions.
use super::{
    CleanupPreview, DAY, HistoryRetention, MAX_FACTS, MAX_TOMBSTONES, MaintenanceResult, Response,
    StorageError, StoragePolicy, StorageStatus, queries, validation::decode_number,
};
use crate::protocol::Revision;
use rusqlite::{Connection, params};

pub(super) fn policy(conn: &Connection) -> Result<StoragePolicy, StorageError> {
    let days: u16 = conn.query_row("SELECT history_days FROM metadata WHERE id=1", [], |row| {
        row.get(0)
    })?;
    Ok(StoragePolicy {
        revision: queries::revision(conn)?,
        history: HistoryRetention::from_days(days)?,
    })
}
pub(super) fn update(
    conn: &mut Connection,
    expected: Revision,
    history: HistoryRetention,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, expected)?;
    tx.execute(
        "UPDATE metadata SET history_days=?1 WHERE id=1",
        [history.days()],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Policy(StoragePolicy {
        revision: next,
        history,
    }))
}
pub(super) fn status(conn: &Connection) -> Result<StorageStatus, StorageError> {
    Ok(StorageStatus {
        revision: queries::revision(conn)?,
        facts: count(conn, "SELECT count(*) FROM facts")?,
        tombstones: count(conn, "SELECT count(*) FROM tombstones")?,
        attention: count(conn, "SELECT count(*) FROM attention")?,
        pending_deliveries: count(conn, "SELECT count(*) FROM outbox WHERE state IN(0,1)")?,
        discarded_events: conn.query_row("SELECT discarded FROM metadata WHERE id=1", [], |row| decode_number(row.get(0)?))?,
        cleanup: conn.query_row("SELECT facts_removed,tombstones_removed,generations_retired,deliveries_expired FROM maintenance WHERE id=1", [], |row| Ok(MaintenanceResult { facts_removed: row.get(0)?, tombstones_removed: row.get(1)?, generations_retired: row.get(2)?, deliveries_expired: row.get(3)?, more: false }))?,
    })
}
pub(super) fn preview(
    conn: &Connection,
    now: i64,
    owner: &std::sync::Arc<()>,
) -> Result<CleanupPreview, StorageError> {
    let policy = policy(conn)?;
    let facts = candidates(
        conn,
        "SELECT rowid FROM facts WHERE accepted<=?1 OR ?2 ORDER BY accepted,rowid LIMIT 100",
        now.saturating_sub(policy.history.days() * DAY),
        u64::from(count(conn, "SELECT count(*) FROM facts")?) >= MAX_FACTS as u64,
    )?;
    let tombstones = candidates(
        conn,
        "SELECT rowid FROM tombstones WHERE created<=?1 OR ?2 ORDER BY created,rowid LIMIT 100",
        now.saturating_sub(30 * DAY),
        u64::from(count(conn, "SELECT count(*) FROM tombstones")?) >= MAX_TOMBSTONES as u64,
    )?;
    let deliveries = candidates(
        conn,
        "SELECT rowid FROM outbox WHERE state IN(0,1) AND created<=?1 AND ?2 ORDER BY created,rowid LIMIT 100",
        now.saturating_sub(7 * DAY),
        true,
    )?;
    let mut generations = std::collections::BTreeSet::new();
    for rowid in &tombstones {
        let retired: Option<String> = conn.query_row("SELECT p.producer FROM tombstones t LEFT JOIN producers p ON p.producer=t.producer AND p.generation=t.generation AND p.active=1 WHERE t.rowid=?1", [rowid], |row| row.get(0))?;
        if let Some(producer) = retired {
            generations.insert(producer);
        }
    }
    let result = MaintenanceResult {
        facts_removed: bounded(facts.len())?,
        tombstones_removed: bounded(tombstones.len())?,
        generations_retired: bounded(generations.len())?,
        deliveries_expired: bounded(deliveries.len())?,
        more: facts.len() == 100 || tombstones.len() == 100 || deliveries.len() == 100,
    };
    Ok(CleanupPreview {
        owner: std::sync::Arc::clone(owner),
        revision: policy.revision,
        now,
        history: policy.history,
        facts,
        tombstones,
        deliveries,
        result,
    })
}
pub(super) fn apply(
    conn: &mut Connection,
    expected: &CleanupPreview,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, expected.revision)?;
    let actual = preview(&tx, expected.now, &expected.owner)?;
    if actual != *expected {
        return Err(StorageError::Conflict);
    }
    execute(&tx, &actual)?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Maintained {
        revision: next,
        result: actual.result,
    })
}
pub(super) fn maintain(
    conn: &mut Connection,
    expected: Revision,
    now: i64,
    history: HistoryRetention,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, expected)?;
    tx.execute(
        "UPDATE metadata SET history_days=?1 WHERE id=1",
        [history.days()],
    )?;
    let plan = preview(&tx, now, &std::sync::Arc::new(()))?;
    execute(&tx, &plan)?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Maintained {
        revision: next,
        result: plan.result,
    })
}
fn execute(conn: &Connection, plan: &CleanupPreview) -> Result<(), StorageError> {
    for rowid in &plan.facts {
        conn.execute("DELETE FROM facts WHERE rowid=?1", [rowid])?;
    }
    for rowid in &plan.tombstones {
        conn.execute("UPDATE producers SET active=0,reconciled=0 WHERE active=1 AND EXISTS(SELECT 1 FROM tombstones t WHERE t.rowid=?1 AND t.producer=producers.producer AND t.generation=producers.generation)", [rowid])?;
        conn.execute("DELETE FROM tombstones WHERE rowid=?1", [rowid])?;
    }
    for rowid in &plan.deliveries {
        conn.execute(
            "UPDATE outbox SET state=3,changed=?1 WHERE rowid=?2",
            params![plan.now, rowid],
        )?;
    }
    let result = plan.result;
    conn.execute("UPDATE maintenance SET facts_removed=facts_removed+?1,tombstones_removed=tombstones_removed+?2,generations_retired=generations_retired+?3,deliveries_expired=deliveries_expired+?4 WHERE id=1", params![result.facts_removed,result.tombstones_removed,result.generations_retired,result.deliveries_expired])?;
    Ok(())
}
fn candidates(
    conn: &Connection,
    sql: &'static str,
    before: i64,
    include: bool,
) -> Result<Vec<i64>, StorageError> {
    Ok(conn
        .prepare(sql)?
        .query_map(params![before, include], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?)
}
fn count(conn: &Connection, sql: &'static str) -> Result<u32, StorageError> {
    Ok(conn.query_row(sql, [], |row| row.get(0))?)
}
fn bounded(count: usize) -> Result<u32, StorageError> {
    u32::try_from(count).map_err(|_| StorageError::ResourceExhausted)
}
