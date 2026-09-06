//! Real private Unix peers exercise the crate-private durable installation seam.
use super::*;
mod fixtures;
use fixtures::{Fixture, frame, id};

#[test]
fn committed_generation_and_checkpoint_replace_only_the_named_producer() {
    let fixture = Fixture::new();
    let mut registry = Registry::new();
    registry.register(fixture.registration(1, 3, 1)).unwrap();
    registry.register(fixture.registration(2, 4, 1)).unwrap();
    let old = fixture.session(&registry, 1).unwrap();
    let other = fixture.session(&registry, 2).unwrap();
    registry.admit(&old, &frame(1, 3, 1)).unwrap();
    registry.admit(&old, &frame(1, 3, 2)).unwrap();
    registry.admit(&other, &frame(2, 4, 1)).unwrap();
    assert_eq!(
        registry.install_committed(fixture.registration(1, 8, 17)),
        Ok(2)
    );
    assert_eq!(registry.discarded(), 2);
    assert_eq!(
        (registry.receipts, registry.receipt_bytes),
        (1, frame(2, 4, 1).len())
    );
    assert_eq!(
        registry.producers[&id::<ProducerId>(1)]
            .registration
            .generation,
        id(8)
    );
    assert_eq!(
        registry.admit(&old, &frame(1, 3, 1)),
        Err(AdmissionError::StaleGeneration)
    );
    let retained = registry.pop().unwrap();
    assert_eq!((retained.producer_id, retained.generation), (id(2), id(4)));
    let current = fixture.session(&registry, 1).unwrap();
    assert_eq!(
        registry.admit(&current, &frame(1, 8, 17)),
        Ok(Admission::Queued)
    );
    let event = registry.pop().unwrap();
    assert_eq!((event.generation, event.seq), (id(8), Seq::new(17)));
    assert!(registry.pop().is_none());
}

#[test]
fn failed_installation_preserves_authority_receipts_and_backlog() {
    for case in 0..5 {
        let fixture = Fixture::new();
        let mut registry = Registry::new();
        registry.register(fixture.registration(1, 3, 1)).unwrap();
        let old = fixture.session(&registry, 1).unwrap();
        registry.admit(&old, &frame(1, 3, 1)).unwrap();
        let mut replacement = fixture.registration(1, 8, 99);
        let expected = match case {
            0 => {
                replacement.run_id = id(999);
                AdmissionError::PermissionDenied
            }
            1 => {
                replacement.harness = HarnessKind::Claude;
                AdmissionError::PermissionDenied
            }
            2 => {
                replacement.uid = replacement.uid.wrapping_add(1);
                AdmissionError::PermissionDenied
            }
            3 => {
                replacement.allowed_signals.push(EventKind::TurnStarted);
                AdmissionError::InvalidHandshake
            }
            _ => {
                replacement.generation = id(3);
                AdmissionError::StaleGeneration
            }
        };
        assert_eq!(registry.install_committed(replacement), Err(expected));
        assert_eq!(
            registry.admit(&old, &frame(1, 3, 1)),
            Ok(Admission::AlreadyAdmitted)
        );
        assert_eq!(
            (registry.queued(), registry.discarded(), registry.receipts),
            (1, 0, 1)
        );
        assert_eq!(registry.receipt_bytes, frame(1, 3, 1).len());
        assert_eq!(registry.admit(&old, &frame(1, 3, 2)), Ok(Admission::Queued));
    }
}

#[test]
fn invalidation_blocks_old_and_new_sessions_without_erasing_replay_receipts() {
    let fixture = Fixture::new();
    let mut registry = Registry::new();
    registry.register(fixture.registration(1, 3, 1)).unwrap();
    registry.register(fixture.registration(2, 4, 1)).unwrap();
    let old = fixture.session(&registry, 1).unwrap();
    let other = fixture.session(&registry, 2).unwrap();
    registry.admit(&old, &frame(1, 3, 1)).unwrap();
    registry.admit(&old, &frame(1, 3, 2)).unwrap();
    registry.admit(&other, &frame(2, 4, 1)).unwrap();
    assert_eq!(registry.invalidate(&id(1)), Ok(2));
    assert_eq!(registry.invalidate(&id(1)), Ok(0));
    assert_eq!(
        (registry.queued(), registry.discarded(), registry.receipts),
        (1, 2, 3)
    );
    assert_eq!(
        registry.admit(&old, &frame(1, 3, 1)),
        Err(AdmissionError::StaleGeneration)
    );
    assert!(matches!(
        fixture.session(&registry, 1),
        Err(AdmissionError::ReconciliationRequired)
    ));
    assert_eq!(
        registry.admit(&other, &frame(2, 4, 1)),
        Ok(Admission::AlreadyAdmitted)
    );
    assert_eq!(
        registry.install_committed(fixture.registration(1, 8, 7)),
        Ok(0)
    );
    let current = fixture.session(&registry, 1).unwrap();
    assert_eq!(
        registry.admit(&current, &frame(1, 8, 7)),
        Ok(Admission::Queued)
    );
    assert_eq!(registry.receipts, 2);
}

#[test]
fn initial_installation_and_existing_replacement_respect_the_registry_cap() {
    let fixture = Fixture::new();
    let mut registry = Registry::new();
    for producer in 1..=u32::try_from(MAX_PRODUCERS).unwrap() {
        assert_eq!(
            registry.install_committed(fixture.registration(producer, 3, 1)),
            Ok(0)
        );
    }
    assert_eq!(
        registry.install_committed(fixture.registration(1, 8, 9)),
        Ok(0)
    );
    let extra = u32::try_from(MAX_PRODUCERS + 1).unwrap();
    assert_eq!(
        registry.install_committed(fixture.registration(extra, 8, 1)),
        Err(AdmissionError::ResourceExhausted)
    );
    assert_eq!(registry.producers.len(), MAX_PRODUCERS);
    assert_eq!(
        registry.producers[&id::<ProducerId>(1)].next,
        Some(Seq::new(9))
    );
    assert_eq!(
        registry.invalidate(&id(extra)),
        Err(AdmissionError::UnknownProducer)
    );
}

#[test]
fn invalid_initial_signal_grant_does_not_create_registration() {
    let fixture = Fixture::new();
    let mut registry = Registry::new();
    let mut invalid = fixture.registration(1, 3, 1);
    invalid.allowed_signals.push(EventKind::TurnStarted);
    assert_eq!(
        registry.install_committed(invalid),
        Err(AdmissionError::InvalidHandshake)
    );
    assert!(registry.producers.is_empty());
    assert_eq!(
        (registry.queued(), registry.discarded(), registry.receipts),
        (0, 0, 0)
    );
}

#[test]
fn trusted_replacement_narrows_the_negotiated_signal_grant() {
    let fixture = Fixture::new();
    let mut registry = Registry::new();
    registry.register(fixture.registration(1, 3, 1)).unwrap();
    let old = fixture.session(&registry, 1).unwrap();
    let mut replacement = fixture.registration(1, 8, 1);
    replacement.allowed_signals = vec![EventKind::TurnCompleted];
    registry.install_committed(replacement).unwrap();
    let current = fixture.session(&registry, 1).unwrap();
    assert_eq!(
        current.negotiated_signals().collect::<Vec<_>>(),
        vec![EventKind::TurnCompleted]
    );
    assert_eq!(
        registry.admit(&old, &frame(1, 3, 1)),
        Err(AdmissionError::StaleGeneration)
    );
    assert_eq!(
        registry.admit(&current, &frame(1, 8, 1)),
        Err(AdmissionError::PermissionDenied)
    );
    assert_eq!(registry.queued(), 0);
}

#[test]
fn invalidation_drop_and_rejection_counters_saturate() {
    let fixture = Fixture::new();
    let mut registry = Registry::new();
    registry.register(fixture.registration(1, 3, 1)).unwrap();
    let old = fixture.session(&registry, 1).unwrap();
    registry.admit(&old, &frame(1, 3, 1)).unwrap();
    registry.admit(&old, &frame(1, 3, 2)).unwrap();
    registry.discarded = u64::MAX - 1;
    registry.rejected = u64::MAX;
    assert_eq!(registry.invalidate(&id(1)), Ok(2));
    assert_eq!(registry.discarded(), u64::MAX);
    assert_eq!(
        registry.admit(&old, &frame(1, 3, 3)),
        Err(AdmissionError::StaleGeneration)
    );
    assert_eq!(registry.rejected(), u64::MAX);
}
