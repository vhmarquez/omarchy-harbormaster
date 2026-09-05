# Harbormaster UX contract — M0 / issue #3

Status: **owner-approved option 02 Project manager and supplemental design/semantics, 2026-09-05.** [M0 owner decisions](../M0-OWNER-DECISIONS.md) records the explicit approval of issue #3, including review/settings/recovery and this UX contract. It is not evidence that the owner executed a test or that an application exists. Read with roadmap #1 and product/security decision #2. Runtime #4 and protocol/domain #6 have completed their scoped M0 gates; #5's Codex Limited visibility is owner-accepted, not native callback qualification. Native UI and full keyboard/scaling/accessibility verification remain future M3/M6 gates.

## 1. Authority and scope

- Preserve the full supplied bundle in `original/`, including rejected alternatives and licenses. Do not fix or regenerate those bytes. `original-sha256.txt` and `original-inventory.json` identify the frozen source.
- `original/02-project-{dark,light}.{svg,png}` and option 02 of `original/harbormaster.html` establish the selected composition. Other supplied boards establish reference states, not production behavior or browser evidence.
- This contract supersedes ambiguous **copy and interaction semantics**, not the selected composition. Supplemental additions live in `supplemental.html`. All examples are explicitly fictional; `/fixture/…` paths are not real project paths.
- Production is native Qt Quick/QML in Quickshell backed by the local Rust manager. No browser, web dashboard, transcript viewer, embedded terminal, credential broker or automatic approval layer. Mock buttons never execute commands, call agents, navigate to real artifacts or write browser storage.
- Do not infer tested adapter capabilities from any illustration. A fixture's enabled button means “this hypothetical adapter/session supplied the required evidence.” Installed-version capability results must come from #5, not this HTML.
- The [owner decision](../M0-OWNER-DECISIONS.md) accepts Codex **Limited visibility** for issue #5; the owner's `#4` clause refers to this Codex decision, not the already-closed runtime issue #4. Keep unsupported signals/actions disabled per the [capability matrix](../harness-capabilities.md). No native Codex callback proof or credentialed-test authorization follows from acceptance. This contract does not authorize M1 implementation or deployment.

## 2. Selected composition and responsive layout

Primary surface: **Operate**; monitoring and command/inspect are secondary. Keep the option 02 left project/view navigation, central stable session list and right inspector. Task label outranks harness, project/branch, state and observation age. Profile is secondary harness metadata, never session identity.

**1280×800 logical reference:** approximately 176 px navigation, flexible central list (minimum approximately 320 px), 330 px inspector. Toolbar and action footer remain reachable. Native list and inspector scroll independently. Avoid nested scrolling inside individual rows.

**1024×768 logical compact:** inspector becomes an in-place detail pane opened with Inspect/Enter and a visible Back to sessions control; do not show an unreadably thin three-column layout. Preserve selected ID and scroll position on Back. Project selector and view navigation may move to full-width rows. View labels never concatenate with counts: each uses a two-column layout `minmax(0,1fr) max-content`, at least 12 px separation, nonshrinking tabular count, sufficient padding. Wrap navigation into two columns when necessary. Do not make fonts smaller to fit. Count accessible name is “Review, 1 session,” not “Review1.” Long counts use visible `99+` only with the exact count available in accessible text/detail; no invented totals.

**Below compact / high scaling:** single-column forms and list/detail; dialog and popup constrain height to available screen and scroll body while keeping actions reachable. Target sizes are logical, not physical pixels. The HTML's Compact study toggle demonstrates this detail transition at a narrow width without changing the real desktop.

## 3. State model — independent dimensions

Persist and project separately:

| Dimension | Vocabulary / invariant |
|---|---|
| Identity | Stable session ID, conversation ID when known, run generation, turn ID, harness; optional profile and verified parent association |
| Ownership | Managed = explicitly launched through Harbormaster with verified runtime ownership; Discovered = observed externally, not adopted or controllable by default |
| Runtime | Starting, live, exited, unknown; boot/start identity prevents stale PID targeting |
| Turn | Unknown, idle, working, waiting input, waiting approval, finished, interrupted, failed; finished is an observed outcome, not task correctness |
| Observation | Fresh / stale / disconnected / limited visibility, source, last-observed absolute time and age; missing observations never imply idle/success |
| Attention | Durable reason set: input, approval, failure, disconnection, interruption, unreviewed outcome; explicit acknowledgment separate from underlying reason |
| Review | Per completed turn: unreviewed or manually reviewed, actor/time, reversible disposition; a later turn creates a new review item |
| Result evidence | Reported / Independently verified / Unavailable with provenance and scope; review disposition must not alter evidence classification |
| Notification | Queued, sent/uncertain, delivered if known, retrying, expired, suppressed/snoozed; independent of viewed/acknowledged/reviewed |

**Views are projections, not one exclusive enum.**

- **Needs you:** any actionable unresolved reason, including completed-but-unreviewed outcomes (roadmap #1). Priority: approval/input, new failures, lost visibility, review. Deduplicate by session in badge counts even when several reasons apply.
- **Running:** known live managed or discovered runtime, including idle, waiting approval and limited observation; distinguish “Working,” “Live · idle” and “Live · limited visibility.” Never count unknown/disconnected as confirmed activity. Working is a subset, not the view total.
- **Review:** completed unreviewed turns, also surfaced in Needs you. Show pending-turn count if a session has several. Opening a folder, inspector, popup or notification does not disposition it.
- **Saved:** no verified live process, saved conversation known, resumable only if supported and liveness reconciled. Reviewed/exited outcomes with context remain discoverable here. Unknown liveness is not Saved. Retain nonresumable history in search/history with “Resume unavailable,” not a fake live state.

This deliberately refines the original study's mutually exclusive grouping: its Running included unknown activity and its Needs you omitted Review. Frozen counts describe the original fixture only. Do not sum the new view counts as if they were distinct sessions.

Supplemental baseline fixture has six sessions: S1 Hermes/Beacon working; S2 Claude Code/Beacon waiting approval; S3 Codex/Tidepool finished unreviewed and exited; S4 Hermes/Tidepool live idle; S5 Codex/Beacon discovered disconnected, runtime unknown; S6 Claude Code/Tidepool discovered live with limited visibility. All capabilities and observations are hypothetical. Initial global counts are derived from the model, not separate UI constants. Project filters affect view counts; text search affects displayed-result counts only; bar attention stays global.

## 4. Actions and capability gates

| Label | Exact operation | Required evidence / guard |
|---|---|---|
| Open terminal | Focus the exact existing live terminal; does not start a process | Verified terminal/window association, revalidated immediately before action; focus failure offers explicit alternatives |
| Attach | Open/connect a terminal to an existing detached managed runtime; no new harness process | Managed ownership, live runtime and supported attach endpoint; if already attached, focus is the preferred action |
| Resume | Start a new process for an existing saved conversation | Tested resume capability, context and confirmed previous process exit; no resume while liveness unknown |
| Continue | Contextual navigation to Open terminal, Attach or Resume, with the actual operation named before activation | Never silently choose process creation; use explicit concrete labels in v1 |
| Open project / Open worktree | Open a validated checkout folder | Allowed, available canonical path; not an acceptance or review action |
| Open artifact / issue | Open a user-associated safe link or approved local artifact | Constrained scheme, root and ownership validation; external handler confirmation where applicable |
| Mark reviewed | Explicit human disposition of the selected completed turn | Show task/turn identity and “Does not verify checks, accept a merge, or mark the task complete”; Undo available and durable |
| Acknowledge failure | Mark the alert seen, not the process healthy | Preserve unresolved reason/history and outcome; distinct from snooze and review |
| Interrupt turn | Ask a supported owned run to interrupt the current turn | Verified identity + tested capability; never blind keystrokes; state changes only after observation |
| End session | Gracefully terminate the specific owned runtime, then bounded explicit escalation if needed | Confirmation naming task, harness, checkout; revalidate identity; never exposed for discovered/unknown targets |
| Detach | Disconnect the terminal client while runtime continues | Supported terminal/runtime client association; distinct from ending or closing the manager |
| Clean up worktree | Explicit Git worktree cleanup only | Separate confirmation; check tracked/untracked changes, active writers and associations again; dirty, busy or unknown blocks destructive confirmation |

No universal Approve, vague Stop, automatic commit/push/merge or automatic worktree removal. Approvals stay in each harness's native terminal. Disabled actions have a persistent nearby explanation and accessible description, including keyboard command results. A stale action request must fail safely and leave the selection intact.

Closing the UI does not end managed sessions. [ADR 0002 / issue #4](../adr/0002-runtime.md) establishes independently owned runtime continuity for the disposable tmux/user-service/standalone-foot/Hyprland matrix, not production persistence or arbitrary-terminal support. Durable registry/reconciliation and actual Rust/QML restart behavior remain implementation tests. Logout policy must be explicit, never silently enable lingering. Suspend/power-off does not keep computation running. Reboot recovers metadata and resume choices, not old processes.

## 5. Primary journeys

### Attention and popup

Bar shows global attention and confirmed working count, plus explicit stale/unknown signal; decorative glyphs are never the sole labels. Popup is approximately 420 logical px, screen-height constrained. Header: product, connection/freshness, notification state (“Enabled,” “Muted,” “Snoozed until …,” or “Delivery uncertain”). Needs-you items precede a short running list; show at most three attention entries and a clearly labeled full total. Each row carries task, harness, project, reason and appropriate primary action. Footer: New session, Next needing attention, Open manager. Selecting a row opens its inspector with stable ID; it does not acknowledge anything.

Next needing attention cycles the current durable queue, preserving selected ID if new events arrive. Native Omarchy panel-switch shortcuts remain the shell's responsibility. Left/Right may switch panels only when shell convention permits and no text field or nested control owns that key. Escape closes this popup and returns focus to the bar invoker. No single-letter command becomes a global shortcut; offer an opt-in, conflict-checked binding rather than overwriting an existing one.

### Safe launch

1. Project: choose/register explicit trusted root; no repository-defined command is trusted automatically.
2. Harness: show installed/unsupported/missing and capability explanation. Only supported options/profile fields are shown; “Use harness default” rather than forced model selection. No login or credential collection here.
3. Workspace: existing checkout or explicit new worktree; branch/base/path preview. **“Separate checkout for parallel edits. Not a security sandbox; agents still run with your user permissions.”** Replace the original's “for isolation” copy in production/supplemental only.
4. Task label, optional issue URL. The label is metadata, not an initial prompt. Initial prompt deferred unless private supported transport exists.
5. Summary and conflict check: name concurrent writers, distinguish verified and unknown activity. Existing shared checkout requires explicit acknowledgment or choose new worktree. Validate path/branch collision, permissions, trusted executable and latest runtime state immediately before launch.
6. Submit: busy state, disabled duplicate submit, stable request ID. Do not retry a launch as a fresh request after timeout. Cancel before submission changes nothing; closing after submission does not cancel a possibly created run.
7. Success only when runtime identity is established; “Open terminal” can still fail independently. If worktree/runtime exists but terminal failed, preserve that context and offer Retry terminal/Attach on the same request. Unknown outcome offers Reconcile first, not Launch again. Pre-runtime failures retain form data with field error and an explicit retry after correction.

The supplemental launch study stages busy, terminal failure and successful terminal retry with the same fictional request. It does not create an extra fixture session or claim an actual launch occurred.

### Completed work and disposition

Show turn identity, observed completion time, project/branch/worktree, optional issue/artifact links, changed-file summary if safely available and Context/Last context only when reliable. Otherwise **Unavailable**, never fabricated 0%. A changed-file summary is metadata from a safe local inspection, not inferred from the agent's narrative.

Two separate evidence treatments:

- **Reported:** “Agent reported checks passed. Harbormaster did not run or independently verify them.” Include reporter and report time when known; never label user review as verification.
- **Independently verified:** only show with a real authorized verifier record, exact scope, command/test identity, outcome, timestamp and source revision/worktree identity. Unverified broader work remains unverified. The supplemental comparison explicitly labels its evidence record fictitious; switching the study selector does not run tests.

Open project/artifact/terminal leaves Review unchanged. Mark reviewed deliberately clears only that completed turn's pending review and updates derived counts. Keep the inspected item temporarily pinned with “Reviewed · no longer in this view” and Undo, instead of moving focus unexpectedly. Navigate away to remove the pin. A later turn is unreviewed again. “Reviewed” does not mean task accepted, verified, committed or merged.

## 6. Settings contract

Settings uses five persistent navigation groups. Native forms use Apply/Cancel for edits, inline validation and visible unsaved state. Navigating away offers Keep editing / Discard; no silent persistence. Hook/cleanup/export operations have their own explicit confirmation. The study intentionally only simulates a small subset of these operations; defaults below are part of the [owner-approved M0 design](../M0-OWNER-DECISIONS.md), not applied user configuration or implemented backend policy.

| Group | Fields, defaults and states |
|---|---|
| Harnesses | Executable/version, supported capability health, installed but unsupported, executable missing, observer missing, observer healthy. Explicit preview/consent for observational-hook installation/removal scoped to selected harness/profile; preserve unrelated hooks, profiles and permissions. No auto-install or automatic adoption. Reprobe is read-only and shows busy/failure. |
| Projects & launching | Trusted roots (display path), supported executable, default workspace = new worktree, supported terminal choice, project removal leaves files/runtimes intact. Shared-checkout warning cannot be globally bypassed. Trust additions require the actual path and explicit consent. |
| Notifications | Enabled by default with generic text only; allow Mute and timed Snooze. Respect desktop DND. Quiet delivery never clears durable attention/review. Show retry/expiry/overflow and uncertain delivery; offer retry only when safe. No exactly-once human-visible delivery claim. |
| Privacy | No prompts/responses/terminal/environment/credentials captured. Hide text now masks task labels, project/path/branch, URLs and metadata in popup/manager/accessibility names/notifications, but does not delete storage. Approved design default for completed metadata retention = 30 days (illustrated 7/30/90 choice), with active and unresolved items protected and cleanup preview; effective backend limits need the reconciliation below. Presentation, collection, retention and deletion are separate sections. |
| Advanced / Diagnostics | Read-only connection/schema/capability health. Sanitized diagnostics preview, explicit local export and path selection; no automatic upload, environment dump, transcripts, full paths or task labels by default. Export error preserves preview and permits retry. Retention/cleanup describes DB/WAL/backups: deletion is not guaranteed secure erasure. No raw command field, root privilege or hidden dangerous switches. |

Retention expiry must not silently destroy active/unreviewed obligations. Cleanup lists what is eligible and protected, requires explicit confirmation, and concerns manager metadata only. It does not delete harness history, files, worktrees, logs or backups belonging to other tools. Worktree cleanup is a different operation. Masking does not secure in-memory data from scripts, disk, screenshots made before masking or same-UID processes.

**Cross-contract boundary:** [ADR 0001](../adr/0001-product-and-architecture.md) and the [threat model](../THREAT-MODEL.md) support metadata minimization, independent review/delivery and no secure-erasure guarantee. [Protocol v0](../PROTOCOL.md) separates history, replay-protection tombstones, attention and pending notifications; [performance limits](../PERFORMANCE.md) cap retained facts at 20,000 or 30 days, whichever first, and give spool/outbox their own limits. The 30-day design default aligns at the policy level, but the illustrated 90-day choice is **not** a promise to exceed that backend cap. Mapping each retention choice to record classes/effective limits remains unresolved before implementation; do not silently raise a ceiling or present an ineffective choice as supported. Tombstone retirement/reconciliation, unread/outbox protection, cleanup preview and DB/WAL/backup behavior still need backend implementation and tests. Owner approval does not establish those safeguards as working.

## 7. Recovery, empty and failure states

| State | Honest content | Available recovery |
|---|---|---|
| First run / onboarding | No projects or adapters configured; no scanning private content | Choose trusted project → choose harness → preview optional observer changes → launch; skip observer with limited-visibility explanation |
| Loading | “Loading session metadata…” with static skeleton/status; no false zero/success | Timeout explains failure and offers read-only Retry |
| Healthy empty | “No sessions in this view”; connected/fresh | New session; no alarm styling |
| Filtered empty | “No sessions match these filters” and current scope | Clear search / reset scope, keeping unrelated selection history |
| Missing harness | Named executable missing, not stopped session | Harness settings; preserve task/launch draft |
| Unsupported / missing instrumentation | Distinguish unsupported version from absent hooks | Read capability details; explicitly consent to repair only if supported |
| Waiting input / approval | Explicit reason; no captured prompt/request body | Open terminal; approve only there |
| Interrupted / failed | Last confirmed outcome and what remains unknown | Inspect / acknowledge; resume only after liveness confirmed; never auto-retry work |
| Disconnected | Last observation and “Current activity unknown” | Read-only recheck, known verified terminal; no destructive controls |
| Missing terminal association | Session exists, exact window unknown | Attach if live managed; manual instructions otherwise; never focus a guessed terminal |
| Daemon reconnect | Cached metadata stale, control requests disabled until resnapshot/sequence reconciliation | Retry connection; do not relaunch agents or clear attention; preserve view/filter/selection |
| Launch busy / unknown | Request in progress / outcome uncertain | Reconcile same request; duplicate submission blocked |
| Terminal launch failure | Created context retained, terminal failed | Retry terminal or Attach without a second harness run |
| Dirty/busy worktree | Name tracked/untracked changes and writers when known | Open worktree / Cancel; destructive confirmation disabled, no force checkbox |
| Storage failure / full disk | “Could not save disposition/settings”; result not durably accepted | Preserve visible pending state, Retry; do not optimistically remove attention without durable acknowledgment |
| Privacy masked | Generic labels, state and counts remain usable | Show text; retention unchanged; clipboard/accessible names also sanitized |

Freshness uses a backend supplied timestamp and capability-specific policy. Do not invent a universal stale threshold here. Relative age updates must not reorder rows or trigger persistence. Absolute observation time is available in inspector without a tooltip dependency.

## 8. Keyboard, focus and accessibility

- Use native semantic controls: Buttons, labeled text fields/selects, list/row semantics, dialogs, navigation groups. Qt `Accessible.name`, `description`, role, checked/selected/expanded/disabled/busy states map to what is visible. Text uses plain-text rendering; escape fixture/external data. No trust in HTML/Markdown supplied by agents.
- Tab/Shift+Tab traverse toolbar → views → project filter → session list → inspector/actions. A list has one roving focus stop; Up/Down move focused selection, Home/End go to boundaries. Pointer and keyboard share the same action gate. Selected fill and keyboard outline are independent.
- Ctrl+K opens discoverable command palette; `/` focuses search only outside editable fields. New session is visible; optional N/T/A/R shortcuts are in-window, documented and inactive in inputs, selects, contenteditable areas, menus and modals. Enter inspects the focused row at compact width; Escape returns from detail to the same row.
- Popup uses native panel navigation conventions; manager does not intercept shell panel-switch keys. No shortcut intercepts typing or requires a US keyboard character to complete a workflow.
- Dialog focus begins on the first meaningful field, or Cancel for destructive confirmations. Trap Tab only while modal; Escape cancels when no irreversible request has been sent. Closing restores the exact trigger or nearest surviving equivalent. Busy requests are not silently canceled by Escape.
- Updates do not steal focus or reorder the selected target. State changes preserve ID. If the selected item leaves a filter, keep it pinned until navigation with an explanatory note. If it is actually removed, focus the next surviving row at the same index, then previous, then the empty-view heading; announce once. Never transfer a held Enter/Space to another session.
- Announce connection/action outcome in a small polite live region, errors assertively only when user action fails; do not make the entire inspector a live region. Reduce motion means no animated skeleton, reorder or smooth auto-scroll; animation is not required in this study.
- Visible focus: 2 logical px accent outline and offset, not color alone. Text target contrast ≥4.5:1, large text ≥3:1, focus/control boundaries ≥3:1 against adjacent surfaces. Disabled explanations remain readable. Desktop controls at least 32 logical px high, roomy rows and 44 px targets where touch is supported.
- Long/Unicode labels wrap or elide with full detail available by focus/inspection. Use bidirectional isolation for identifiers and paths. Missing icons fall back to text; status symbols are decorative when text duplicates meaning. Test keyboard-only, no icons, reduced motion, 100/125/150/200% scaling, dark/light and 1024×768.

## 9. Native theme translation (not a hard-coded production skin)

Reuse installed Omarchy `Color`, `Style` and font conventions after reading the actual supported component sources. Source inspected for this handoff: `/usr/share/omarchy/shell/Commons/Color.qml` (foundational foreground/background/accent/urgent/muted; surface roles including `Color.popups`) and `Commons/Style.qml` (theme typography/spacing, corner radius and independent normal/hover/selected/focus tokens). This was read-only inspection, not native runtime compatibility verification. Token API names are integration candidates, not an import/version compatibility claim. Map semantic surface/base/panel/selection/foreground/muted/border/accent/warning/error roles to current theme. Respect runtime theme/font changes without reconstructing session state. Readable disabled/error/focus treatments may need contrast-safe derivation per theme; never silently fall back to Tokyo Night as production policy.

The supplied study used Tokyo Night / Catppuccin Latte values. Supplemental HTML reuses those palettes and 4 px rhythm, 1 px borders, 3 px control radii, restrained H mark, compact ledger and detail layout. It uses deliberate desktop sans body text with monospace paths, unlike the original's all-monospace study. These are **portable illustration values only**. Production UI font follows the system, with monospace limited to identifiers. Do not copy CSS into QML as a static theme or ship these fonts as mandatory UI settings.

## 10. Verification and review gate

`VERIFICATION.md` distinguishes executed source/hash/model checks and partial parent browser evidence from the [explicit owner approval](../M0-OWNER-DECISIONS.md). `REVIEW-CHECKLIST.md` records the approved design/semantics separately from unperformed technical checks. Approval covers the supplemental baseline and its stated defaults/keyboard/focus/privacy policy; it does not establish that any of those behaviors worked in native rendering or that the owner exercised them. Complete browser/keyboard/scaling/accessibility and native Qt/Quickshell acceptance remain future M3/M6 gates. Pre-approval notes in preserved assets/evidence remain historical, not outstanding owner approval requests.
