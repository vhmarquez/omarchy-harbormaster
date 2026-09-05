# Design handoff review checklist — issue #3

**Approval boundary (2026-09-05):** [M0 owner decisions](../M0-OWNER-DECISIONS.md) records explicit approval of option 02 Project manager and supplemental review/settings/recovery design and UX semantics, including the stated defaults and focus/privacy policy. This resolves the owner design-approval gate, not the technical test matrix. No claim is made that the owner executed browser/native tests. Issue publication/state is parent-owned; this checklist does not itself change GitHub.

## Artifact map

| Requirement | Reviewable material | Status / limitation |
|---|---|---|
| Three composition alternatives | `original/01-console-*`, `02-project-*`, `03-attention-*`; original HTML option switch | Preserved, option 02 selected; alternatives retained for provenance only |
| Popup and expanded manager | `original/02-project-dark.png`, `02-project-light.png`, original HTML | Supplied static boards, **not browser captures**; supplemental contract fills notification/header/action semantics |
| Launch workspace/conflict flow | `original/04-launch.{svg,png}`, original HTML launch modal | Original wording frozen; contract corrects “isolation,” task-label/prompt distinction, project-first sequence |
| Launch busy/failure/success/idempotency | Supplemental New session → Launch simulation → Stage terminal failure → Retry terminal → Stage success | Actual in-memory reducer tested; no OS/runtime proof |
| Review disposition | Supplemental Sessions & review, initial S3 | Open project keeps Review; Mark reviewed changes counts, pins item; Undo restores; notification delivery tested separately in model |
| Reported vs independently verified | Supplemental Evidence comparison below the window, with S3 selected | Both **fictitious** evidence examples, plus Unavailable; no real verifier or agent result |
| Settings five groups | Supplemental Settings → Harnesses / Projects & launching / Notifications / Privacy / Advanced | Explicit hook simulation confirmation; form changes Apply/Cancel; opt-in export preview only |
| Onboarding, errors and recovery | Supplemental Onboarding & recovery state selector | Distinct staged text and contextual controls, not actual failure injection |
| End session vs worktree cleanup | Recovery → End owned session / Dirty worktree cleanup | Separate dialogs; cancel initial focus; dirty cleanup disabled |
| Masking vs retention | Header Hide text now; Settings → Privacy | Presentation-only masking; retention selection does not collect text or erase files |
| Dark/light and compact | Supplemental top controls; original dark/light and `06-small-window.*` boards | Supplemental compact means list/detail navigation rather than microscopic three columns |
| Component states | Original `05-states.*`; supplemental rows, navigation, forms, dialogs, busy/unavailable actions | Keyboard/selected/busy/disabled/state text covered in contract; native implementation verification not done |
| Long/Unicode/missing icons | Supplemental Running → S6; text-only harness labels | Long multilingual fictitious label; no essential icon dependency |
| Focus/scaling/reduced motion | UX contract §§2,8–9; supplemental roving rows, dialogs, CSS reduced-motion treatment | Browser/AT/native matrix requires real inspection; source checks alone do not prove it |

## Parent browser exercise (do not touch actual sessions)

Open `supplemental.html` in a normal browser. It has no network/service/shell integration and reload resets in-memory data. Use isolated browser session `harbormaster-m0-design` if using Browser Use; do not share another agent's session.

1. **1280×800 dark:** inspect option 02 rail/list/inspector; capture true browser image separately from original boards. Verify no horizontal overflow. Inspector body may scroll; actions must remain reachable.
2. **Review:** initial Needs you 3, Running 4, Review 1, Saved 0 (overlapping views). Open project must leave Review 1. Mark reviewed → Review 0, Needs you 2, Saved 1; item remains pinned and Undo gets focus. Undo → initial counts. Result evidence remains Reported.
3. **Evidence comparison:** Reported → Independently verified → Unavailable. Confirm the verified comparison is visibly called a fictitious example and never claims that this study ran a test. Switch back to Reported.
4. **1024×768:** compact list displays views with separated labels/counts. Select S3 → detail pane and visible Back control. Escape/Back returns to S3. Check controls at 125/150/200% scaling and no clipped labels. At wider width, toggle Compact study for the same transition.
5. **Light:** inspect review, disabled-action explanations, form borders and 2 px keyboard focus, not merely a palette swap screenshot.
6. **Keyboard:** Tab to view/filter/list; arrows/Home/End select rows. Enter opens compact detail. Ctrl+K opens command dialog; Escape closes/restores focus. `/` enters search outside fields. Type ordinary shortcut characters in a field and verify no actions fire. Modal Tab stays inside; Cancel initially focused for destructive confirmation.
7. **Settings:** select all five groups. Change notification or retention choice; Cancel restores and Apply changes only this fixture. Navigation with unsaved edits asks rather than silently applying. Preview/install/remove observer affects only in-memory fixture. Export preview contains no sensitive fixture paths or labels.
8. **Privacy:** Hide text now masks task/project/branch/path names in manager/settings/dialogs and accessible text; counts and state stay useful. Show text restores. Retention remains unchanged. No actual sensitive data is used to test this.
9. **Launch:** double-submit while Busy is blocked. Stage failure → context retained; Retry terminal keeps `fixture-request-01` and one start. Stage success says explicitly that no actual agent or terminal launched. Closing/reopening a busy dialog does not reset its request.
10. **Recovery:** exercise each selector item. Dirty cleanup confirmation has disabled destructive button; End session is a separate confirmation; no approvals, commits, merges or deletion controls exist. Daemon/disconnected scenarios do not assert that runtime stopped.
11. **Freshness and notices:** no fake context percentages; notifications and review independent. Search/no-results, healthy empty and loading explain different conditions. Count views are not additive.
12. **Safety:** inspect console/runtime exceptions and network requests; no scripts/assets fetched remotely, no persistent browser writes, no live process integrations. All displayed data are fixtures.

## Parent browser evidence received

Read-only review of [`../M0-PARENT-VERIFICATION.md`](../M0-PARENT-VERIFICATION.md) confirms independent parent checks: 23 source/copy hashes; the historical eight-test Node run (expanded to 15 after the masking regression fix); rendered 1280×1000 dark review and 1024×768 light settings; Open project leaves review unchanged; Mark reviewed pins the item without changing reported/not-verified classification; rendered fixture label masking; no horizontal overflow in the tested light settings; terminal failure/retry retains `fixture-request-01`; no captured JavaScript errors during those interactions. Parent also recorded RED/GREEN checks for unapplied privacy/notification/workspace drafts surviving masking. This is **partial browser coverage, separate from the later explicit owner approval**. Full keyboard/scaling/accessibility matrix remains open. Final contrast-only token refinements are source-verified; no fresh parent screenshot of those final values is claimed.

## Recorded approvals and remaining technical gates

- [x] Owner approves option 02 plus supplemental review/settings/recovery and UX semantics. Actual response and scope: [M0 owner decisions](../M0-OWNER-DECISIONS.md), 2026-09-05. This records approval, not personal test execution.
- [x] Parent's limited browser layout/interactions recorded in `../M0-PARENT-VERIFICATION.md`; original static boards were not relabeled as captures. Full matrix remains unchecked below.
- [x] M0 runtime evidence reconciled with design: [ADR 0002 / #4](../adr/0002-runtime.md) provides constrained disposable persistence/focus/attach/recovery proof, not production DB persistence, arbitrary-terminal support or saved-conversation resume.
- [x] Harness scope reconciled with the [capability matrix](../harness-capabilities.md) and [owner decision](../M0-OWNER-DECISIONS.md): accept Codex **Limited visibility** for #5. The owner's `#4` clause maps to #5; runtime #4 was already closed. Native Codex callbacks remain unproven; no credentialed-test authorization or uniform harness parity is implied.
- [x] M0 design-policy alignment checked against [ADR 0001 / #2](../adr/0001-product-and-architecture.md), [protocol/domain #6](../PROTOCOL.md) and the [threat model](../THREAT-MODEL.md): verified ownership, separate focus/attach/resume, outcome/review/delivery independence, metadata minimization and no secure erasure. This is document-level alignment, not complete wire mappings or backend enforcement.
- [x] Owner-approved retention design records a 30-day default, active/unresolved protection, explicit cleanup preview/confirmation and masking distinct from deletion; see [UX contract §6](UX-CONTRACT.md#6-settings-contract). No settings were applied.
- [ ] Remaining browser/keyboard/scaling/accessibility matrix in the exercise above completed and evidenced in future M3/M6 UI work, including rendered dark/light/compact, empty/error/privacy and exact focus behavior.
- [ ] Native Qt/Quickshell theme/font/runtime, accessibility tree, actual keyboard routing/panel switching and display-scaling matrix exercised when production UI exists (M3/M6); owner approval is not a pass.
- [ ] Production capability/action gates, durable review/Undo, identity/recovery and UI-to-wire mappings implemented and tested against #4–#6 contracts. Closed M0 feasibility/draft gates do not prove implementation.
- [ ] Backend retention mapping and enforcement completed: the illustrated 7/30/90 choices must be reconciled with [PERFORMANCE.md](../PERFORMANCE.md)'s 20,000-fact/30-day ceiling and separate spool/outbox/tombstone limits. The 90-day option is not yet a supported effective retention promise. Test protected attention/review, atomic generation retirement, cleanup preview and DB/WAL/backups; no secure-erasure claim.

Check only the scope supported by the cited approval or evidence. Earlier outstanding-approval statements in preserved HTML/evidence are historical; they do not override the owner decision. No GitHub edit, M1 implementation or deployment is performed or authorized by this handoff.
