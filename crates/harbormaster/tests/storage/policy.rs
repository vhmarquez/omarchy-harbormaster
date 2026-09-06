use super::common::*;
use harbormaster::storage::{
    DAY, DeliveryState, HistoryRetention, Request, Response, ReviewUpdate,
};

#[test]
fn seven_day_delivery_expiry_keeps_history_and_human_review_separate() {
    let fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let set = write_set(&generation, fixture.revision(), 1);
    fixture
        .call(Request::Commit(Box::new(set.clone())))
        .unwrap();
    let Response::Maintained { result, .. } = fixture
        .call(Request::Maintain {
            expected_revision: fixture.revision(),
            now: 100 + 8 * DAY,
            history: HistoryRetention::ThirtyDays,
        })
        .unwrap()
    else {
        panic!("missing maintenance result")
    };
    assert_eq!(result.facts_removed, 0);
    assert_eq!(result.tombstones_removed, 0);
    assert_eq!(result.deliveries_expired, 1);
    let Response::Outcome(Some(outcome)) = fixture.call(Request::Outcome(key(&set))).unwrap()
    else {
        panic!("lost outcome")
    };
    assert_eq!(outcome.delivery, Some(DeliveryState::Expired));
    assert_eq!(outcome.attention, set.attention);
    fixture
        .call(Request::MarkReviewed(ReviewUpdate {
            expected_revision: fixture.revision(),
            outcome: key(&set),
        }))
        .unwrap();
    let Response::Outcome(Some(reviewed)) = fixture.call(Request::Outcome(key(&set))).unwrap()
    else {
        panic!("lost reviewed outcome")
    };
    assert!(reviewed.attention.iter().all(|item| item.reviewed));
    assert_eq!(reviewed.delivery, Some(DeliveryState::Expired));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "One fixture follows history, tombstone and obligation lifetimes together"
)]
fn history_removal_preserves_obligations_and_tombstones_until_their_own_policy() {
    let fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let set = write_set(&generation, fixture.revision(), 1);
    fixture
        .call(Request::Commit(Box::new(set.clone())))
        .unwrap();
    let Response::Maintained { result, .. } = fixture
        .call(Request::Maintain {
            expected_revision: fixture.revision(),
            now: 100 + 8 * DAY,
            history: HistoryRetention::SevenDays,
        })
        .unwrap()
    else {
        panic!("missing maintenance result")
    };
    assert_eq!(
        (
            result.facts_removed,
            result.tombstones_removed,
            result.generations_retired
        ),
        (1, 0, 0)
    );
    let Response::Outcome(Some(outcome)) = fixture.call(Request::Outcome(key(&set))).unwrap()
    else {
        panic!("lost protected obligation")
    };
    assert_eq!(outcome.attention, set.attention);
    let Response::Maintained { result, .. } = fixture
        .call(Request::Maintain {
            expected_revision: fixture.revision(),
            now: 100 + 31 * DAY,
            history: HistoryRetention::ThirtyDays,
        })
        .unwrap()
    else {
        panic!("missing retirement result")
    };
    assert_eq!(
        (result.tombstones_removed, result.generations_retired),
        (1, 1)
    );
    let Response::Producer(Some(producer)) = fixture
        .call(Request::Producer(PRODUCER.parse().unwrap()))
        .unwrap()
    else {
        panic!("missing retired producer")
    };
    assert!(!producer.active);
    assert!(matches!(
        fixture.call(Request::Outcome(key(&set))),
        Ok(Response::Outcome(Some(_)))
    ));
}

#[test]
fn approved_history_choices_do_not_silently_clamp_the_illustrated_ninety_days() {
    assert_eq!(HistoryRetention::default(), HistoryRetention::ThirtyDays);
    for days in [0, 1, 29, 31, 90, u16::MAX] {
        assert!(HistoryRetention::from_days(days).is_err());
    }
    assert_eq!(HistoryRetention::from_days(7).unwrap().days(), 7);
    assert_eq!(HistoryRetention::from_days(30).unwrap().days(), 30);
}
