use super::{
    Reconciliation, ReconciliationBaseline, Response, StorageError, queries, reducer_transaction,
    transaction,
    validation::{decode_number, number},
};
use crate::domain::{ObservationState, ProcessState, RunProjection, TurnKey, TurnState};
use rusqlite::Connection;

pub(super) fn reconcile(
    conn: &mut Connection,
    request: &Reconciliation,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    queries::require_revision(&tx, request.registration.expected_revision)?;
    let discarded = discarded_next(&tx, request.discarded_events)?;
    let verified = matches!(request.baseline, ReconciliationBaseline::Verified { .. });
    resolve_old(&tx, request)?;
    let (generation, next) =
        transaction::registration_inside(&tx, &request.registration, verified, verified)?;
    let (projection, current) = match &request.baseline {
        ReconciliationBaseline::Unavailable => (
            RunProjection {
                run_id: request.registration.run_id.clone(),
                process: ProcessState::Unknown,
                observation: ObservationState::Stale,
                turn: TurnState::Unknown,
                turn_id: None,
            },
            None,
        ),
        ReconciliationBaseline::Verified {
            projection,
            current_turn,
        } => (
            projection.clone(),
            current_turn.as_ref().map(|turn_id| TurnKey {
                producer_id: request.registration.producer_id.clone(),
                generation: generation.clone(),
                run_id: request.registration.run_id.clone(),
                turn_id: turn_id.clone(),
            }),
        ),
    };
    reducer_transaction::projection(&tx, &projection, current.as_ref())?;
    tx.execute(
        "UPDATE metadata SET discarded=?1 WHERE id=1",
        [number(discarded).as_slice()],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Registered {
        generation,
        revision: next,
    })
}

pub(super) fn retire(
    conn: &mut Connection,
    request: &super::ProducerRetirement,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let next = queries::require_revision(&tx, request.expected_revision)?;
    let record =
        queries::producer(&tx, &request.producer_id)?.ok_or(StorageError::UnknownProducer)?;
    if record.generation != request.generation || !record.active {
        return Err(StorageError::StaleGeneration);
    }
    let discarded = discarded_next(&tx, request.discarded_events)?;
    let (mut projection, current) = super::context::projection(&tx, &record.run_id)?;
    if current.as_ref().is_some_and(|key| {
        key.producer_id != record.producer_id || key.generation != record.generation
    }) {
        return Err(StorageError::Conflict);
    }
    projection.observation = ObservationState::Stale;
    if !projection.turn.is_terminal() {
        projection.turn = TurnState::Unknown;
    }
    reducer_transaction::projection(&tx, &projection, current.as_ref())?;
    tx.execute(
        "UPDATE producers SET active=0,reconciled=0 WHERE producer=?1 AND generation=?2",
        rusqlite::params![record.producer_id.as_str(), record.generation.as_str()],
    )?;
    tx.execute(
        "UPDATE metadata SET discarded=?1 WHERE id=1",
        [number(discarded).as_slice()],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Retired { revision: next })
}

fn discarded_next(conn: &Connection, adding: u64) -> Result<u64, StorageError> {
    conn.query_row("SELECT discarded FROM metadata WHERE id=1", [], |row| {
        decode_number(row.get(0)?)
    })?
    .checked_add(adding)
    .ok_or(StorageError::ResourceExhausted)
}

fn resolve_old(conn: &Connection, request: &Reconciliation) -> Result<(), StorageError> {
    let Some(key) = &request.resolved_turn else {
        return Ok(());
    };
    let (_, current) = super::context::projection(conn, &request.registration.run_id)?;
    if current.as_ref() != Some(key)
        || key.producer_id != request.registration.producer_id
        || request.registration.previous_generation.as_ref() != Some(&key.generation)
    {
        return Err(StorageError::Conflict);
    }
    conn.execute("UPDATE attention SET resolved=1 WHERE producer=?1 AND generation=?2 AND run=?3 AND turn_id=?4 AND scoped=1 AND resolved=0 AND reason IN(0,1,3)", rusqlite::params![key.producer_id.as_str(),key.generation.as_str(),key.run_id.as_str(),key.turn_id.as_str()])?;
    Ok(())
}
