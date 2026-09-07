use super::*;
use crate::{
    projects::{CatalogRequest, CatalogResponse},
    runtime::{RunnerRequest, RunnerResponse},
};

fn catalog(engine: &mut Engine, request: CatalogRequest) -> CatalogResponse {
    let Response::Catalog(response) = engine
        .execute(&Request::Catalog(Box::new(request)))
        .unwrap()
    else {
        panic!()
    };
    *response
}

#[test]
fn version_four_upgrade_preserves_tasks_and_reserves_one_durable_attempt() {
    let f = Fixture::new();
    let mut engine = f.open();
    let task = registered_task(&f, &mut engine);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute_batch("DROP TABLE managed_runs; PRAGMA user_version=4")
        .unwrap();
    drop(engine);
    let mut engine = f.open();
    assert_eq!(
        schema::header(&f.root.join("harbormaster/state.backup.db")).unwrap(),
        4
    );
    assert_eq!(count(&engine, "tasks"), 1);
    let request = Request::Runner(Box::new(RunnerRequest::Reserve {
        task_id: task.id,
        runtime_root: "/run/user/1000".parse().unwrap(),
    }));
    let Response::Runner(response) = engine.execute(&request).unwrap() else {
        panic!()
    };
    let RunnerResponse::Reserved { run, created } = *response else {
        panic!()
    };
    assert!(created);
    drop(engine);
    let mut engine = f.open();
    let Response::Runner(response) = engine.execute(&request).unwrap() else {
        panic!()
    };
    assert_eq!(
        *response,
        RunnerResponse::Reserved {
            run,
            created: false
        }
    );
    assert_eq!(count(&engine, "managed_runs"), 1);
}

fn registered_task(f: &Fixture, engine: &mut Engine) -> crate::projects::Task {
    let CatalogResponse::Project(project) = catalog(
        engine,
        CatalogRequest::AddProject {
            root: f.root.to_str().unwrap().parse().unwrap(),
            label: "Example".parse().unwrap(),
        },
    ) else {
        panic!()
    };
    let binary = f.root.join("hermes");
    std::fs::write(&binary, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    catalog(
        engine,
        CatalogRequest::AddPreset {
            name: "coding".into(),
            executable: binary.to_str().unwrap().parse().unwrap(),
            profile: "synthetic".into(),
        },
    );
    let CatalogResponse::Task(task) = catalog(
        engine,
        CatalogRequest::AddTask {
            project_id: project.id.clone(),
            preset: "coding".into(),
            label: "Example task".parse().unwrap(),
        },
    ) else {
        panic!()
    };
    task
}
