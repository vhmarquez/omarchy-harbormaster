//! Coalesced active transient reasons and independent human review.
use super::{
    MAX_ATTENTION, MAX_OUTBOX_AUDIT, MAX_OUTBOX_PENDING, OutcomeReview, ReducerWrite, Response,
    StorageError, queries, transaction::capacity, validation::number,
};
use crate::{domain::AttentionReason, protocol::Revision};
use rusqlite::{Connection, params};

pub(super) fn apply(
    conn: &Connection,
    set: &ReducerWrite,
    next: Revision,
) -> Result<(), StorageError> {
    if let Some(key) = &set.effects.resolve_transient {
        // The partial unique index proves at most three matching active rows.
        conn.execute("UPDATE attention SET resolved=1 WHERE producer=?1 AND generation=?2 AND run=?3 AND turn_id=?4 AND scoped=1 AND resolved=0 AND reason IN(0,1,3)", params![key.producer_id.as_str(),key.generation.as_str(),key.run_id.as_str(),key.turn_id.as_str()])?;
    }
    let mut inserted = false;
    for reason in &set.effects.attention {
        inserted |= ensure(conn, set, *reason, next)?;
    }
    if set.notify && inserted {
        capacity(conn, "SELECT count(*) FROM outbox", MAX_OUTBOX_AUDIT, 1)?;
        capacity(
            conn,
            "SELECT count(*) FROM outbox WHERE state IN(0,1)",
            MAX_OUTBOX_PENDING,
            1,
        )?;
        conn.execute(
            "INSERT INTO outbox VALUES(?1,?2,?3,?4,0,?5,?5,?6)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                set.fact.event_id.as_str(),
                set.fact.run_id.as_str(),
                set.accepted_at,
                number(next.value()).as_slice()
            ],
        )?;
    }
    Ok(())
}
fn ensure(
    conn: &Connection,
    set: &ReducerWrite,
    reason: AttentionReason,
    next: Revision,
) -> Result<bool, StorageError> {
    let turn = set
        .fact
        .event
        .turn_id()
        .map(crate::protocol::TurnId::as_str);
    if matches!(
        reason,
        AttentionReason::Input
            | AttentionReason::NativeApproval
            | AttentionReason::ConnectionUncertainty
    ) {
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM attention WHERE producer=?1 AND generation=?2 AND run=?3 AND turn_id IS ?4 AND reason=?5 AND scoped=1 AND resolved=0)", params![set.fact.producer_id.as_str(),set.fact.generation.as_str(),set.fact.run_id.as_str(),turn,reason as u8], |row| row.get(0))?;
        if exists {
            return Ok(false);
        }
    }
    capacity(conn, "SELECT count(*) FROM attention", MAX_ATTENTION, 1)?;
    conn.execute(
        "INSERT INTO attention VALUES(?1,?2,?3,?4,?5,0,?6,0,?7,1)",
        params![
            set.fact.producer_id.as_str(),
            set.fact.generation.as_str(),
            set.fact.event_id.as_str(),
            set.fact.run_id.as_str(),
            reason as u8,
            turn,
            number(next.value()).as_slice()
        ],
    )?;
    Ok(true)
}
pub(super) fn review(
    conn: &mut Connection,
    update: &OutcomeReview,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, update.expected_revision)?;
    let outcome = queries::outcome(&tx, &update.outcome)?.ok_or(StorageError::Conflict)?;
    if outcome.outcome_revision != Some(update.outcome_revision) || outcome.attention.is_empty() {
        return Err(StorageError::Conflict);
    }
    tx.execute(
        "UPDATE attention SET reviewed=1 WHERE producer=?1 AND generation=?2 AND event=?3",
        params![
            update.outcome.producer_id.as_str(),
            update.outcome.generation.as_str(),
            update.outcome.event_id.as_str()
        ],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Committed {
        revision: next,
        duplicate: false,
    })
}
