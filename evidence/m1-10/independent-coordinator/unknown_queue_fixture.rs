
#[test]
fn independent_unknown_apply_keeps_new_queue_loss_separate_from_original_intent() {
    let mut fixture = Fixture::new();
    let peer = Peer::new();
    let generation = fixture.registered();
    let session = peer.session(&fixture.registry, &generation).unwrap();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    assert_eq!(fixture.registry.admit(&session, &encode_frame(entry.event()).unwrap()), Ok(Admission::Queued));
    let mut job = CommitJob::new(&fixture.worker, &mut fixture.registry, entry, policy(), 1, true);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(job.purpose, super::super::commit::Purpose::Apply) {
        assert_eq!(job.poll(), CommitProgress::Pending);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    loop {
        match job.operation.poll(&fixture.worker) {
            super::super::operation::Poll::Pending => (),
            super::super::operation::Poll::Result(Ok(Response::Committed { .. })) => break,
            _ => panic!("actual commit before dropping the reply"),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(job.poll(), CommitProgress::UnknownOutcome);
    let observed = job.status();
    assert_eq!(observed.known_queued_discards, 1);
    assert!(observed.unknown_gap);
    let original = job.operation.request().clone();
    let Request::Apply(set) = &original else { panic!("original Apply"); };
    assert_eq!(set.discarded_events, 0);
    assert!(job.take_committed().is_none());
    let pending = job.into_pending().unwrap();
    let mut resumed = CommitJob::resume(&fixture.worker, &mut fixture.registry, pending);
    assert_eq!(resumed.operation.request(), &original);
    assert_eq!(finish(&mut resumed), CommitProgress::Committed { revision: Revision::new(2) });
    assert_eq!(resumed.status(), observed);
    assert!(resumed.take_committed().is_some());
    drop(resumed);
    assert_eq!(fixture.status().discarded_events, 0);
    assert_eq!(fixture.registry.discarded(), 1);
}
