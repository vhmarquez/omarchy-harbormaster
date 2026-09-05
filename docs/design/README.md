# Harbormaster design handoff — M0 / issue #3

**Owner-approved design baseline (2026-09-05):** option 02 Project manager plus the supplemental review/settings/recovery design and UX semantics. The explicit response and scope are recorded in [M0 owner decisions](../M0-OWNER-DECISIONS.md). **Delivered:** frozen original bundle, explicit UX contract, and a nonproduction offline supplemental study. **Not claimed:** that the owner executed tests, complete browser/native UI acceptance, real harness capability parity or production implementation.

## Start here

- Open **[supplemental.html](supplemental.html)** in a regular browser. Everything is fictitious, local and in memory. No agents, shell commands, external requests or persistent browser state. Reload or Reset fixtures resets it.
- Read **[UX-CONTRACT.md](UX-CONTRACT.md)** for view/state/action semantics, native theme mapping, launch/review/settings/recovery, keyboard/focus/accessibility and compact layout.
- Use **[REVIEW-CHECKLIST.md](REVIEW-CHECKLIST.md)** for the recorded design approval, acceptance coverage and remaining technical gates. Approval is separate from browser/native test evidence; full keyboard/scaling/accessibility checks remain future M3/M6 gates.
- Read **[VERIFICATION.md](VERIFICATION.md)** for exact executed checks and limitations.

## Frozen source

`original/` contains the complete supplied bundle. The selected source is `original/02-project-dark.png`, `original/02-project-light.png` and option 02 of `original/harbormaster.html`. Other alternatives and original state boards remain for provenance; the selection is not reopened.

**The supplied PNG/SVG boards are not browser captures.** The original author's browser-verification limitation is retained, not retroactively converted into a pass. See [PROVENANCE.md](PROVENANCE.md), `original-inventory.json` and `original-sha256.txt`. Font license is retained at `original/assets/OFL.txt`.

## Supplemental scope

- Review queue and explicit Mark reviewed/Undo, with folder-open and notification delivery kept independent.
- Reported / Independently verified / Unavailable evidence comparison, all clearly marked fictional.
- Harnesses, Projects & launching, Notifications, Privacy and Advanced/Diagnostics settings studies.
- Onboarding, loading/empty/filtered-empty, missing/limited instrumentation, approval/input, failure/interruption, disconnected/daemon recovery, launch retry and safe confirmations.
- Masking vs collection vs retention; no secure-erasure claim.
- Dark/light and compact controls; separate label/count tracks fix compact navigation crowding. Worktree copy says separate checkout, **not a security sandbox**.

The HTML is a portable mockup, not the production stack. Production must be native Qt Quick/QML in Quickshell, using the user's current Omarchy theme/font rather than these illustrative CSS values.

The owner also accepted **Limited visibility for Codex**, explicitly mapped to issue #5 in the [decision record](../M0-OWNER-DECISIONS.md); runtime issue #4 was already closed. This is a scope decision, not evidence of native Codex callbacks or authorization for credentialed tests. The illustrations do not override the [installed-version capability matrix](../harness-capabilities.md). No M1 work or deployment is authorized by this design approval.

Original assets, supplemental HTML, tests and existing evidence remain unchanged by this approval reconciliation. Any pre-approval wording in those preserved artifacts describes the historical handoff, not the current owner decision.

## Reproduce local checks

From the repository root:

```sh
node --test docs/design/tests/study.test.cjs
python3 docs/design/tests/verify_bundle.py
```

Or from `docs/design/`:

```sh
sha256sum --check original-sha256.txt
```

Tests have no package dependencies or network requirements. Node runs the actual inline model from the HTML. They do not establish rendered layout, native integration or user approval. No application code, git staging/commit/push or GitHub changes belong to this handoff.
