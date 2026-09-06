
// Disposable independent review fixture only; absent from repository/library.
pub(crate) fn review_codex_entry(store: &mut ArtifactStore, source: &[u8]) -> super::ReplayEntry {
    let reviewed = &[Mapping { harness: HarnessKind::Codex, version: "synthetic-review-only-v1",
        turn: Source::Metadata, session: Source::Metadata, health_version: Source::Metadata }];
    store.stage_reviewed(AdapterRecord { harness: HarnessKind::Codex,
        version: reviewed[0].version, source }, 1000, reviewed).unwrap();
    store.read_reviewed(1000, reviewed).unwrap().pop().unwrap()
}
