//! One short revision-consistent read; no reader survives a worker request.
use super::{EventReceipt, ReducerContext, StorageError, queries, validation::number};
use crate::{
    domain::{
        ObservationState, ProcessState, ReductionState, RunProjection, TombstoneEvidence, TurnKey,
        TurnState,
    },
    protocol::{EventEnvelope, Revision, TurnId},
};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn context(
    conn: &mut Connection,
    fact: &EventEnvelope,
) -> Result<ReducerContext, StorageError> {
    let tx = conn.transaction()?;
    let revision = queries::revision(&tx)?;
    let producer = queries::producer(&tx, &fact.producer_id)?;
    let receipt = receipt(&tx, fact, producer.as_ref())?;
    let (projection, current_turn) = projection(&tx, &fact.run_id)?;
    let tombstone = fact
        .event
        .turn_id()
        .map(|turn_id| {
            tombstone(
                &tx,
                &TurnKey {
                    producer_id: fact.producer_id.clone(),
                    generation: fact.generation.clone(),
                    run_id: fact.run_id.clone(),
                    turn_id: turn_id.clone(),
                },
            )
        })
        .transpose()?
        .flatten();
    tx.commit()?;
    Ok(ReducerContext {
        revision,
        producer,
        receipt,
        state: ReductionState {
            projection,
            current_turn,
            tombstone,
        },
    })
}

fn receipt(
    conn: &Connection,
    fact: &EventEnvelope,
    producer: Option<&super::ProducerRecord>,
) -> Result<EventReceipt, StorageError> {
    let stored: Option<(Vec<u8>, Vec<u8>)> = conn.query_row("SELECT write_set,revision FROM facts WHERE producer=?1 AND generation=?2 AND (event=?3 OR seq=?4) LIMIT 1", params![fact.producer_id.as_str(),fact.generation.as_str(),fact.event_id.as_str(),number(fact.seq.value()).as_slice()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
    if let Some((identity, revision)) = stored {
        return Ok(if decode_fact(&identity)? == *fact {
            EventReceipt::Exact {
                revision: Revision::new(super::validation::decode_number(revision)?),
            }
        } else {
            EventReceipt::Conflict
        });
    }
    Ok(
        if producer.is_some_and(|record| {
            record.generation == fact.generation
                && record.run_id == fact.run_id
                && record.next_sequence.is_none_or(|next| next > fact.seq)
        }) {
            EventReceipt::PriorSequenceUnverifiable
        } else {
            EventReceipt::Missing
        },
    )
}

pub(super) fn decode_fact(identity: &[u8]) -> Result<EventEnvelope, StorageError> {
    if identity.len() > 24_576 {
        return Err(StorageError::CorruptDatabase);
    }
    let value: serde_json::Value =
        serde_json::from_slice(identity).map_err(|_| StorageError::CorruptDatabase)?;
    let fact = value.get("fact").ok_or(StorageError::CorruptDatabase)?;
    let mut bytes = serde_json::to_vec(fact).map_err(|_| StorageError::CorruptDatabase)?;
    bytes.push(b'\n');
    crate::protocol::parse_event(&bytes).map_err(|_| StorageError::CorruptDatabase)
}

pub(super) fn projection(
    conn: &Connection,
    run: &crate::protocol::RunId,
) -> Result<(RunProjection, Option<TurnKey>), StorageError> {
    let stored = conn.query_row("SELECT process,observation,turn,turn_id,producer,generation FROM projections WHERE run=?1", [run.as_str()], |row| {
        let turn_id: Option<TurnId> = row.get::<_,Option<String>>(3)?.as_deref().map(queries::parse).transpose()?;
        let producer: Option<String> = row.get(4)?;
        let generation: Option<String> = row.get(5)?;
        let current = match (producer, generation, &turn_id) {
            (Some(producer), Some(generation), Some(turn_id)) => Some(TurnKey { producer_id: queries::parse(&producer)?, generation: queries::parse(&generation)?, run_id: run.clone(), turn_id: turn_id.clone() }),
            (None, None, _) => None,
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        Ok((RunProjection { run_id: run.clone(), process: queries::process(row.get(0)?)?, observation: queries::observation(row.get(1)?)?, turn: queries::turn(row.get(2)?)?, turn_id },current))
    }).optional()?;
    Ok(stored.unwrap_or_else(|| {
        (
            RunProjection {
                run_id: run.clone(),
                process: ProcessState::Unknown,
                observation: ObservationState::Stale,
                turn: TurnState::Unknown,
                turn_id: None,
            },
            None,
        )
    }))
}

pub(super) fn tombstone(
    conn: &Connection,
    key: &TurnKey,
) -> Result<Option<TombstoneEvidence>, StorageError> {
    Ok(conn.query_row("SELECT event,terminal FROM tombstones WHERE producer=?1 AND generation=?2 AND run=?3 AND turn_id=?4", params![key.producer_id.as_str(),key.generation.as_str(),key.run_id.as_str(),key.turn_id.as_str()], |row| {
        let terminal = row.get::<_,Option<u8>>(1)?.map(queries::turn).transpose()?;
        if terminal.is_some_and(|state| !state.is_terminal()) { return Err(rusqlite::Error::InvalidQuery); }
        Ok(TombstoneEvidence { key: key.clone(), outcome_id: queries::parse(&row.get::<_,String>(0)?)?, terminal })
    }).optional()?)
}
