use super::Fixture;
use harbormaster::ingestion::{AdmissionError, ControlAuthorizer};
use harbormaster::ipc::Channel;
use harbormaster::protocol::{CapabilityRecord, CapabilityReason, CapabilitySource,
    CapabilityStatus, ControlHandshake, ObservationFreshness, Operation, Revision,
    parse_control};

fn capability(operation: Operation) -> CapabilityRecord {
    CapabilityRecord { operation, status: CapabilityStatus::Supported,
        source: CapabilitySource::Manager, tested_version: None,
        reason: CapabilityReason::Available, freshness: ObservationFreshness::Fresh }
}

#[test]
fn event_credentials_cannot_negotiate_control_even_with_matching_uid() {
    let f = Fixture::new(Channel::Event);
    let auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    let h = ControlHandshake { requested_operations: vec![Operation::Snapshot] };
    assert!(matches!(auth.connect(&f.peer, &h), Err(AdmissionError::PermissionDenied)));
}

#[test]
fn peer_identity_and_requested_operation_alone_do_not_grant_permission() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), []);
    auth.set_capability(capability(Operation::Snapshot));
    let h = ControlHandshake { requested_operations: vec![Operation::Snapshot] };
    let session = auth.connect(&f.peer, &h).unwrap();
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"snapshot\",\"args\":{\"limit\":10,\"cursor\":null}}\n").unwrap();
    assert_eq!(auth.authorize(&session, &request, Revision::new(1)), Err(AdmissionError::PermissionDenied));
    let wrong_uid = ControlAuthorizer::new(f.peer.peer().uid().wrapping_add(1), [Operation::Snapshot]);
    assert!(matches!(wrong_uid.connect(&f.peer, &h), Err(AdmissionError::PermissionDenied)));
}

#[test]
fn current_capabilities_and_expected_revision_are_rechecked_without_side_effects() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::ReviewMark]);
    let h = ControlHandshake { requested_operations: vec![Operation::ReviewMark] };
    let session = auth.connect(&f.peer, &h).unwrap();
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"review.mark\",\"expected_revision\":\"7\",\"args\":{\"run_id\":\"44444444-4444-4444-8444-444444444444\",\"outcome_revision\":\"6\"}}\n").unwrap();
    assert_eq!(auth.authorize(&session, &request, Revision::new(7)), Err(AdmissionError::CapabilityUnavailable));
    auth.set_capability(capability(Operation::ReviewMark));
    assert_eq!(auth.authorize(&session, &request, Revision::new(8)), Err(AdmissionError::StaleRevision));
    for _ in 0..10 { assert_eq!(auth.authorize(&session, &request, Revision::new(7)), Ok(())); }
    for freshness in [ObservationFreshness::Stale, ObservationFreshness::Disconnected, ObservationFreshness::Unsupported] {
        let mut record = capability(Operation::ReviewMark); record.freshness = freshness;
        auth.set_capability(record);
        assert_eq!(auth.authorize(&session, &request, Revision::new(7)), Err(AdmissionError::CapabilityUnavailable));
    }
    for status in [CapabilityStatus::Unsupported, CapabilityStatus::Unknown] {
        let mut record = capability(Operation::ReviewMark); record.status = status;
        auth.set_capability(record);
        assert_eq!(auth.authorize(&session, &request, Revision::new(7)), Err(AdmissionError::CapabilityUnavailable));
    }
}

#[test]
fn duplicate_negotiation_and_missing_requested_operation_fail_closed() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    auth.set_capability(capability(Operation::Snapshot));
    let h = ControlHandshake { requested_operations: vec![Operation::Snapshot, Operation::Snapshot] };
    assert!(matches!(auth.connect(&f.peer, &h), Err(AdmissionError::InvalidHandshake)));
    let session = auth.connect(&f.peer, &ControlHandshake { requested_operations: vec![] }).unwrap();
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"snapshot\",\"args\":{\"limit\":10,\"cursor\":null}}\n").unwrap();
    assert_eq!(auth.authorize(&session, &request, Revision::new(1)), Err(AdmissionError::PermissionDenied));
}
