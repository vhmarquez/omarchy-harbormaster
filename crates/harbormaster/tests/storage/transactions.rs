use super::common::*;
use harbormaster::protocol::Seq;
use harbormaster::storage::{ReceiptError, Request, Response, SnapshotQuery, StorageError};

#[test]
fn commit_atomically_exposes_projection_attention_outbox_and_durable_retry() {
    let fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let set = write_set(&generation, fixture.revision(), 1);
    let response = fixture
        .call(Request::Commit(Box::new(set.clone())))
        .unwrap();
    let Response::Committed {
        revision,
        duplicate: false,
    } = response
    else {
        panic!("not committed")
    };
    assert_eq!(fixture.revision(), revision);
    let Response::Outcome(Some(outcome)) = fixture.call(Request::Outcome(key(&set))).unwrap()
    else {
        panic!("missing atomic outcome")
    };
    assert_eq!(outcome.attention, set.attention);
    assert_eq!(
        outcome.delivery,
        Some(harbormaster::storage::DeliveryState::Pending)
    );
    let Response::Producer(Some(producer)) = fixture
        .call(Request::Producer(PRODUCER.parse().unwrap()))
        .unwrap()
    else {
        panic!("missing producer")
    };
    assert_eq!(producer.next_sequence, Some(Seq::new(2)));
    assert_eq!(
        fixture.call(Request::Commit(Box::new(set.clone()))),
        Ok(Response::Committed {
            revision,
            duplicate: true
        })
    );
    let mut conflict = set;
    conflict.attention[0].reviewed = true;
    assert_eq!(
        fixture.call(Request::Commit(Box::new(conflict))),
        Err(ReceiptError::Storage(StorageError::Conflict))
    );
    assert_eq!(fixture.revision(), revision);
}

#[test]
fn stale_revision_and_sequence_gap_do_not_partially_mutate_any_rows() {
    let fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let revision = fixture.revision();
    let mut set = write_set(&generation, revision, 2);
    assert_eq!(
        fixture.call(Request::Commit(Box::new(set.clone()))),
        Err(ReceiptError::Storage(StorageError::SequenceGap))
    );
    assert_eq!(fixture.revision(), revision);
    assert_eq!(
        fixture.call(Request::Outcome(key(&set))),
        Ok(Response::Outcome(None))
    );
    set.fact.seq = Seq::new(1);
    set.expected_revision = harbormaster::protocol::Revision::new(0);
    assert_eq!(
        fixture.call(Request::Commit(Box::new(set))),
        Err(ReceiptError::Storage(StorageError::StaleRevision))
    );
    let Response::Snapshot(page) = fixture
        .call(Request::Snapshot(SnapshotQuery {
            expected_revision: None,
            after_run: None,
            limit: 100,
        }))
        .unwrap()
    else {
        panic!("missing snapshot")
    };
    assert!(page.runs.is_empty());
    assert_eq!(page.revision, revision);
}

#[test]
fn full_u64_sequence_survives_sqlite_and_cannot_wrap() {
    let fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, u64::MAX);
    let set = write_set(&generation, fixture.revision(), u64::MAX);
    assert!(matches!(
        fixture.call(Request::Commit(Box::new(set.clone()))),
        Ok(Response::Committed {
            duplicate: false,
            ..
        })
    ));
    let Response::Producer(Some(producer)) = fixture
        .call(Request::Producer(PRODUCER.parse().unwrap()))
        .unwrap()
    else {
        panic!("missing producer")
    };
    assert_eq!(producer.next_sequence, None);
    let mut next = set;
    next.expected_revision = fixture.revision();
    next.fact.seq = Seq::new(0);
    next.fact.event_id = "11111111-1111-4111-8111-111111111112".parse().unwrap();
    next.attention.clear();
    next.outbox = None;
    next.tombstone = None;
    assert_eq!(
        fixture.call(Request::Commit(Box::new(next))),
        Err(ReceiptError::Storage(StorageError::Conflict))
    );
}

#[test]
fn event_ids_are_scoped_to_their_producer_and_generation() {
    let fixture = Fixture::new();
    let first_gen = fixture.register(PRODUCER, RUN, 1);
    let other_producer = "22222222-2222-4222-8222-222222222223";
    let other_run = "44444444-4444-4444-8444-444444444445";
    let second_gen = fixture.register(other_producer, other_run, 1);
    let first = write_set(&first_gen, fixture.revision(), 1);
    fixture
        .call(Request::Commit(Box::new(first.clone())))
        .unwrap();
    let mut second = write_set(&second_gen, fixture.revision(), 1);
    second.fact.producer_id = other_producer.parse().unwrap();
    second.fact.run_id = other_run.parse().unwrap();
    second.projection.run_id = second.fact.run_id.clone();
    fixture
        .call(Request::Commit(Box::new(second.clone())))
        .unwrap();
    for set in [first, second] {
        let Response::Outcome(Some(outcome)) = fixture.call(Request::Outcome(key(&set))).unwrap()
        else {
            panic!("missing scoped outcome")
        };
        assert_eq!(outcome.run_id, set.fact.run_id);
        assert_eq!(outcome.outcome, key(&set));
    }
}
