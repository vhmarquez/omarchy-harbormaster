use super::{
    MAX_FACTS, MAX_RUNS, MAX_TOMBSTONES, ReducerWrite, Response, StorageError, attention_state,
    queries, transaction, validation::number,
};
use crate::{
    domain::{RunProjection, TurnKey},
    protocol::{EventEnvelope, Revision},
};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn apply(conn: &mut Connection, set: &ReducerWrite) -> Result<Response, StorageError> {
    let identity = serde_json::to_vec(set).map_err(|_| StorageError::InvalidRequest)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if let Some(revision) = duplicate(&tx, &set.fact, &identity)? {
        return Ok(Response::Committed {
            revision,
            duplicate: true,
        });
    }
    let next = queries::require_revision(&tx, set.expected_revision)?;
    scope(&tx, &set.fact)?;
    transaction::capacity(&tx, "SELECT count(*) FROM facts", MAX_FACTS, 1)?;
    projection(
        &tx,
        &set.effects.projection,
        set.effects.current_turn.as_ref(),
    )?;
    attention_state::apply(&tx, set, next)?;
    tombstone(&tx, set, next)?;
    tx.execute(
        "INSERT INTO facts VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            set.fact.event_id.as_str(),
            set.fact.producer_id.as_str(),
            set.fact.generation.as_str(),
            number(set.fact.seq.value()).as_slice(),
            set.fact.run_id.as_str(),
            set.accepted_at,
            identity,
            number(next.value()).as_slice()
        ],
    )?;
    let next_sequence = set
        .fact
        .seq
        .checked_next()
        .map(|seq| number(seq.value()).to_vec());
    tx.execute("UPDATE producers SET next_seq=?1,active=?2,reconciled=?2 WHERE producer=?3 AND generation=?4", params![next_sequence,!set.effects.reconciliation_required,set.fact.producer_id.as_str(),set.fact.generation.as_str()])?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Committed {
        revision: next,
        duplicate: false,
    })
}
pub(super) fn duplicate(
    conn: &Connection,
    fact: &EventEnvelope,
    identity: &[u8],
) -> Result<Option<Revision>, StorageError> {
    let stored: Option<(Vec<u8>,Vec<u8>)> = conn.query_row("SELECT write_set,revision FROM facts WHERE producer=?1 AND generation=?2 AND (event=?3 OR seq=?4) LIMIT 1", params![fact.producer_id.as_str(),fact.generation.as_str(),fact.event_id.as_str(),number(fact.seq.value()).as_slice()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
    match stored {
        Some((bytes, revision)) if bytes == identity => Ok(Some(Revision::new(
            super::validation::decode_number(revision)?,
        ))),
        Some(_) => Err(StorageError::Conflict),
        None => Ok(None),
    }
}
pub(super) fn scope(conn: &Connection, fact: &EventEnvelope) -> Result<(), StorageError> {
    let record =
        queries::producer(conn, &fact.producer_id)?.ok_or(StorageError::UnknownProducer)?;
    if !record.active
        || !record.reconciled
        || record.generation != fact.generation
        || record.run_id != fact.run_id
    {
        return Err(StorageError::StaleGeneration);
    }
    match record.next_sequence {
        Some(next) if next == fact.seq => Ok(()),
        Some(next) if next < fact.seq => Err(StorageError::SequenceGap),
        _ => Err(StorageError::Conflict),
    }
}
pub(super) fn projection(
    conn: &Connection,
    projection: &RunProjection,
    current: Option<&TurnKey>,
) -> Result<(), StorageError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projections WHERE run=?1)",
        [projection.run_id.as_str()],
        |row| row.get(0),
    )?;
    if !exists {
        transaction::capacity(conn, "SELECT count(*) FROM projections", MAX_RUNS, 1)?;
    }
    conn.execute("INSERT INTO projections VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(run) DO UPDATE SET process=excluded.process,observation=excluded.observation,turn=excluded.turn,turn_id=excluded.turn_id,producer=excluded.producer,generation=excluded.generation", params![projection.run_id.as_str(),projection.process as u8,projection.observation as u8,projection.turn as u8,projection.turn_id.as_ref().map(crate::protocol::TurnId::as_str),current.map(|key| key.producer_id.as_str()),current.map(|key| key.generation.as_str())])?;
    Ok(())
}
fn tombstone(conn: &Connection, set: &ReducerWrite, next: Revision) -> Result<(), StorageError> {
    let Some(item) = &set.effects.tombstone else {
        return Ok(());
    };
    if super::context::tombstone(conn, &item.key)?.is_some() {
        return Err(StorageError::Conflict);
    }
    transaction::capacity(conn, "SELECT count(*) FROM tombstones", MAX_TOMBSTONES, 1)?;
    conn.execute(
        "INSERT INTO tombstones VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            item.key.producer_id.as_str(),
            item.key.generation.as_str(),
            item.key.run_id.as_str(),
            item.key.turn_id.as_str(),
            item.outcome_id.as_str(),
            set.accepted_at,
            item.terminal.map(|state| state as u8),
            number(next.value()).as_slice()
        ],
    )?;
    Ok(())
}
