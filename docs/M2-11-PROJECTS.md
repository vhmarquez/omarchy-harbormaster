# M2 #11: usable project and task registration

The owner authorized continued implementation after M1 and the revised GitHub
plan. This issue provides CLI registration/listing and a launch preview. The
manager/runtime in #12 remains dependent on this PR's approved merge.

## Responsibilities recorded before implementation

- `projects`: bounded project/task/preset records, filesystem identity checks,
  and a fixed Hermes launch plan. Explicit task labels never become prompts.
  No project configuration, Git helpers or harness profiles are loaded.
- `storage/catalog`: fixed queries and transactions on the existing bounded
  SQLite worker. Version 4 adds project, preset and task tables; recognized
  older schemas retain their existing migration/backup guarantees.
- `cli`: bounded argument parsing, the user's state directory, worker receipts
  and JSON output. The executable routes operational commands here; help and
  invalid arguments retain their side-effect-free behavior.

Dependency direction: CLI -> project records/planning + storage worker;
storage -> project record types; project validation/planning -> protocol IDs
and OS primitives. Pure lifecycle reducers remain independent. No dependency,
daemon, live adapter, desktop installation or worktree operation is added.

The intended commands are `project add/list`, `preset add/list`, `task add/list`
and `task plan`. A preset explicitly chooses an absolute Hermes executable and
a profile; the sole supported argv shape is `-p PROFILE chat`. This follows
[Hermes profile documentation](https://hermes-agent.nousresearch.com/docs/user-guide/profiles/).
No arbitrary options, prompt transport or shell command string is accepted.
Executable selection is explicit user trust, not binary/version attestation.
Launch preparation rechecks saved directory/executable identities; #12 must
handle the actual process-launch boundary and runtime ownership.

State lives in `$XDG_STATE_HOME/harbormaster` or `$HOME/.local/state/harbormaster`.
The XDG parent may have ordinary 0755 permissions; it must remain owned by the
user and not writable by other users. The manager directory remains 0700,
database/lock files 0600, with nofollow traversal and the existing owner lock.
This makes the existing worker usable with standard XDG directories without
changing Docker/native confinement or exposing manager state.

Projects, presets and tasks have fixed caps (1,000 / 100 / 10,000), bounded
fields and 100-item pages. Capacity rejects writes; it never removes user work.
Create retries are idempotent for the same saved identity/name and contents;
conflicting registrations require an explicit later edit operation. Registry
records are user-entered metadata, separate from the minimal event channel.

## Verification scope

Use focused CLI persistence/argument/environment and migration checks, one
working command-flow demonstration and one independent review. Keep logs outside
the source diff. Existing Docker CI and separate native qualification remain
mandatory, with actual paired evidence bound to the final selected source bytes.
Do not run additional mutation campaigns or repeat unchanged full suites.
