# Harbormaster — design specification

Three desktop compositions and a local, offline interaction prototype. All names, tasks, paths, branches and observations are fictitious. The prototype executes no commands, calls no services, and keeps changes only in memory. Reload to reset.

Open `harbormaster.html` in a regular browser, keeping the adjacent `assets` folder. Use the top controls to compare compositions, themes and a 620px manager. The bottom state selector stages each requested state. These study controls and annotations are outside the product chrome.

## Composition choices

| Option | Hierarchy and geometry | Best fit | Tradeoff |
|---|---|---|---|
| 01 Operations console | 163px project/view rail, dense wide ledger, horizontal selected-session inspector underneath | Frequent cross-project switching with J/K and action keys | Long task names reduce scan density; bottom detail shifts the reading direction |
| 02 Project manager | Project/view rail, list, persistent 330px inspector | General daily use across projects; recommended baseline | Fewer visible rows than the console at the same height |
| 03 Attention queue | Project/view rail, reason-led grouped queue, 350px inspector | Responding to blocked agents and reviewing reported results | Expanded reasons consume vertical space |

The paired bar popup is deliberately consistent across all three. It shows waiting-for-approval entries before disconnected entries, oldest-first within a priority class in a production implementation. The fixture uses a fixed illustrative ordering. It lists at most three entries, and its total can exceed the number shown. Open manager accesses the complete queue. Running counts separate observed activity from unknown activity. Review is never labeled “verified.”

Branding is a small squared H monogram, a quiet name and blue selection accents. No nautical labels, scenery or decoration appear in the operational UI.

## Session identity and views

Needs you: A1 and A2 wait for approval; A3 is disconnected. Running: A4 and A5 report activity; A6 is discovered with unknown activity. Review: A7 and A8 reported completion but have not been reviewed. Saved: A9 has resumable context and no running process. The four view counts reflect the selected project; the bar remains global.

Task title is primary. Status uses a symbol plus text. Harness, project/branch and last-observed age follow, with ownership on its own line. Discovered labels remain visible; they are never silently promoted to Managed. An external task name is explicitly described as a local alias in the inspector. The inspector repeats identity to prevent actions on the wrong checkout.

The demonstration freezes time at 09:41:00. Relative ages describe observations at that snapshot. A real implementation must age them continuously and show absolute timestamps on focus or inspection. Disconnection should supersede stale Working labels without implying that a process stopped.

## Actions and capability gates

| Action | Meaning | Capability needed |
|---|---|---|
| New session | Select harness, project and workspace; create a managed run and open its terminal | Installed harness and valid workspace |
| Open terminal | Focus or open the session's native agent terminal | Known terminal handle or supported terminal-opening target |
| Attach | Connect a terminal to an existing live managed session; no new run | Live session and attachment endpoint |
| Resume | Start a new run from saved harness context | Saved context and evidence the previous process is not live |
| Open project | Open the selected checkout folder in the configured file manager | Known accessible checkout path |

These are capability fixtures, **not claims about actual Hermes, Claude Code or Codex adapters**. Production discovery must negotiate per-session capabilities. The mock A6 has a known folder but no terminal handle, attachment endpoint or saved context, so only Open project is enabled. A3 allows opening the known terminal but disables Attach and Resume while its process state is unknown.

Unavailable buttons are disabled with a persistent, adjacent reason; explanations require neither hovering nor clicking a disabled button. Keyboard shortcuts enforce the same gates. An enabled action in the prototype produces a simulation message. Resume does not falsely change the fixture into a running process.

There is no universal Approve button. Waiting-for-approval copy directs users to the native agent terminal, where they inspect the real request and make their decision. No approval payload or terminal transcript is reproduced here.

## Launch flow

1. New session / N opens a modal. Focus starts in Task name.
2. Choose Hermes, Claude Code or Codex, then Atlas, Fieldnotes or Relay.
3. New worktree is the default. Specify a new branch and base branch; read the proposed folder and harness summary.
4. Existing checkout reveals the main checkout instead. Atlas/main contains the working A4 “Index documentation” session: a persistent warning names that session and explains overlapping edits. Creation stays disabled until the user acknowledges sharing, or selects New worktree.
5. Create session & open terminal adds a simulated managed Working session and selects it in Running. The toast explicitly says no terminal was launched.
6. Cancel or Escape closes the dialog and restores focus without creating a session.

Production validation must check harness availability, branch validity and collisions, checkout existence, permissions, and conflicts immediately before launch. Unknown external activity must prompt a qualified collision warning too. Filesystem work and terminal launch can fail separately: preserve created context, show the failure and offer a terminal retry rather than silently creating a second session. These real-system checks are outside this integration-free prototype.

## Keyboard and small windows

Tab and Shift+Tab traverse controls; Enter/Space activate focused buttons. J/K or Down/Up move the selected row and keyboard focus together. / focuses filtering. N opens launch. T opens terminal, A attaches, R resumes, P opens the project. Shortcuts do not fire while typing or inside the launch modal. Escape clears/exits filtering, closes the popup, or cancels launch. Native modal behavior traps focus; closing returns to the launch trigger.

A 2px accent outline with 3px offset indicates keyboard focus separately from the selected row's tinted fill and 2px left edge. Selection can persist without focus. None of the single-letter shortcuts should become global desktop bindings; an Omarchy bar invocation shortcut should be user-configurable and conflict-checked.

Below 950px, list and inspector stack beside the rail. Below 650px, views and projects wrap into compact navigation rows; list and inspector stack at full width, and metadata wraps. The Small window study control shows a 620px manager even on a wide canvas. All five primary actions remain accessible. The prototype uses document scrolling; a native implementation should give the session region its own scroll area and retain the action footer.

## State vocabulary

| State | Appearance and meaning | Next action |
|---|---|---|
| Waiting for approval | Amber diamond; explicit waiting text | Open terminal to inspect the request |
| Working | Blue dot; last-observed age | Open terminal or Attach |
| Finished · unreviewed | Neutral square; agent-reported completion | Open project and inspect changes; Resume if supported |
| Disconnected | Red cross; last observation and unknown process state | Open known terminal; Attach/Resume gated |
| Limited visibility | Neutral hollow circle; Discovered externally | Open project; unsupported controls carry reasons |
| Loading | Static skeleton lines and discovery message; no usable session actions | Wait for discovery; production timeout should offer retry |
| Empty | Specific no-results/empty-view text | Reset filters or New session |
| Saved | Neutral dash; explicitly not running | Resume saved context |

No success-green completion label, celebratory icon or verification badge is used. Reported completion does not prove correctness. Review disposition is intentionally outside this study: opening a folder alone must never clear the Review queue.

## Visual tokens and provenance

The installed Omarchy Tokyo Night and Catppuccin Latte `colors.toml` files informed the palettes. Local JetBrains Mono Nerd Font informed typography; the prototype bundles the regular font for offline consistency. Dark foreground is Tokyo Night bright foreground, with muted text kept brighter than the stock low-contrast dark foreground. Light warning and success colors are darkened for legibility. Future native UI should resolve semantic tokens from the current Omarchy theme rather than require either palette.

Spacing uses a 4px base with 8/12/16px control and panel intervals. One-pixel structural borders, a two-pixel active edge, minimal three-pixel control rounding, 12px operational text and 11px secondary metadata keep the density close to a native utility. Static boards use a slightly larger presentation scale for legibility. No gradients, glass, charts or oversized cards.

## Deliverables and validation

- `harbormaster.html`: clickable local prototype; no real integrations.
- `01-console-dark.png` / `01-console-light.png`: annotated composition boards.
- `02-project-dark.png` / `02-project-light.png`: annotated composition boards.
- `03-attention-dark.png` / `03-attention-light.png`: annotated composition boards.
- `04-launch.png`: launch and shared-checkout warning board.
- `05-states.png`: state reference board.
- `06-small-window.png`: compact manager and focus treatment.
- Matching SVG files are editable vector mockups, independently drawn from the same design. They are not browser screenshots.

Browser verification was unavailable: headless Chromium could not start in the sandbox, and the in-app browser security policy rejected the local-file URL. No workaround was used. Static boards were rendered and visually inspected; prototype validation uses syntax and fixture interaction checks. Actual browser layout, focus trapping and accessibility behavior still require browser validation.
