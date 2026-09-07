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
        .execute_batch(
            "DROP TABLE runtime_controls; DROP TABLE managed_runs; PRAGMA user_version=4",
        )
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

#[test]
fn version_five_upgrade_preserves_legacy_attempt_and_new_atomic_control_intent() {
    use crate::runtime::{ControlRequest, ControlResponse};
    let f = Fixture::new();
    let (mut engine, old_task, old) = legacy_runner_fixture(&f);
    assert_eq!(reserve(&mut engine, &old_task), old);
    assert_eq!(
        control(&mut engine, ControlRequest::Get(old.id.clone())),
        ControlResponse::State(None)
    );
    let CatalogResponse::Task(new_task) = catalog(
        &mut engine,
        CatalogRequest::AddTask {
            project_id: old_task.project_id,
            preset: "coding".into(),
            label: "Second task".parse().unwrap(),
        },
    ) else {
        panic!()
    };
    let new = reserve(&mut engine, &new_task);
    let ControlResponse::State(Some(state)) =
        control(&mut engine, ControlRequest::Get(new.id.clone()))
    else {
        panic!()
    };
    assert!(state.launch_nonce.is_some());
    let warning = ControlRequest::OtherWriters {
        project_id: new_task.project_id,
        task_id: new_task.id,
    };
    assert_eq!(
        control(&mut engine, warning.clone()),
        ControlResponse::OtherWriters(true)
    );
    stop_legacy_and_check_warning(&mut engine, old.id, &warning);
    drop(engine);
    let mut engine = f.open();
    assert_eq!(
        control(&mut engine, ControlRequest::Get(new.id)),
        ControlResponse::State(Some(state))
    );
    assert_eq!(
        control(&mut engine, warning),
        ControlResponse::OtherWriters(false)
    );
}

fn reserve(engine: &mut Engine, task: &crate::projects::Task) -> crate::runtime::ManagedRun {
    let Response::Runner(response) = engine
        .execute(&Request::Runner(Box::new(RunnerRequest::Reserve {
            task_id: task.id.clone(),
            runtime_root: "/run/user/1000".parse().unwrap(),
        })))
        .unwrap()
    else {
        panic!()
    };
    let RunnerResponse::Reserved { run, .. } = *response else {
        panic!()
    };
    run
}

fn control(
    engine: &mut Engine,
    request: crate::runtime::ControlRequest,
) -> crate::runtime::ControlResponse {
    let Response::RuntimeControl(response) = engine
        .execute(&Request::RuntimeControl(Box::new(request)))
        .unwrap()
    else {
        panic!()
    };
    *response
}

fn stop_legacy_and_check_warning(
    engine: &mut Engine,
    id: crate::protocol::RunId,
    warning: &crate::runtime::ControlRequest,
) {
    use crate::runtime::{ControlRequest, ControlResponse};
    let mut stopped = crate::runtime::ControlState::new(id, None);
    stopped.stopped = true;
    stopped.ending = true;
    control(engine, ControlRequest::Save(Box::new(stopped.clone())));
    assert_eq!(
        control(engine, warning.clone()),
        ControlResponse::OtherWriters(false)
    );
    stopped.stopped = false;
    assert!(matches!(
        engine.execute(&Request::RuntimeControl(Box::new(ControlRequest::Save(
            Box::new(stopped)
        )))),
        Err(StorageError::Conflict)
    ));
}

fn legacy_runner_fixture(
    f: &Fixture,
) -> (Engine, crate::projects::Task, crate::runtime::ManagedRun) {
    let mut engine = f.open();
    let old_task = registered_task(f, &mut engine);
    let old = reserve(&mut engine, &old_task);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute_batch("DROP TABLE runtime_controls; PRAGMA user_version=5")
        .unwrap();
    drop(engine);
    let engine = f.open();
    assert_eq!(
        schema::header(&f.root.join("harbormaster/state.backup.db")).unwrap(),
        5
    );
    (engine, old_task, old)
}
