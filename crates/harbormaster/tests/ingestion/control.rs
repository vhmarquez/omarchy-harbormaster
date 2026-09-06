use super::Fixture;
use harbormaster::ingestion::{AdmissionError, ControlAuthorizer};
use harbormaster::ipc::Channel;
use harbormaster::protocol::{
    CapabilityReason, CapabilityRecord, CapabilitySource, CapabilityStatus, ControlHandshake,
    ObservationFreshness, Operation, Revision, parse_control,
};

fn capability(operation: Operation) -> CapabilityRecord {
    CapabilityRecord {
        operation,
        status: CapabilityStatus::Supported,
        source: CapabilitySource::Manager,
        tested_version: None,
        reason: CapabilityReason::Available,
        freshness: ObservationFreshness::Fresh,
    }
}

#[test]
fn event_credentials_cannot_negotiate_control_even_with_matching_uid() {
    let f = Fixture::new(Channel::Event);
    let auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    let h = ControlHandshake {
        requested_operations: vec![Operation::Snapshot],
    };
    assert!(matches!(
        auth.connect(&f.peer, &h),
        Err(AdmissionError::PermissionDenied)
    ));
}

#[test]
fn peer_identity_and_requested_operation_alone_do_not_grant_permission() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), []);
    auth.set_capability(capability(Operation::Snapshot));
    let h = ControlHandshake {
        requested_operations: vec![Operation::Snapshot],
    };
    let session = auth.connect(&f.peer, &h).unwrap();
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"snapshot\",\"arguments\":{\"limit\":10,\"cursor\":null}}\n").unwrap();
    assert_eq!(
        auth.authorize(&session, &request, Revision::new(1)),
        Err(AdmissionError::PermissionDenied)
    );
    let wrong_uid =
        ControlAuthorizer::new(f.peer.peer().uid().wrapping_add(1), [Operation::Snapshot]);
    assert!(matches!(
        wrong_uid.connect(&f.peer, &h),
        Err(AdmissionError::PermissionDenied)
    ));
}

#[test]
fn current_capabilities_and_expected_revision_are_rechecked_without_side_effects() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::ReviewMark]);
    let h = ControlHandshake {
        requested_operations: vec![Operation::ReviewMark],
    };
    let session = auth.connect(&f.peer, &h).unwrap();
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"review.mark\",\"expected_revision\":\"7\",\"arguments\":{\"run_id\":\"44444444-4444-4444-8444-444444444444\",\"outcome_revision\":\"6\"}}\n").unwrap();
    assert_eq!(
        auth.authorize(&session, &request, Revision::new(7)),
        Err(AdmissionError::CapabilityUnavailable)
    );
    auth.set_capability(capability(Operation::ReviewMark));
    assert_eq!(
        auth.authorize(&session, &request, Revision::new(8)),
        Err(AdmissionError::StaleRevision)
    );
    for _ in 0..10 {
        assert_eq!(auth.authorize(&session, &request, Revision::new(7)), Ok(()));
    }
    for freshness in [
        ObservationFreshness::Stale,
        ObservationFreshness::Disconnected,
        ObservationFreshness::Unsupported,
    ] {
        let mut record = capability(Operation::ReviewMark);
        record.freshness = freshness;
        auth.set_capability(record);
        assert_eq!(
            auth.authorize(&session, &request, Revision::new(7)),
            Err(AdmissionError::CapabilityUnavailable)
        );
    }
    for status in [CapabilityStatus::Unsupported, CapabilityStatus::Unknown] {
        let mut record = capability(Operation::ReviewMark);
        record.status = status;
        auth.set_capability(record);
        assert_eq!(
            auth.authorize(&session, &request, Revision::new(7)),
            Err(AdmissionError::CapabilityUnavailable)
        );
    }
}

#[test]
fn duplicate_negotiation_and_missing_requested_operation_fail_closed() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    auth.set_capability(capability(Operation::Snapshot));
    let h = ControlHandshake {
        requested_operations: vec![Operation::Snapshot, Operation::Snapshot],
    };
    assert!(matches!(
        auth.connect(&f.peer, &h),
        Err(AdmissionError::InvalidHandshake)
    ));
    let session = auth
        .connect(
            &f.peer,
            &ControlHandshake {
                requested_operations: vec![],
            },
        )
        .unwrap();
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"snapshot\",\"arguments\":{\"limit\":10,\"cursor\":null}}\n").unwrap();
    assert_eq!(
        auth.authorize(&session, &request, Revision::new(1)),
        Err(AdmissionError::PermissionDenied)
    );
}

#[test]
fn control_session_cannot_be_reused_with_another_authorizer() {
    let f = Fixture::new(Channel::Control);
    let first = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    let h = ControlHandshake {
        requested_operations: vec![Operation::Snapshot],
    };
    let session = first.connect(&f.peer, &h).unwrap();
    let mut second = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    second.set_capability(capability(Operation::Snapshot));
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"snapshot\",\"arguments\":{\"limit\":10,\"cursor\":null}}\n").unwrap();
    assert_eq!(
        second.authorize(&session, &request, Revision::new(1)),
        Err(AdmissionError::PermissionDenied)
    );
}

#[test]
fn negotiation_reports_actual_grants_and_incomplete_evidence_cannot_authorize() {
    let f = Fixture::new(Channel::Control);
    let mut auth = ControlAuthorizer::new(f.peer.peer().uid(), [Operation::Snapshot]);
    let session = auth
        .connect(
            &f.peer,
            &ControlHandshake {
                requested_operations: vec![Operation::Snapshot, Operation::Subscribe],
            },
        )
        .unwrap();
    let negotiated = auth.negotiated_capabilities(&session).unwrap();
    assert_eq!(negotiated.len(), 2);
    assert!(negotiated.iter().any(
        |r| r.operation == Operation::Snapshot && r.reason == CapabilityReason::NotImplemented
    ));
    assert!(
        negotiated.iter().any(
            |r| r.operation == Operation::Subscribe && r.reason == CapabilityReason::NotGranted
        )
    );
    let request = parse_control(b"{\"protocol\":0,\"channel\":\"control\",\"request_id\":\"11111111-1111-4111-8111-111111111111\",\"operation\":\"snapshot\",\"arguments\":{\"limit\":10,\"cursor\":null}}\n").unwrap();
    let mut untested = capability(Operation::Snapshot);
    untested.source = CapabilitySource::NativeHarness;
    auth.set_capability(untested);
    assert_eq!(
        auth.authorize(&session, &request, Revision::new(1)),
        Err(AdmissionError::CapabilityUnavailable)
    );
    let mut contradictory = capability(Operation::Snapshot);
    contradictory.reason = CapabilityReason::NotGranted;
    auth.set_capability(contradictory);
    assert_eq!(
        auth.authorize(&session, &request, Revision::new(1)),
        Err(AdmissionError::CapabilityUnavailable)
    );
    auth.set_capability(capability(Operation::Snapshot));
    assert_eq!(auth.authorize(&session, &request, Revision::new(1)), Ok(()));
    assert_eq!(
        auth.negotiated_capabilities(&session)
            .unwrap()
            .iter()
            .find(|c| c.operation == Operation::Snapshot)
            .unwrap()
            .status,
        CapabilityStatus::Supported
    );
}
