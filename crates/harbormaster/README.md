# Harbormaster manager and terminal runtime

The owner-approved #11 registry is complete. The #12 candidate adds a runnable
manager and independently owned terminal jobs. Project, preset and task commands
use SQLite; with the manager running, clients use its private Unix control socket.
The installed native UI and Hermes observer follow in M3.

## Build

With the [prepared pinned tools](../../docs/TOOLCHAIN.md), including SQLite:

```sh
env -i PATH=/usr/bin python3 -B scripts/build-cli.py \
  --tools "$PWD/.tools-m1-9" --output /absolute/new-build-directory
```

The output directory must not exist; its parent must exist. The command builds
and checks help inside the existing offline bubblewrap boundary, then exports
`harbormaster`, a checksum and build records. It does not install or start a
service. On another machine, pass that machine's prepared tools directory.
Build records do not replace required verification.

## Use

Run the exported binary; quote paths and labels containing spaces:

```sh
/absolute/new-build-directory/harbormaster project add /absolute/project "My project"
/absolute/new-build-directory/harbormaster project list
/absolute/new-build-directory/harbormaster preset add coding /absolute/bin/hermes default
/absolute/new-build-directory/harbormaster preset list
/absolute/new-build-directory/harbormaster task add PROJECT_UUID coding "Implement settings"
/absolute/new-build-directory/harbormaster task list PROJECT_UUID
/absolute/new-build-directory/harbormaster task plan TASK_UUID
```

Use the returned `Project.id` and `Task.id` values. Commands return JSON.
List pages contain `items`, `revision` and `next_after`; supply the cursor as
an extra final argument to continue. If the revision changes between pages,
restart listing. Pages contain at most 100 entries. Capacity is 1,000 projects,
100 presets and 10,000 tasks; exhausted capacity rejects changes without cleanup.

A project uses an existing absolute, normalized directory without symlinks.
The preset explicitly trusts a regular executable named `hermes`, with safe
ownership/permissions, and a profile name of at most 64 ASCII letters, digits,
hyphens or underscores (no leading hyphen). The stored executable/profile is
user-selected metadata, not proof of installed-version compatibility. Nothing
reads project configuration, changes Git state or installs a Hermes profile.

`task plan` rechecks saved directory/executable identities and returns exactly
`-p PROFILE chat`, the working directory and task identity. Labels are metadata,
never prompts or arguments. The plan command executes no harness. The in-memory
`PreparedLaunch::command` builds an unstarted argv-based process with explicit
HOME, fixed system PATH and bounded terminal/locale settings. It does not copy
API keys, loader overrides, PYTHONPATH or HERMES_HOME from the parent. Native
Hermes retains its own profile-based authentication; real runtime environment
is limited to the supported profile/terminal path. At actual launch, the worker
pins the registered working directory and executable, enters the held directory
and executes through the held descriptor. Executable scripts must support this
Linux descriptor path; installed-version Hermes qualification remains M3.

State lives in `$XDG_STATE_HOME/harbormaster`, falling back to
`$HOME/.local/state/harbormaster`. Missing state parents are created privately.
Existing XDG parents may be 0755; the manager directory stays 0700 and its files
0600. Symlinked or other-user-writable paths are refused, without changing their
permissions. Only one process owns the database. Registry commands route through the manager when it is running; otherwise they
can open the existing worker directly. A busy database or unsafe/stale socket
fails explicitly.

Repeated registration with the same contents returns the saved record. Changed
contents/identities conflict; rename, replacement and deletion are not yet CLI
operations. Worker timeouts are unknown outcomes: inspect saved state before
retrying. Help and invalid command shapes do not open storage. Diagnostics use
fixed errors without echoing supplied values; listing intentionally returns the
explicit metadata the user registered. No transcripts or environment values
are stored in the registry.

## Verification

The existing `rust-integration` check includes the actual CLI persistence,
validation and synthetic child-environment flow. The library suite covers the
V3-to-V4 migration and retained backup/outcome behavior. Full qualification uses
unchanged `scripts/verify.py`, locked Docker CI and separate mandatory native
execution. Raw development logs remain outside the source diff. Workspace
unsafe-code and existing lint/security/license policies remain in force.

## Start the manager and launch a task

Start the exported executable in a terminal (no installation required):

```sh
/absolute/new-build-directory/harbormaster manager serve
```

In another terminal, register the project, preset and task using the commands
above, then use the returned IDs:

```sh
/absolute/new-build-directory/harbormaster run launch TASK_UUID --logout-policy existing
/absolute/new-build-directory/harbormaster run list PROJECT_UUID
/absolute/new-build-directory/harbormaster manager status
/absolute/new-build-directory/harbormaster manager stop
```

`manager stop`, SIGTERM and SIGINT stop the manager gracefully and release its
sockets/database. Each job's foreground tmux server belongs to its own transient
`harbormaster-runtime-<run-id>.service`; manager and CLI lifetime never own that
service. Restart `manager serve` and list the same saved job. Hard-kill/stale-socket
recovery remains #15; startup refuses existing sockets rather than guessing
which entries can be removed.

Runtime prerequisites are Linux, an available systemd user manager, tmux and
an existing owned mode-0700 `$XDG_RUNTIME_DIR`. The observed system is systemd
261 and tmux 3.7c; other platforms/versions are not claimed qualified. Missing
prerequisites return a fixed error. `HARBORMASTER_RUNTIME_DIR` optionally selects
another existing private IPC/runtime root, useful for disposable instances; it
does not redirect the systemd user bus. State still uses the XDG state location.

A launch requires explicit `--logout-policy existing`: accept the existing user
manager/logind lifetime policy. Harbormaster does not enable lingering, promise
logout survival, or change session dependencies. Suspend pauses computation;
reboot ends processes. These are separate from manager restart continuity.

Each named task currently has one durable launch attempt, capped at 1,000 managed
records. Repeating launch returns the existing attempt. A failed/ambiguous start
remains visible, with no automatic duplicate spawn. Relaunch/retry handling and
owned end/focus/attach controls are #13. Runtime pages have at most 10 records;
all IPC responses fit the existing frame limit and may use smaller pages.

`running` describes an observed live terminal process, `exited` an exited pane,
`unavailable` a missing/stale server and `uncertain` an incomplete observation.
Every run reports `limited_visibility: true`: these are not Hermes working,
waiting, approval or task-completion claims. No terminal text is captured.

Until #13 supplies verified terminal controls, a user can explicitly attach in
an existing terminal with `/usr/bin/tmux -S SOCKET_PATH attach-session -t =managed`,
where `SOCKET_PATH` is the saved runtime root plus
`/harbormaster/runners/RUN_UUID/tmux.sock`. Detaching closes only that client.
An explicit `systemctl --user stop UNIT_NAME` ends the named runtime and its
associated pane scope. Use only the exact unit returned by the run listing;
there is no wildcard cleanup or discovered-session control. Finished runtimes
remain available for inspection until explicitly stopped. Private launch files
are consumed once; retained attempts/directories are bounded and never removed
by speculative startup cleanup. No default tmux server/config is used.

## Disposable service demonstration

After building, this explicit opt-in uses temporary user services and a synthetic
PTY heartbeat worker. It demonstrates IPC registration, actual independent
runtime/pane cgroups, manager-service restart survival, repeat-launch reuse,
explicit runtime stop and cleanup. It opens no desktop window or real harness.

```sh
python3 -B scripts/demo-runtime.py --binary /absolute/new-build-directory/harbormaster \
  --allow-user-services --output /absolute/new-demo-results
```

The script checks default tmux/config stat metadata without reading transcripts
or configuration content. Its manager service has a 180-second cap and the synthetic worker a 90-second
cap; these are demonstration bounds, not a product job lifetime. Raw results remain outside source control.
Canonical Docker CI and separately mandatory native qualification are unchanged.
