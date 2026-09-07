//! Known V1/V2 upgrades preserve legacy receipt bytes and unprovable evidence.
use super::{StorageError, schema, schema_layout};
use rusqlite::{Connection, params};

pub(super) fn migrate(conn: &mut Connection) -> Result<(), StorageError> {
    let version = schema::inspect(conn)?;
    let tx = conn.transaction()?;
    let next = super::queries::revision(&tx)?
        .checked_next()
        .ok_or(StorageError::ResourceExhausted)?;
    if version < 3 {
        for (name, sql) in schema_layout::version_three() {
            if name == "attention_active_scope" {
                tx.execute_batch(&sql)?;
                continue;
            }
            if name == "maintenance" && version == 1 {
                tx.execute_batch(&sql)?;
                tx.execute_batch("INSERT INTO maintenance VALUES(1,0,0,0,0)")?;
                continue;
            }
            let columns = match name {
                "producers" => "producer,generation,run,harness,next_seq,active",
                "metadata" => "id,revision,history_days",
                "projections" => "run,process,observation,turn,turn_id",
                "attention" => "producer,generation,event,run,reason,reviewed",
                "outbox" => "producer,generation,event,run,state,created,changed",
                "tombstones" => "producer,generation,run,turn_id,event,created",
                _ => continue,
            };
            // Names/columns come only from these fixed manager-owned literals.
            tx.execute_batch(&format!("ALTER TABLE {name} RENAME TO legacy_{name};{sql};INSERT INTO {name}({columns}) SELECT {columns} FROM legacy_{name};DROP TABLE legacy_{name}"))?;
        }
        backfill(&tx)?;
        super::retirement::all(&tx)?;
        super::retirement::legacy(&tx)?;
    }
    if version < 4 {
        for (_, sql) in super::catalog::TABLES {
            tx.execute_batch(sql)?;
        }
    }
    tx.execute_batch(super::runners::TABLE.1)?;
    super::queries::advance(&tx, next)?;
    tx.execute_batch("PRAGMA user_version=5")?;
    tx.commit()?;
    Ok(())
}

fn backfill(conn: &Connection) -> Result<(), StorageError> {
    for table in ["attention", "outbox", "tombstones"] {
        conn.execute_batch(&format!("UPDATE {table} SET outcome_revision=(SELECT revision FROM facts WHERE facts.producer={table}.producer AND facts.generation={table}.generation AND facts.event={table}.event)"))?;
    }
    let mut stmt = conn.prepare("SELECT t.producer,t.generation,t.run,t.turn_id,f.write_set FROM tombstones t JOIN facts f ON f.producer=t.producer AND f.generation=t.generation AND f.event=t.event LIMIT 20001")?;
    let mut rows = stmt.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        count += 1;
        if count > super::MAX_TOMBSTONES {
            return Err(StorageError::ResourceExhausted);
        }
        let identity: Vec<u8> = row.get(4)?;
        let fact = super::context::decode_fact(&identity)?;
        let run: String = row.get(2)?;
        let turn: String = row.get(3)?;
        let terminal = super::reducer_validation::terminal(&fact)
            .filter(|_| {
                fact.run_id.as_str() == run
                    && fact.event.turn_id().is_some_and(|id| id.as_str() == turn)
            })
            .map(|state| state as u8);
        conn.execute("UPDATE tombstones SET terminal=?1 WHERE producer=?2 AND generation=?3 AND run=?4 AND turn_id=?5", params![terminal,row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?])?;
    }
    Ok(())
}
