//! Disposable source/linkage probe; this is never a Harbormaster binary target.
fn main() -> rusqlite::Result<()> {
    let database = rusqlite::Connection::open_in_memory()?;
    let source: String = database.query_row("SELECT sqlite_source_id()", [], |row| row.get(0))?;
    println!("version={}", rusqlite::version());
    println!("number={}", rusqlite::version_number());
    println!("source={source}");
    let mut options = database.prepare("PRAGMA compile_options")?;
    for row in options.query_map([], |row| row.get::<_, String>(0))? {
        println!("option={}", row?);
    }
    database.execute_batch("CREATE TABLE smoke (v INTEGER); INSERT INTO smoke VALUES (1)")?;
    let value: i64 = database.query_row("SELECT v FROM smoke", [], |row| row.get(0))?;
    assert_eq!(value, 1);
    Ok(())
}
