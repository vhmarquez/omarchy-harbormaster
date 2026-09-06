//! Passive observer retirement preserves independent process and outcome facts.
use super::StorageError;
use crate::protocol::{ProducerGeneration, ProducerId};
use rusqlite::{Connection, params};

pub(super) fn one(
    conn: &Connection,
    producer: &ProducerId,
    generation: &ProducerGeneration,
) -> Result<(), StorageError> {
    degrade(
        conn,
        Some(producer.as_str()),
        Some(generation.as_str()),
        false,
    )?;
    conn.execute("UPDATE producers SET active=0,reconciled=0 WHERE producer=?1 AND generation=?2 AND active=1",params![producer.as_str(),generation.as_str()])?;
    Ok(())
}
pub(super) fn all(conn: &Connection) -> Result<(), StorageError> {
    degrade(conn, None, None, false)?;
    conn.execute(
        "UPDATE producers SET active=0,reconciled=0 WHERE active=1",
        [],
    )?;
    Ok(())
}
pub(super) fn legacy(conn: &Connection) -> Result<(), StorageError> {
    // Recognized old projections never contained proven current scope, even
    // when a previous opener already retired their producer registration.
    degrade(conn, None, None, true)
}
fn degrade(
    conn: &Connection,
    producer: Option<&str>,
    generation: Option<&str>,
    legacy: bool,
) -> Result<(), StorageError> {
    conn.execute("UPDATE projections AS r SET observation=1,turn=CASE WHEN turn IN(4,5,6) THEN turn ELSE 0 END WHERE (?3 AND r.producer IS NULL AND r.generation IS NULL) OR EXISTS(SELECT 1 FROM producers p WHERE p.active=1 AND p.run=r.run AND (?1 IS NULL OR (p.producer=?1 AND p.generation=?2)) AND ((r.producer=p.producer AND r.generation=p.generation) OR (r.producer IS NULL AND r.generation IS NULL)))",params![producer,generation,legacy])?;
    Ok(())
}
