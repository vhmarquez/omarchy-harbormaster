use std::process::Command;

#[test]
fn actual_rust_consumer_uses_the_pinned_static_sqlite_with_hardened_defaults() {
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("../../../../tools/sqlite.lock.json")).unwrap();
    assert_eq!(
        i64::from(rusqlite::version_number()),
        pin["version_number"].as_i64().unwrap()
    );
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let source: String = connection
        .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(source, pin["source_id"].as_str().unwrap());
    let trusted: i64 = connection
        .pragma_query_value(None, "trusted_schema", |row| row.get(0))
        .unwrap();
    let foreign: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .unwrap();
    assert_eq!((trusted, foreign), (0, 1));
    assert!(
        connection
            .prepare("SELECT load_extension('not-a-library')")
            .is_err()
    );
    let elf = Command::new("/usr/bin/readelf")
        .arg("-d")
        .arg(std::env::current_exe().unwrap())
        .output()
        .unwrap();
    assert!(elf.status.success());
    let dynamic = String::from_utf8(elf.stdout).unwrap();
    assert!(
        !dynamic.to_ascii_lowercase().contains("libsqlite"),
        "unexpected SQLite dynamic linkage"
    );
    let mappings = std::fs::read_to_string("/proc/self/maps").unwrap();
    assert!(
        !mappings.to_ascii_lowercase().contains("libsqlite"),
        "unexpected loaded SQLite shared library"
    );
}
