use super::common::*;
use harbormaster::storage::{
    ReceiptError, Request, Response, SnapshotQuery, StorageError, SubmitFailure,
};

#[test]
fn bounded_snapshot_pages_have_no_omissions_and_reject_a_stale_cursor() {
    let fixture = Fixture::new();
    let mut expected = Vec::new();
    for index in 0..5 {
        let producer = format!("22222222-2222-4222-8222-{index:012}");
        let run = format!("44444444-4444-4444-8444-{index:012}");
        let generation = fixture.register(&producer, &run, 1);
        let mut set = write_set(&generation, fixture.revision(), 1);
        set.fact.producer_id = producer.parse().unwrap();
        set.fact.run_id = run.parse().unwrap();
        set.projection.run_id = set.fact.run_id.clone();
        fixture.call(Request::Commit(Box::new(set))).unwrap();
        expected.push(run);
    }
    let revision = fixture.revision();
    let mut after = None;
    let mut actual = Vec::new();
    loop {
        let Response::Snapshot(page) = fixture
            .call(Request::Snapshot(SnapshotQuery {
                expected_revision: Some(revision),
                after_run: after.clone(),
                limit: 2,
            }))
            .unwrap()
        else {
            panic!("missing page")
        };
        assert_eq!(page.revision, revision);
        assert!(page.runs.len() <= 2);
        actual.extend(page.runs.iter().map(|run| run.run_id.to_string()));
        if page.runs.len() < 2 {
            break;
        }
        after = page.runs.last().map(|run| run.run_id.clone());
        assert!(actual.len() <= expected.len(), "cursor failed to advance");
    }
    assert_eq!(actual, expected);
    fixture.register("22222222-2222-4222-8222-222222222229", RUN, 1);
    assert_eq!(
        fixture.call(Request::Snapshot(SnapshotQuery {
            expected_revision: Some(revision),
            after_run: after,
            limit: 2,
        })),
        Err(ReceiptError::Storage(StorageError::StaleRevision))
    );
}

#[test]
fn invalid_page_limits_are_rejected_before_queuing_and_return_the_request() {
    let fixture = Fixture::new();
    for limit in [0, 101, u16::MAX] {
        let request = Request::Snapshot(SnapshotQuery {
            expected_revision: None,
            after_run: None,
            limit,
        });
        let failure = fixture.worker().try_submit(request.clone()).err().unwrap();
        assert_eq!(failure.request, request);
        assert_eq!(
            failure.reason,
            SubmitFailure::Invalid(StorageError::InvalidRequest)
        );
        assert_eq!(fixture.worker().outstanding(), 0);
    }
}
