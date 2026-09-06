# Protocol component evidence

These are observed component runs inside the unchanged offline bubblewrap
`scripts.verification.sandbox.run` boundary, using a disposable source snapshot,
state and Cargo cache copied from the dependency agent's pinned verified cache.
They are not current-head combined/Docker/native/paired qualification receipts.
The final receipt identifies the exact selected component/source bytes tested;
Cargo files and the temporary library export came from coordinated integration
and are not owned by this component commit.

- `red.json`: test-first compile RED before the protocol module existed.
- `initial-green.json`: first three strict M0 example/duplicate/framing tests pass.
- `expanded-tests.json`: behavioral RED: Serde's internally tagged *unit* variant
  accepted unexpected fields on `accepted_pending` despite `deny_unknown_fields`.
  An empty struct variant fixes that actual failure; `expanded-green.json`
  records the observed GREEN.
- `check-*` and `clippy-retry.json`: intermediate test/lint/format attempts,
  including lint failures subsequently corrected rather than hidden.
- `final.json` and `final-*.log`: final 14 protocol integration tests, two strict
  parser/writer unit tests plus eight existing CLI unit tests, and two seeded
  malformed-input/property tests pass. Clippy with `-D warnings` and formatting
  pass. No skips, timeout or output truncation. Component check elapsed times
  are compilation-inclusive fixture measurements, not product latency claims.

The deterministic seed is 5206533088969097224. Its 12,000 mutated inputs cover all
five entry points and eight mutation modes, with 1,701 accepted canonical
roundtrips and 10,299 rejections. Every entry point records both accepts/rejects;
every mutation mode executes. Sixty separately asserted malformed structures
exercise twelve hostile families across five entry points. Each entry point
accepts a frame at 16,384 bytes and rejects 16,385. Private parser unit coverage
isolates the depth rule: depth eight accepts and nine rejects even where no typed
wire schema permits such nesting, avoiding the misleading unknown-field proxy.
This finite seeded target is executable hostile-input/property coverage, not
coverage-guided fuzzing or proof that every input has been explored.

Production module responsibilities are data-only identity, strict framing/JSON,
events, controls, capabilities and responses. The control parser uses Serde's
exact internally tagged struct schemas rather than duplicating each operation's
field decoder. Shared turn and target argument records express identical fields;
the event/operation tags remain distinct. All production files are below 300
nonblank lines after formatting, and Clippy's 50-line function limit passes.
The larger protocol integration test file holds readable contract fixture tables
and independent tests for nine events, twelve controls, framing, identity and
capabilities; production complexity limits are not enforced by compressing it.

The protocol exposes no filesystem, IPC, database, clock, process or UI behavior.
Claims and canonical identity strings confer no authority. In particular, health
versions and opaque IDs are bounded metadata, not a secrecy/provenance guarantee.
