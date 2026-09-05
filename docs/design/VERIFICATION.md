# Verification — M0 design handoff

## Current approval and evidence boundary

On 2026-09-05 the owner explicitly approved option 02 and the supplemental design/UX semantics; the verbatim response and its scope are recorded in [M0 owner decisions](../M0-OWNER-DECISIONS.md). This resolves the previous approval gap, not the technical test matrix. It does not establish that the owner executed tests. Codex Limited visibility is accepted for #5, not proven native callbacks or permission for credentialed testing. No M1 work or deployment follows from this record.

The original handoff, failed attempts, eight-test output and post-review evidence below are historical records. Approval reconciliation changes only the five top-level design Markdown documents; originals, supplemental HTML, tests and existing evidence remain untouched.

## Initial handoff checks — historical execution

- Copied the complete supplied bundle into `original/`; compared all **23** copied files' SHA-256 values to source bytes. The inventory contains **4,312,835 bytes**. No original was edited or regenerated. Parent independently rechecked all 23 against the supplied source.
- `sha256sum --check original-sha256.txt` from `docs/design/`: all 23 entries reported `OK`.
- `node --test docs/design/tests/study.test.cjs` from repository root: **8 tests passed, 0 failed, 0 skipped** in the initial handoff. Raw historical output: [`evidence/node-tests.txt`](evidence/node-tests.txt). The suite subsequently expanded to 15 tests; see the post-review addendum and current rerun below.
- `python3 docs/design/tests/verify_bundle.py`: all required checks passed. Raw output: [`evidence/bundle-verification.json`](evidence/bundle-verification.json). It verifies the exact original file set, hashes, sizes, font license, selected option, two inline scripts, 18 recovery selector states, unique static HTML IDs, local study links, compact label/count structure, reduced-motion CSS and absence of the tested network/storage APIs. This is a source check, not proof against every possible network technique.
- Inline JavaScript was syntax-compiled using Node's VM. Model tests execute the actual `study-model` script extracted from the self-contained HTML, not a separately reimplemented fixture.
- Read-only source inspection of native Omarchy `Commons/Color.qml` and `Commons/Style.qml` informed the theme contract. Nothing was installed/reloaded or changed in the desktop.
- `git diff --check -- docs/design/` returned zero; because these are new/untracked files, this is only diff hygiene, not comprehensive source verification.

## Test-first evidence and corrections

Observed RED → GREEN sequences for explicit review/undo, presentation masking, same-request launch retry, action capability gates and explicit/reversible settings. The initial RED was the missing supplemental study; subsequent feature REDs identified missing reducer functions/fields. The UI structural check failed for missing required controls before the markup was added.

A later regression check failed because maskable project options lacked explicit stable values. Adding `value="Beacon"` / `value="Tidepool"` prevents presentation-label changes from changing project identity. The suite at that historical checkpoint passed eight tests.

Source-token contrast checks initially found insufficient dark border contrast and several light roles against selection backgrounds. Supplemental-only tokens were adjusted; original bytes remain unchanged. The final verifier checks text/muted/accent/warning/error against all four opaque background roles at ≥4.5:1 and control border at ≥3:1:

| Minimum across tested backgrounds | Dark | Light |
|---|---:|---:|
| Main text | 8.318 | 5.173 |
| Muted text | 6.361 | 4.966 |
| Accent text / focus | 5.333 | 5.002 |
| Warning text | 6.715 | 4.815 |
| Error text | 5.075 | 5.156 |
| Border | 4.011 | 3.617 |

These are WCAG relative-luminance calculations on CSS tokens, **not** a rendered/native accessibility audit. They do not verify glyph rendering, native controls, antialiasing, selection differentiation or every theme.

One attempted Node invocation used a repository-relative test path while already inside `docs/design/`; it failed to locate the test. It was rerun correctly from repository root and passed. No pass claim relies on the mistaken invocation.

## Browser evidence — parent-owned, scoped

The initial design subagent's Browser Use attempt in session `harbormaster-m0-design` was blocked by Chrome's “Allow remote debugging?” prompt. No permission was accepted by that subagent and no screenshot was produced in that attempt. No browser was used for the subsequent owner-approval reconciliation.

The parent subsequently performed independent real-Chrome CDP checks and recorded them in **[`../M0-PARENT-VERIFICATION.md`](../M0-PARENT-VERIFICATION.md)**. That report was read, not modified. It establishes:

- rendered dark review/list/inspector inspected at **1280×1000**;
- Open project preserves unreviewed disposition;
- Mark reviewed moves Review 1 → 0 while pinning the inspected item and retaining Reported/not-independently-verified classification;
- rendered light settings at **1024×768** had no horizontal document overflow;
- masking hid the fixture task label from rendered main text (not a full accessibility-tree privacy audit);
- staged terminal failure retained context; Retry terminal reused `fixture-request-01` and returned to busy with duplicate submission disabled;
- no JavaScript error events captured during those interactions.

These are the parent's exact verification scope, not a claim that every checklist interaction or viewport passed. The parent's screenshots may predate the final supplemental contrast-only palette refinement; final palette values are source-verified above, not claimed freshly screenshot-reviewed. `browser_layout_verified: false` in the Python output describes what **that source verifier** can prove, not a denial of the separate parent browser evidence.

## Remaining technical verification — approval is separate

Supplemental screens, stated defaults and keyboard/focus/privacy semantics are now owner-approved as design policy. The following checks remain unperformed or incomplete; the complete UI/native acceptance matrix belongs to future M3/M6 work:

- Complete keyboard Tab/roving-focus/disappearing-selection/modal restoration matrix, screen-reader behavior, full accessibility-tree masking, reduced motion in a real browser and 100/125/150/200% scaling.
- Rendered 1280×800 acceptance and all dark/light/compact combinations; parent inspected 1280×1000 dark and 1024×768 light settings, not the entire matrix.
- Native Qt/QML rendering, Omarchy theme updates, panel switching and production runtime behavior.
- Real harness capability parity, trusted native Codex callbacks, production liveness/recovery, actual launch idempotency, persistence, filesystem cleanup or diagnostic export. The constrained runtime spike and limited installed-harness results are recorded separately in [ADR 0002](../adr/0002-runtime.md) and the [capability matrix](../harness-capabilities.md); they are not production proof. HTML success/retry/health/result records are fictitious simulations. Credentialed tests still require separate authorization.
- Backend retention mapping/enforcement, including the illustrated 90-day choice versus the current 30-day retained-fact ceiling, separate tombstone/outbox policies and protected obligations; see [UX contract §6](UX-CONTRACT.md#6-settings-contract). Design approval does not resolve this implementation-contract gap or prove secure erasure.

The supplemental HTML uses document scrolling for long inspector/settings content. Native production must constrain list/detail regions and keep actions reachable per `UX-CONTRACT.md`. The recovery selector stages explanatory states; it is not a complete executable backend state machine. Source checks do not prove every DOM focus-restoration path.

## Design audit and provenance discipline

Source/composition self-audit: **0/10 generic-design tells identified**. No gradient, default violet accent, feature-tile grid, decorative accent rail, blur, monument statistic, icon topper, centered marketing stack, default Inter styling or wrong-surface composition. The small selected-row edge communicates selection, not decorative emphasis. This is a source-based audit informed by the supplied option 02 board, not an independent final-render visual acceptance.

All original PNG/SVG files remain **supplied static boards, not browser captures**. No real prompt/session/terminal data was added. The font's full OFL notice is preserved. No production app code, git staging/commit/push, GitHub edits or parent-owned report modifications were performed.

## Reusable handoff procedure

For future design handoffs: copy all supplied assets unchanged; inventory and hash every file; separate selected direction from new delegated decisions; distinguish static boards from browser captures; put executable fictional models behind test-first checks; test view counts from one fixture source; verify contrast and stable identifiers under masking; record actual browser/native/user-review gates separately. This scoped workflow is recorded here without writing outside `docs/design/`.

## Post-review addendum — historical, before owner approval

Independent review found masking discarded unapplied settings. A fresh-context fix added seven real-inline-UI VM regressions; that post-review Node suite passed **15 tests**. The original `evidence/node-tests.txt` is the historical eight-test run. Parent additionally reproduced RED and GREEN in real Chrome for privacy, notifications and workspace drafts, both privacy masking controls and explicit Cancel. See `../../evidence/review/final-design-review.json` and `../M0-PARENT-VERIFICATION.md`. At that checkpoint supplemental owner approval had no response; it was subsequently supplied in [M0 owner decisions](../M0-OWNER-DECISIONS.md).

## Owner-approval reconciliation rerun — 2026-09-05

The design subagent reran the documented offline checks after the Markdown reconciliation; these were not owner-executed tests:

| Command / location | Actual result |
|---|---|
| `node --test docs/design/tests/study.test.cjs` / repository root | Exit 0; 15 tests passed, 0 failed, 0 cancelled, 0 skipped, 0 todo |
| `python3 -B docs/design/tests/verify_bundle.py` / repository root | Exit 0; all 23 original files / 4,312,835 bytes matched, OFL retained, 2 inline scripts, 18 recovery states, 0 duplicate static IDs, local study links resolved and both palette contrast checks passed |
| `sha256sum --check original-sha256.txt` / `docs/design/` | Exit 0; all 23 entries `OK` |
| `git diff --check -- docs/design/` / repository root | Exit 0; design diff whitespace check passed |

Raw rerun output and scope caveats are retained in the parent-local `harbormaster-m0-evidence/design-owner-approval-result.json`, outside the repository. Existing evidence was not overwritten. The source verifier still correctly reports `browser_layout_verified: false`; no new browser/native/accessibility, real harness or backend-retention test was performed.

For approval-only updates, record the exact owner decision separately, label prior outstanding-approval wording as historical, and split approved policy from executable acceptance. Reconcile against the actual runtime/capability/protocol/retention sources without changing frozen artifacts or turning scope acceptance into technical proof.
