
#[test]
fn independent_confirmation_receipt_cannot_authorize_a_different_source_harness() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let original = entry.event().clone();
    let (receipt, _original_entry) = fixture.commit(entry).1.unwrap();
    let other_root = artifacts::Fixture::new();
    let mut other_store = other_root.open();
    let mut value: serde_json::Value = serde_json::from_slice(&artifacts::source(1, 1)).unwrap();
    value["metadata"]["generation"] = generation.as_str().into();
    value["metadata"]["kind"] = "turn.completed".into();
    let entry = artifacts::review_codex_entry(&mut other_store, &serde_json::to_vec(&value).unwrap());
    assert_eq!(entry.event(), &original);
    assert_eq!(entry.harness(), HarnessKind::Codex);
    assert!(confirm_spool(&mut other_store, entry, &receipt).is_err(),
        "durable Hermes receipt cannot confirm a Codex source artifact");
    assert_eq!(other_store.snapshot().unwrap().spool_files, 1);
}
