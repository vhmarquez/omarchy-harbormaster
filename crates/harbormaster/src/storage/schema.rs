//! Exact manager schema identity; foreign/future databases are never configured.
use super::{
    StorageError,
    paths::{DATABASE, Paths},
};
use rusqlite::{Connection, OpenFlags};
use std::io::Read;
use std::path::Path;

pub(super) const APPLICATION_ID: i64 = 0x4842_4d31;
pub(super) const VERSION: i64 = 2;
pub(super) const PAGE_SIZE: u32 = 4096;
pub(super) const MAX_PAGES: u32 = 16_384;
pub(super) const WAL_TRIGGER: u32 = 4 * 1024 * 1024;
pub(super) const WAL_BOUND: u64 = 4 * 1024 * 1024 + 16_384 * (4096 + 24) + 32;

const TABLES: [(&str, &str); 8] = [
    (
        "metadata",
        "CREATE TABLE metadata(id INTEGER PRIMARY KEY CHECK(id=1),revision BLOB NOT NULL CHECK(typeof(revision)='blob' AND length(revision)=8),history_days INTEGER NOT NULL CHECK(history_days IN(7,30))) STRICT",
    ),
    (
        "producers",
        "CREATE TABLE producers(producer TEXT PRIMARY KEY,generation TEXT NOT NULL,run TEXT NOT NULL,harness INTEGER NOT NULL,next_seq BLOB CHECK(next_seq IS NULL OR (typeof(next_seq)='blob' AND length(next_seq)=8)),active INTEGER NOT NULL CHECK(active IN(0,1))) STRICT",
    ),
    (
        "facts",
        "CREATE TABLE facts(event TEXT NOT NULL,producer TEXT NOT NULL,generation TEXT NOT NULL,seq BLOB NOT NULL CHECK(typeof(seq)='blob' AND length(seq)=8),run TEXT NOT NULL,accepted INTEGER NOT NULL CHECK(accepted>=0),write_set BLOB NOT NULL CHECK(length(write_set)<=24576),revision BLOB NOT NULL CHECK(typeof(revision)='blob' AND length(revision)=8),PRIMARY KEY(producer,generation,event),UNIQUE(producer,generation,seq)) STRICT",
    ),
    (
        "projections",
        "CREATE TABLE projections(run TEXT PRIMARY KEY,process INTEGER NOT NULL,observation INTEGER NOT NULL,turn INTEGER NOT NULL,turn_id TEXT) STRICT",
    ),
    (
        "attention",
        "CREATE TABLE attention(producer TEXT NOT NULL,generation TEXT NOT NULL,event TEXT NOT NULL,run TEXT NOT NULL,reason INTEGER NOT NULL,reviewed INTEGER NOT NULL CHECK(reviewed IN(0,1)),PRIMARY KEY(producer,generation,event,reason)) STRICT",
    ),
    (
        "outbox",
        "CREATE TABLE outbox(producer TEXT NOT NULL,generation TEXT NOT NULL,event TEXT NOT NULL,run TEXT NOT NULL,state INTEGER NOT NULL,created INTEGER NOT NULL CHECK(created>=0),changed INTEGER NOT NULL CHECK(changed>=0),PRIMARY KEY(producer,generation,event)) STRICT",
    ),
    (
        "tombstones",
        "CREATE TABLE tombstones(producer TEXT NOT NULL,generation TEXT NOT NULL,run TEXT NOT NULL,turn_id TEXT NOT NULL,event TEXT NOT NULL,created INTEGER NOT NULL CHECK(created>=0),PRIMARY KEY(producer,generation,run,turn_id)) STRICT",
    ),
    (
        "maintenance",
        "CREATE TABLE maintenance(id INTEGER PRIMARY KEY CHECK(id=1),facts_removed INTEGER NOT NULL,tombstones_removed INTEGER NOT NULL,generations_retired INTEGER NOT NULL,deliveries_expired INTEGER NOT NULL) STRICT",
    ),
];

pub(super) fn header(path: &Path) -> Result<i64, StorageError> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    let mut file = std::fs::File::from(fd);
    let length = file.metadata()?.len();
    if length > u64::from(MAX_PAGES) * u64::from(PAGE_SIZE) || length < 100 {
        return Err(StorageError::CorruptDatabase);
    }
    let mut bytes = [0_u8; 100];
    file.read_exact(&mut bytes)?;
    if &bytes[..16] != b"SQLite format 3\0" {
        return Err(StorageError::ForeignDatabase);
    }
    if i64::from(u32::from_be_bytes(
        bytes[68..72]
            .try_into()
            .map_err(|_| StorageError::CorruptDatabase)?,
    )) != APPLICATION_ID
    {
        return Err(StorageError::ForeignDatabase);
    }
    let version = i64::from(u32::from_be_bytes(
        bytes[60..64]
            .try_into()
            .map_err(|_| StorageError::CorruptDatabase)?,
    ));
    if version > VERSION {
        return Err(StorageError::FutureSchema);
    }
    if !(1..=VERSION).contains(&version) || u16::from_be_bytes([bytes[16], bytes[17]]) != 4096 {
        return Err(StorageError::CorruptDatabase);
    }
    Ok(version)
}

pub(super) fn read_only(path: &Path) -> Result<Connection, StorageError> {
    header(path)?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(50))?;
    local_limits(&conn)?;
    inspect(&conn)?;
    Ok(conn)
}

pub(super) fn writable(paths: &Paths) -> Result<Connection, StorageError> {
    let path = paths.path(DATABASE);
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(50))?;
    Ok(conn)
}

pub(super) fn initialize(conn: &mut Connection) -> Result<(), StorageError> {
    conn.pragma_update(None, "page_size", PAGE_SIZE)?;
    conn.pragma_update(None, "max_page_count", MAX_PAGES)?;
    let tx = conn.transaction()?;
    for (_, statement) in TABLES {
        tx.execute_batch(statement)?;
    }
    tx.execute(
        "INSERT INTO metadata VALUES(1,?1,30)",
        [0_u64.to_be_bytes().as_slice()],
    )?;
    tx.execute_batch("INSERT INTO maintenance VALUES(1,0,0,0,0); PRAGMA application_id=1212304689; PRAGMA user_version=2;")?;
    tx.commit()?;
    Ok(())
}

pub(super) fn inspect(conn: &Connection) -> Result<i64, StorageError> {
    let app: i64 = conn.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if app != APPLICATION_ID {
        return Err(StorageError::ForeignDatabase);
    }
    if version > VERSION {
        return Err(StorageError::FutureSchema);
    }
    if !(1..=VERSION).contains(&version) {
        return Err(StorageError::CorruptDatabase);
    }
    let mut query = conn.prepare(
        "SELECT name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 9",
    )?;
    let actual = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected = TABLES
        .iter()
        .filter(|(name, _)| version == 2 || *name != "maintenance")
        .map(|(name, sql)| ((*name).to_owned(), (*sql).to_owned()))
        .collect::<Vec<_>>();
    expected.sort();
    if actual != expected {
        return Err(StorageError::ForeignDatabase);
    }
    let integrity: String = conn.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(StorageError::CorruptDatabase);
    }
    let size: u32 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
    let pages: u32 = conn.pragma_query_value(None, "page_count", |row| row.get(0))?;
    if size != PAGE_SIZE || pages > MAX_PAGES {
        return Err(StorageError::ResourceExhausted);
    }
    Ok(version)
}

pub(super) fn local_limits(conn: &Connection) -> Result<(), StorageError> {
    use rusqlite::limits::Limit;
    conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, 32_768)?;
    conn.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 8192)?;
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    conn.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 32)?;
    conn.pragma_update(None, "trusted_schema", false)?;
    Ok(())
}

pub(super) fn configure(conn: &Connection) -> Result<(), StorageError> {
    local_limits(conn)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "max_page_count", MAX_PAGES)?;
    conn.pragma_update(None, "cache_size", -1024)?;
    conn.pragma_update(None, "cache_spill", false)?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if mode != "wal" {
        return Err(StorageError::PersistenceUnavailable);
    }
    conn.pragma_update(None, "wal_autocheckpoint", 0)?;
    conn.pragma_update(None, "journal_size_limit", WAL_TRIGGER)?;
    Ok(())
}

pub(super) fn migrate(conn: &mut Connection) -> Result<(), StorageError> {
    let tx = conn.transaction()?;
    tx.execute_batch(TABLES[7].1)?;
    tx.execute_batch("INSERT INTO maintenance VALUES(1,0,0,0,0); PRAGMA user_version=2;")?;
    tx.commit()?;
    Ok(())
}
