use super::{
    AttentionMutation, OutcomeKey, OutcomeRecord, StorageError, WriteSet, queries,
    validation::{decode_number, number},
};
use crate::protocol::Revision;
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn outcome(
    conn: &Connection,
    key: &OutcomeKey,
) -> Result<Option<OutcomeRecord>, StorageError> {
    let args = params![
        key.producer_id.as_str(),
        key.generation.as_str(),
        key.event_id.as_str()
    ];
    let run: Option<String> = conn.query_row("SELECT run FROM attention WHERE producer=?1 AND generation=?2 AND event=?3 UNION SELECT run FROM outbox WHERE producer=?1 AND generation=?2 AND event=?3 UNION SELECT run FROM tombstones WHERE producer=?1 AND generation=?2 AND event=?3 LIMIT 1", args, |row| row.get(0)).optional()?;
    let Some(run) = run else {
        return Ok(None);
    };
    let mut stmt = conn.prepare("SELECT reason,reviewed,resolved FROM attention WHERE producer=?1 AND generation=?2 AND event=?3 ORDER BY reason LIMIT 9")?;
    let mut attention = Vec::new();
    let mut resolved_reasons = Vec::new();
    let mut rows = stmt.query(args)?;
    while let Some(row) = rows.next()? {
        if attention.len() == 8 {
            return Err(StorageError::CorruptDatabase);
        }
        let reason = queries::reason(row.get(0)?)?;
        if row.get::<_, bool>(2)? {
            resolved_reasons.push(reason);
        }
        attention.push(AttentionMutation {
            outcome_id: key.event_id.clone(),
            reason,
            reviewed: row.get(1)?,
        });
    }
    let delivery = conn
        .query_row(
            "SELECT state FROM outbox WHERE producer=?1 AND generation=?2 AND event=?3",
            args,
            |row| queries::delivery(row.get(0)?),
        )
        .optional()?;
    Ok(Some(OutcomeRecord {
        outcome: key.clone(),
        run_id: queries::parse(&run)?,
        attention,
        delivery,
        outcome_revision: outcome_revision(conn, key)?,
        resolved_reasons,
    }))
}
fn outcome_revision(conn: &Connection, key: &OutcomeKey) -> Result<Option<Revision>, StorageError> {
    let mut stmt = conn.prepare("SELECT outcome_revision FROM attention WHERE producer=?1 AND generation=?2 AND event=?3 AND outcome_revision IS NOT NULL UNION SELECT outcome_revision FROM outbox WHERE producer=?1 AND generation=?2 AND event=?3 AND outcome_revision IS NOT NULL UNION SELECT outcome_revision FROM tombstones WHERE producer=?1 AND generation=?2 AND event=?3 AND outcome_revision IS NOT NULL LIMIT 2")?;
    let revisions = stmt
        .query_map(
            params![
                key.producer_id.as_str(),
                key.generation.as_str(),
                key.event_id.as_str()
            ],
            |row| decode_number(row.get(0)?),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if revisions.len() > 1 {
        return Err(StorageError::CorruptDatabase);
    }
    Ok(revisions.first().copied().map(Revision::new))
}
pub(super) fn record_legacy(
    conn: &Connection,
    set: &WriteSet,
    next: Revision,
) -> Result<(), StorageError> {
    for table in ["attention", "outbox", "tombstones"] {
        conn.execute(&format!("UPDATE {table} SET outcome_revision=?1 WHERE producer=?2 AND generation=?3 AND event=?4"), params![number(next.value()).as_slice(),set.fact.producer_id.as_str(),set.fact.generation.as_str(),set.fact.event_id.as_str()])?;
    }
    conn.execute(
        "UPDATE tombstones SET terminal=?1 WHERE producer=?2 AND generation=?3 AND event=?4",
        params![
            super::reducer_validation::terminal(&set.fact).map(|state| state as u8),
            set.fact.producer_id.as_str(),
            set.fact.generation.as_str(),
            set.fact.event_id.as_str()
        ],
    )?;
    Ok(())
}
