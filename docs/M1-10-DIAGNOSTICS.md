# M1 diagnostics responsibility, before implementation

The diagnostics module will project caller-owned `StorageStatus`, `StoragePolicy`
and `RecoveryStatus` snapshots into immutable fixed metadata. It will reject
mismatched storage revisions before returning any projection. Inputs contain
only existing bounded counters, closed policy values and revisions; output will
contain no run/producer/event IDs, source payloads, paths, environment or raw
error strings. It will explicitly retain the recovery unknown-gap flag and
the distinction between rejected attempts and discarded artifacts.

`diagnostics::project` will accept immutable references. Its implementation will
have no worker, filesystem, log, cleanup, collection or policy-mutation handle.
The returned fixed-schema value may be serialized with the existing serde_json
dependency; even maximum numeric values will remain below a tested fixed output
bound. It will not infer healthy observation from empty counters or qualify an
adapter. No CLI mode, collector, daemon, export workflow or M2 behavior is added.

Public integration target `coordination` will test revision disagreement,
maximum output size and repeated diagnostic projection/formatting using actual
caller-owned storage/recovery snapshots. It will compare fixture bytes, modes,
mtimes and policy/status before and afterward, and check that private fixture
content/path canaries never enter output. Root owns coordinator pipeline tests
and canonical inventory registration. Tests run in the existing isolated
verifier using prepared private dependencies; no raw host product tests.
