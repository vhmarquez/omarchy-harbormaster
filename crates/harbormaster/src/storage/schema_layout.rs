//! Exact recognized layouts; legacy SQL is retained byte-for-byte.
pub(super) const LEGACY: [(&str, &str); 8] = [
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

pub(super) fn current() -> Vec<(&'static str, String)> {
    let mut tables = LEGACY
        .iter()
        .map(|(name, sql)| (*name, (*sql).to_owned()))
        .collect::<Vec<_>>();
    for (name, sql) in &mut tables {
        let extra = match *name {
            "producers" => ",reconciled INTEGER NOT NULL DEFAULT 0 CHECK(reconciled IN(0,1))",
            "metadata" => {
                ",discarded BLOB NOT NULL DEFAULT X'0000000000000000' CHECK(typeof(discarded)='blob' AND length(discarded)=8)"
            }
            "projections" => {
                ",producer TEXT,generation TEXT,CHECK((producer IS NULL)=(generation IS NULL))"
            }
            "attention" => {
                ",turn_id TEXT,resolved INTEGER NOT NULL DEFAULT 0 CHECK(resolved IN(0,1)),outcome_revision BLOB CHECK(outcome_revision IS NULL OR (typeof(outcome_revision)='blob' AND length(outcome_revision)=8)),scoped INTEGER NOT NULL DEFAULT 0 CHECK(scoped IN(0,1)),resolution_generation TEXT,CHECK(scoped=0 OR resolution_generation IS NOT NULL)"
            }
            "outbox" => {
                ",outcome_revision BLOB CHECK(outcome_revision IS NULL OR (typeof(outcome_revision)='blob' AND length(outcome_revision)=8))"
            }
            "tombstones" => {
                ",terminal INTEGER CHECK(terminal IS NULL OR terminal IN(4,5,6)),outcome_revision BLOB CHECK(outcome_revision IS NULL OR (typeof(outcome_revision)='blob' AND length(outcome_revision)=8))"
            }
            _ => "",
        };
        // Table constraints must follow the added column definitions.
        let position = sql
            .find(",PRIMARY KEY(")
            .unwrap_or_else(|| sql.len() - ") STRICT".len());
        sql.insert_str(position, extra);
    }
    tables.push(("attention_active_scope", "CREATE UNIQUE INDEX attention_active_scope ON attention(producer,resolution_generation,run,coalesce(turn_id,''),reason) WHERE scoped=1 AND resolved=0 AND reason IN(0,1,3)".to_owned()));
    tables
}
