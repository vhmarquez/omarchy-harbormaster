# Harbormaster project and task CLI

The #11 candidate turns the existing foundation into usable project, preset and
task commands. It saves manager-owned metadata in SQLite and previews a fixed
Hermes launch command. Runtime launch and the native interface follow in #12/M3.

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
never prompts or arguments. The command executes no harness. The in-memory
`PreparedLaunch::command` builds an unstarted argv-based process with explicit
HOME, fixed system PATH and bounded terminal/locale settings. It does not copy
API keys, loader overrides, PYTHONPATH or HERMES_HOME from the parent. Native
Hermes retains its own profile-based authentication; real runtime environment
integration remains #12. Launch preparation is not a race-free spawn authority.

State lives in `$XDG_STATE_HOME/harbormaster`, falling back to
`$HOME/.local/state/harbormaster`. Missing state parents are created privately.
Existing XDG parents may be 0755; the manager directory stays 0700 and its files
0600. Symlinked or other-user-writable paths are refused, without changing their
permissions. Only one process owns the database. These pre-daemon CLI commands
open the existing worker; #12 must route clients through the running manager.

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
