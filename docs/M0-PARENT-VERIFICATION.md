# Parent verification — M0 working evidence

Owner approval was subsequently received on 2026-09-05: [MIT, supplemental design approval and accepted Codex Limited visibility](M0-OWNER-DECISIONS.md). The technical evidence below predates that decision and is retained unchanged in scope. Approval resolves the M0 owner gates; it does not turn limited tests into native callback, accessibility or production verification.

## Contract/fixture checker

`python3 -B -m unittest discover -s tests -v` was run after each checker change. First RED: missing checker assertion failed. First GREEN: required scenario coverage check passed. Second RED: mislabeled/duplicate/incomplete fixture data was incorrectly accepted. Second GREEN: both tests passed after adding explicit provenance, uniqueness and completeness validation.

`python3 -B scripts/verify-m0.py` returned `{"passed": true, "errors": []}` for the 14 synthetic failure specifications. This checks fixture-document integrity, not future daemon regressions, a full protocol schema, or M0 completion. Protocol/performance are draft contracts and unmeasured budgets.

After independent review, a separate fix context enforced exact integer schema version, structured malformed-input errors and deletion protection for every scenario. Parent independently reran all 10 checker tests. The post-review final aggregate run contains 41 passing tests plus two integrity checks in `evidence/parent/final-tests.json`; it does not imply M0's owner approval gates have passed.

## Original bundle and supplemental model tests

Parent compared SHA-256 for all 23 supplied files against `docs/design/original/`: every file matched exactly, with no missing files. `node --test docs/design/tests/study.test.cjs` passed 8 tests with 0 failures and 0 skips (review/undo, masking, explicit settings, capability gates, counts, stable project values and same-request launch retry). These are design-model tests, not native application tests.

## Real runtime and installed harness reruns

Parent independently ran:

    python3 -B -m unittest discover -s spikes/runtime -p test_identity.py -v
    python3 -B -m unittest discover -s spikes/runtime -p test_runtime.py -v
    python3 -B -m unittest discover -s spikes/harnesses -p test_probe.py -v
    python3 -B spikes/harnesses/run.py --out <private-parent-evidence-file>
    python3 -B -m unittest discover -s spikes/harnesses -p test_live.py -v

Results: 2 runtime identity tests, 2 real lifecycle/desktop runtime tests, 5 harness helper tests and 1 installed harness inventory/startup test passed, with no skipped tests. The runtime integration suite completed in 2.158s; this is test duration, not an application latency benchmark. Sanitized read-back artifacts are under `evidence/parent/`.

Every runtime check/cleanup field in the parent lifecycle and desktop reports was true: independent runtime ownership, private socket, exact focus/attach/reattach, UI/daemon restart and daemon SIGKILL survival, stale/wrong-target rejection, no duplicate recovery, runtime stop terminating its worker, owned resources removed, original focus restored, default tmux unchanged. The UI/daemon/harness worker are explicit fixture stand-ins around real systemd/tmux/foot/Hyprland, not the future product.

Harness evidence:

- Hermes v0.21.0 installed plugin loader: bounded allowlisted callbacks exercised with **synthetic lifecycle dispatch**, reversible enable/disable and unrelated fixture configuration preserved. Not a live generated turn.
- Claude Code 2.1.260: actual offline `--init-only` emitted Setup, SessionStart and SessionEnd; configuration restored byte-exactly. No prompt/model request.
- Codex 0.153.2: actual read-only app-server hook discovery and native CLI startup exercised. **Native startup stopped at authentication; command hooks remain untrusted and no native callback was observed.** No credentials, fake provider responses, trust hashes or approval bypass were used. The test passes for honest inventory/gap detection, not Codex integration completion.

The Codex native callback portion remains technically blocked. Issue #5 is accepted only under the owner's explicit Limited visibility scope revision, not because the inventory test is green. Trusted callback qualification remains future work.

## Supplemental browser checks

Parent opened `docs/design/supplemental.html` in real Chrome via CDP and directly inspected rendered screenshots:

- 1280×1000 dark review/list/inspector layout.
- Open project left the finished outcome unreviewed.
- Mark reviewed changed Review from 1 to 0 while pinning the inspected item; the reported/not-independently-verified classification remained unchanged.
- 1024×768 light settings layout had no horizontal document overflow. Masking hid the fixture task label from rendered main text; this was not a full accessibility-tree privacy audit.
- Launch → stage terminal failure retained created-context messaging; Retry terminal reused `fixture-request-01` and entered busy with duplicate submit disabled. This is simulated design logic, not proof that a real runtime did not duplicate.
- No JavaScript error events were captured during these interactions. Native Qt/QML rendering, screen-reader behavior, full Tab/focus matrix and production timing remain unverified.

The first launch probe clicked the study's failure control before submitting; that did not stage a failure. The corrected sequence above was actually exercised. No success claim relies on the incorrect sequence.

## Review-discovered regressions and parent browser recheck

Independent review found the runtime probe finalizer could abandon later cleanup/export if one service stop timed out. A separate fix added independent guarded attempts, structured failure reporting and six fault tests. Parent reran all ten runtime tests, including real service/desktop operations; current source fingerprints match the updated canonical and post-review evidence and all cleanup readbacks passed.

A separate UI review found presentation masking discarded unapplied settings. Parent reproduced the defect in actual Chrome: retention `7`/dirty true became `30`/dirty false after the header mask. After the fresh-context fix and page reload:

- Unapplied retention `7`/dirty true survived both header and Privacy mask controls; explicit Cancel reset it to `30`/dirty false, proving masking did not apply it.
- Unapplied notification `muted` and workspace `checkout` values each survived header masking with dirty true; only explicit Cancel cleared them.
- No page error events were captured during these corrected interaction checks. A first diagnostic query mistakenly referenced a nonexistent `model` variable; the actual reproduction and verification used DOM values and the existing dirty-state binding instead.

These are mockup browser checks, not native application behavior or personal owner approval. The failed reviews are retained as pre-fix evidence; final independent verdicts are recorded separately.

These checks do not constitute personal user review of new supplemental screens. Original option 02 approval is recorded separately. No real commands/agents were launched by the HTML.
