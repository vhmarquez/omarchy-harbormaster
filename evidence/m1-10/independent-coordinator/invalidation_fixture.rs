
#[test]
fn independent_old_scope_invalidation_preserves_new_peer_and_queued_event() {
    let mut fixture = Fixture::new();
    let peer = Peer::new();
    let old = fixture.registered();
    let input = fixture.input(Some(old.clone()));
    let ReconciliationProgress::Installed { generation: new, .. } = fixture.reconcile(input)
    else { panic!("new registration"); };
    let session = peer.session(&fixture.registry, &new).unwrap();
    let entry = fixture.stage(&new, 1, "turn.started");
    let frame = encode_frame(entry.event()).unwrap();
    assert_eq!(fixture.registry.admit(&session, &frame), Ok(Admission::Queued));
    assert_eq!(fixture.registry.queued(), 1);
    assert_eq!(fixture.registry.invalidate_generation(&id(2), Some(&old)), Err(AdmissionError::StaleGeneration));
    assert_eq!(fixture.registry.queued(), 1);
    assert_eq!(fixture.registry.discarded(), 0);
    assert_eq!(fixture.registry.admit(&session, &frame), Ok(Admission::AlreadyAdmitted));
    assert!(peer.session(&fixture.registry, &new).is_ok());
}
