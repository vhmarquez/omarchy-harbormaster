use super::*;
use crate::storage::{HistoryRetention, MAX_OUTSTANDING};

#[test]
fn full_worker_retains_the_unsubmitted_intent_and_never_acknowledges() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let tickets: Vec<_> = (0..MAX_OUTSTANDING)
        .map(|_| fixture.worker.try_submit(Request::Status).unwrap())
        .collect();
    let mut job = CommitJob::new(
        &fixture.worker,
        &mut fixture.registry,
        entry,
        policy(),
        1,
        true,
    );
    let original = job.operation.request().clone();
    assert_eq!(job.poll(), CommitProgress::Backpressure);
    assert_eq!(job.operation.request(), &original);
    assert!(job.take_committed().is_none());
    drop(tickets);
    assert!(matches!(finish(&mut job), CommitProgress::Committed { .. }));
}

fn at_apply(job: &mut CommitJob<'_>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(job.purpose, super::super::commit::Purpose::Apply) {
        assert!(matches!(job.poll(), CommitProgress::Pending));
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn only_an_explicit_revision_conflict_refreshes_context_before_reducing_again() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let mut job = CommitJob::new(
        &fixture.worker,
        &mut fixture.registry,
        entry,
        policy(),
        1,
        true,
    );
    at_apply(&mut job);
    fixture
        .worker
        .try_submit(Request::UpdatePolicy {
            expected_revision: Revision::new(1),
            history: HistoryRetention::SevenDays,
        })
        .unwrap()
        .wait_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(
        finish(&mut job),
        CommitProgress::Committed {
            revision: Revision::new(3)
        }
    );
    assert_eq!(job.retries, 1);
    let Request::Apply(set) = job.operation.request() else {
        panic!("apply")
    };
    assert_eq!(set.accepted_at, 1);
}

#[test]
fn discarded_actual_worker_reply_is_unknown_until_the_same_intent_proves_commit() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let mut job = CommitJob::new(
        &fixture.worker,
        &mut fixture.registry,
        entry,
        policy(),
        1,
        true,
    );
    at_apply(&mut job);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match job.operation.poll(&fixture.worker) {
            super::super::operation::Poll::Pending => (),
            super::super::operation::Poll::Result(Ok(Response::Committed { .. })) => break,
            _ => panic!("actual worker commit required before deliberate receipt loss"),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(job.poll(), CommitProgress::UnknownOutcome);
    assert!(job.unknown_gap());
    assert!(job.take_committed().is_none());
    let original = job.operation.request().clone();
    let pending = job.into_pending().unwrap();
    let mut resumed = CommitJob::resume(&fixture.worker, &mut fixture.registry, pending);
    assert_eq!(resumed.operation.request(), &original);
    assert_eq!(
        finish(&mut resumed),
        CommitProgress::Committed {
            revision: Revision::new(2)
        }
    );
    assert!(resumed.take_committed().is_some());
}
