use super::*;
use crate::projects::{CatalogRequest, CatalogResponse};

#[test]
fn version_three_migration_preserves_state_and_creates_a_verified_backup() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 8);
    commit(&mut engine, &set);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute_batch(
            "DROP TABLE managed_runs; DROP TABLE tasks; DROP TABLE presets; DROP TABLE projects; PRAGMA user_version=3",
        )
        .unwrap();
    drop(engine);
    let mut engine = fixture.open();
    assert_eq!(
        schema::header(&fixture.root.join("harbormaster/state.backup.db")).unwrap(),
        3
    );
    assert_eq!(count(&engine, "facts"), 1);
    assert!(matches!(
        commit(&mut engine, &set),
        Response::Committed {
            duplicate: true,
            ..
        }
    ));
    let Response::Catalog(response) = engine
        .execute(&Request::Catalog(Box::new(CatalogRequest::Projects {
            after: None,
        })))
        .unwrap()
    else {
        panic!()
    };
    let CatalogResponse::Projects(page) = *response else {
        panic!()
    };
    assert!(page.items.is_empty());
    engine.execute(&Request::RestoreBackup).unwrap();
    assert_eq!(count(&engine, "facts"), 1);
    assert_eq!(
        schema::inspect(engine.connection.as_ref().unwrap()).unwrap(),
        schema::VERSION
    );
}
