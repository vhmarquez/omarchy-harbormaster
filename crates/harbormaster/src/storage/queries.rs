use super::{
    AttentionMutation, AttentionReason, DeliveryState, ObservationState, OutcomeKey, OutcomeRecord,
    ProcessState, ProducerRecord, RunProjection, SnapshotPage, SnapshotQuery, StorageError,
    TurnState, validation::decode_number,
};
use crate::protocol::Revision;
use rusqlite::{Connection, OptionalExtension};

pub(super) fn revision(conn: &Connection) -> Result<Revision, StorageError> {
    Ok(Revision::new(conn.query_row(
        "SELECT revision FROM metadata WHERE id=1",
        [],
        |row| decode_number(row.get(0)?),
    )?))
}
pub(super) fn require_revision(
    conn: &Connection,
    expected: Revision,
) -> Result<Revision, StorageError> {
    if revision(conn)? != expected {
        return Err(StorageError::StaleRevision);
    }
    expected
        .checked_next()
        .ok_or(StorageError::ResourceExhausted)
}
pub(super) fn advance(conn: &Connection, next: Revision) -> Result<(), StorageError> {
    conn.execute(
        "UPDATE metadata SET revision=?1 WHERE id=1",
        [next.value().to_be_bytes().as_slice()],
    )?;
    Ok(())
}
pub(super) fn producer(
    conn: &Connection,
    id: &crate::protocol::ProducerId,
) -> Result<Option<ProducerRecord>, StorageError> {
    Ok(conn
        .query_row(
            "SELECT generation,run,next_seq,active,harness FROM producers WHERE producer=?1",
            [id.as_str()],
            |row| {
                let next: Option<Vec<u8>> = row.get(2)?;
                Ok(ProducerRecord {
                    producer_id: id.clone(),
                    harness: match row.get::<_, u8>(4)? {
                        0 => crate::protocol::HarnessKind::Hermes,
                        1 => crate::protocol::HarnessKind::Claude,
                        2 => crate::protocol::HarnessKind::Codex,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    generation: parse(&row.get::<_, String>(0)?)?,
                    run_id: parse(&row.get::<_, String>(1)?)?,
                    next_sequence: next
                        .map(decode_number)
                        .transpose()?
                        .map(crate::protocol::Seq::new),
                    active: row.get(3)?,
                })
            },
        )
        .optional()?)
}
pub(super) fn snapshot(
    conn: &Connection,
    query: &SnapshotQuery,
) -> Result<SnapshotPage, StorageError> {
    let revision = revision(conn)?;
    if query
        .expected_revision
        .is_some_and(|expected| expected != revision)
    {
        return Err(StorageError::StaleRevision);
    }
    let mut stmt = conn.prepare("SELECT run,process,observation,turn,turn_id FROM projections WHERE run>?1 ORDER BY run LIMIT ?2")?;
    let mut runs = stmt
        .query_map(
            rusqlite::params![
                query
                    .after_run
                    .as_ref()
                    .map_or("", crate::protocol::RunId::as_str),
                i64::from(query.limit) + 1
            ],
            |row| {
                Ok(RunProjection {
                    run_id: parse(&row.get::<_, String>(0)?)?,
                    process: process(row.get(1)?)?,
                    observation: observation(row.get(2)?)?,
                    turn: turn(row.get(3)?)?,
                    turn_id: row
                        .get::<_, Option<String>>(4)?
                        .as_deref()
                        .map(parse)
                        .transpose()?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let next_after = if runs.len() > usize::from(query.limit) {
        runs.pop();
        runs.last().map(|run| run.run_id.clone())
    } else {
        None
    };
    Ok(SnapshotPage {
        revision,
        runs,
        next_after,
    })
}
pub(super) fn outcome(
    conn: &Connection,
    key: &OutcomeKey,
) -> Result<Option<OutcomeRecord>, StorageError> {
    let args = rusqlite::params![
        key.producer_id.as_str(),
        key.generation.as_str(),
        key.event_id.as_str()
    ];
    let run: Option<String> = conn.query_row("SELECT run FROM attention WHERE producer=?1 AND generation=?2 AND event=?3 UNION SELECT run FROM outbox WHERE producer=?1 AND generation=?2 AND event=?3 LIMIT 1", args, |row| row.get(0)).optional()?;
    let Some(run) = run else {
        return Ok(None);
    };
    let mut stmt = conn.prepare("SELECT reason,reviewed FROM attention WHERE producer=?1 AND generation=?2 AND event=?3 ORDER BY reason LIMIT 8")?;
    let attention = stmt
        .query_map(args, |row| {
            Ok(AttentionMutation {
                outcome_id: key.event_id.clone(),
                reason: reason(row.get(0)?)?,
                reviewed: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let delivery = conn
        .query_row(
            "SELECT state FROM outbox WHERE producer=?1 AND generation=?2 AND event=?3",
            args,
            |row| delivery(row.get(0)?),
        )
        .optional()?;
    Ok(Some(OutcomeRecord {
        outcome: key.clone(),
        run_id: parse(&run)?,
        attention,
        delivery,
    }))
}

pub(super) fn parse<T: std::str::FromStr>(value: &str) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}
fn process(value: u8) -> Result<ProcessState, rusqlite::Error> {
    match value {
        0 => Ok(ProcessState::Unknown),
        1 => Ok(ProcessState::Alive),
        2 => Ok(ProcessState::Exited),
        3 => Ok(ProcessState::Zombie),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn observation(value: u8) -> Result<ObservationState, rusqlite::Error> {
    match value {
        0 => Ok(ObservationState::Fresh),
        1 => Ok(ObservationState::Stale),
        2 => Ok(ObservationState::Disconnected),
        3 => Ok(ObservationState::Unsupported),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn turn(value: u8) -> Result<TurnState, rusqlite::Error> {
    match value {
        0 => Ok(TurnState::Unknown),
        1 => Ok(TurnState::Working),
        2 => Ok(TurnState::AwaitingInput),
        3 => Ok(TurnState::AwaitingApproval),
        4 => Ok(TurnState::Completed),
        5 => Ok(TurnState::Failed),
        6 => Ok(TurnState::Interrupted),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn reason(value: u8) -> Result<AttentionReason, rusqlite::Error> {
    match value {
        0 => Ok(AttentionReason::Input),
        1 => Ok(AttentionReason::NativeApproval),
        2 => Ok(AttentionReason::Failure),
        3 => Ok(AttentionReason::ConnectionUncertainty),
        4 => Ok(AttentionReason::Completion),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
pub(super) fn delivery(value: u8) -> Result<DeliveryState, rusqlite::Error> {
    match value {
        0 => Ok(DeliveryState::Pending),
        1 => Ok(DeliveryState::AttemptedUncertain),
        2 => Ok(DeliveryState::Acknowledged),
        3 => Ok(DeliveryState::Expired),
        4 => Ok(DeliveryState::Overflowed),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
