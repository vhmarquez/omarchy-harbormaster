use super::{
    MAX_ATTENTION, MAX_FACTS, MAX_OUTBOX_AUDIT, MAX_OUTBOX_PENDING, MAX_RUNS, MAX_TOMBSTONES,
    Registration, Response, StorageError, WriteSet, queries, validation::number,
};
use crate::protocol::Revision;
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn register(
    conn: &mut Connection,
    registration: &Registration,
) -> Result<Response, StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let (generation, next) = registration_inside(&tx, registration, true, false)?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Registered {
        generation,
        revision: next,
    })
}

pub(super) fn registration_inside(
    conn: &Connection,
    registration: &Registration,
    active: bool,
    reconciled: bool,
) -> Result<(crate::protocol::ProducerGeneration, Revision), StorageError> {
    let next = queries::require_revision(conn, registration.expected_revision)?;
    let previous = queries::producer(conn, &registration.producer_id)?;
    if previous.as_ref().map(|record| &record.generation)
        != registration.previous_generation.as_ref()
        || previous.as_ref().is_some_and(|record| {
            record.run_id != registration.run_id || record.harness != registration.harness
        })
    {
        return Err(StorageError::StaleGeneration);
    }
    let conflict: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM producers WHERE run=?1 AND active=1 AND producer<>?2)",
        params![
            registration.run_id.as_str(),
            registration.producer_id.as_str()
        ],
        |row| row.get(0),
    )?;
    if conflict {
        return Err(StorageError::Conflict);
    }
    if previous.is_none() {
        capacity(conn, "SELECT count(*) FROM producers", MAX_RUNS, 1)?;
    }
    let generation =
        crate::generation::fresh().map_err(|_| StorageError::PersistenceUnavailable)?;
    conn.execute("INSERT INTO producers VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(producer) DO UPDATE SET generation=excluded.generation,next_seq=excluded.next_seq,active=excluded.active,reconciled=excluded.reconciled", params![registration.producer_id.as_str(),generation.as_str(),registration.run_id.as_str(),registration.harness as u8,number(registration.next_sequence.value()).as_slice(),active,reconciled])?;
    Ok((generation, next))
}

pub(super) fn commit(conn: &mut Connection, set: &WriteSet) -> Result<Response, StorageError> {
    let identity = serde_json::to_vec(set).map_err(|_| StorageError::InvalidRequest)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if let Some(revision) = duplicate(&tx, set, &identity)? {
        return Ok(Response::Committed {
            revision,
            duplicate: true,
        });
    }
    let next = queries::require_revision(&tx, set.expected_revision)?;
    scope(&tx, set)?;
    capacity(&tx, "SELECT count(*) FROM facts", MAX_FACTS, 1)?;
    insert_projection(&tx, set)?;
    insert_attention(&tx, set)?;
    insert_outbox(&tx, set)?;
    insert_tombstone(&tx, set)?;
    super::outcomes::record_legacy(&tx, set, next)?;
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
    tx.execute(
        "UPDATE producers SET next_seq=?1,reconciled=0 WHERE producer=?2",
        params![next_sequence, set.fact.producer_id.as_str()],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(Response::Committed {
        revision: next,
        duplicate: false,
    })
}

fn duplicate(
    conn: &Connection,
    set: &WriteSet,
    identity: &[u8],
) -> Result<Option<Revision>, StorageError> {
    let stored: Option<(Vec<u8>, Vec<u8>)> = conn.query_row("SELECT write_set,revision FROM facts WHERE producer=?2 AND generation=?3 AND (event=?1 OR seq=?4) LIMIT 1", params![set.fact.event_id.as_str(), set.fact.producer_id.as_str(), set.fact.generation.as_str(), number(set.fact.seq.value()).as_slice()], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
    match stored {
        Some((bytes, revision)) if bytes == identity => Ok(Some(Revision::new(
            super::validation::decode_number(revision)?,
        ))),
        Some(_) => Err(StorageError::Conflict),
        None => Ok(None),
    }
}
fn scope(conn: &Connection, set: &WriteSet) -> Result<(), StorageError> {
    let record =
        queries::producer(conn, &set.fact.producer_id)?.ok_or(StorageError::UnknownProducer)?;
    if !record.active
        || record.generation != set.fact.generation
        || record.run_id != set.fact.run_id
    {
        return Err(StorageError::StaleGeneration);
    }
    match record.next_sequence {
        Some(next) if next == set.fact.seq => Ok(()),
        Some(next) if next < set.fact.seq => Err(StorageError::SequenceGap),
        _ => Err(StorageError::Conflict),
    }
}
fn insert_projection(conn: &Connection, set: &WriteSet) -> Result<(), StorageError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projections WHERE run=?1)",
        [set.fact.run_id.as_str()],
        |row| row.get(0),
    )?;
    if !exists {
        capacity(conn, "SELECT count(*) FROM projections", MAX_RUNS, 1)?;
    }
    conn.execute("INSERT INTO projections(run,process,observation,turn,turn_id) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(run) DO UPDATE SET process=excluded.process,observation=excluded.observation,turn=excluded.turn,turn_id=excluded.turn_id", params![set.fact.run_id.as_str(), set.projection.process as u8, set.projection.observation as u8, set.projection.turn as u8, set.projection.turn_id.as_ref().map(crate::protocol::TurnId::as_str)])?;
    Ok(())
}
fn insert_attention(conn: &Connection, set: &WriteSet) -> Result<(), StorageError> {
    capacity(
        conn,
        "SELECT count(*) FROM attention",
        MAX_ATTENTION,
        set.attention.len(),
    )?;
    for item in &set.attention {
        conn.execute(
            "INSERT INTO attention(producer,generation,event,run,reason,reviewed) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                item.outcome_id.as_str(),
                set.fact.run_id.as_str(),
                item.reason as u8,
                item.reviewed
            ],
        )?;
    }
    Ok(())
}
fn insert_outbox(conn: &Connection, set: &WriteSet) -> Result<(), StorageError> {
    if let Some(intent) = &set.outbox {
        capacity(conn, "SELECT count(*) FROM outbox", MAX_OUTBOX_AUDIT, 1)?;
        capacity(
            conn,
            "SELECT count(*) FROM outbox WHERE state IN(0,1)",
            MAX_OUTBOX_PENDING,
            1,
        )?;
        conn.execute(
            "INSERT INTO outbox(producer,generation,event,run,state,created,changed) VALUES(?1,?2,?3,?4,0,?5,?5)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                intent.outcome_id.as_str(),
                set.fact.run_id.as_str(),
                set.accepted_at
            ],
        )?;
    }
    Ok(())
}
fn insert_tombstone(conn: &Connection, set: &WriteSet) -> Result<(), StorageError> {
    if let Some(item) = &set.tombstone {
        // Full protection is retained until the manager explicitly performs a
        // bounded maintenance transaction that retires affected generations.
        capacity(conn, "SELECT count(*) FROM tombstones", MAX_TOMBSTONES, 1)?;
        conn.execute(
            "INSERT INTO tombstones(producer,generation,run,turn_id,event,created) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                set.fact.run_id.as_str(),
                item.turn_id.as_str(),
                item.outcome_id.as_str(),
                set.accepted_at
            ],
        )?;
    }
    Ok(())
}
pub(super) fn capacity(
    conn: &Connection,
    query: &'static str,
    maximum: usize,
    adding: usize,
) -> Result<(), StorageError> {
    let count = usize::try_from(conn.query_row(query, [], |row| row.get::<_, u32>(0))?)
        .map_err(|_| StorageError::ResourceExhausted)?;
    if count
        .checked_add(adding)
        .is_none_or(|total| total > maximum)
    {
        return Err(StorageError::ResourceExhausted);
    }
    Ok(())
}
