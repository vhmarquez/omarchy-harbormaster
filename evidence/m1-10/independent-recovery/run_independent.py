"""Independent execution: mutations only in an ephemeral reviewed-source snapshot."""
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path('/home/vhm/Work/harbormaster-m1-10-recovery')
evidence = Path('/home/vhm/Work/harbormaster-m1-evidence/m1-10-independent-recovery')
tools = Path('/home/vhm/Work/omarchy-harbormaster/.tools-m1-9')
expected = '0e7589c318d914a1724251fb449d420273e8d977'
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
assert head == expected
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=root, text=True).strip()
sys.path.insert(0, str(root / 'scripts'))
from verification import sandbox

receipt = {
    'observed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'reviewed_commit': head,
    'method': 'Unchanged repository bwrap helper; offline prepared tools; ephemeral source copy and private state; no production edits.',
    'source_sha256': {}, 'fixture_sha256': {}, 'results': [],
}
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

with tempfile.TemporaryDirectory(prefix='hb-independent-recovery-', dir=evidence.parent) as temporary:
    directory = Path(temporary)
    source = directory / 'source'
    state = directory / 'state'
    state.mkdir(mode=0o700)
    sandbox.snapshot(root, source)
    for path in sorted(source.rglob('*')):
        if path.is_file():
            receipt['source_sha256'][str(path.relative_to(source))] = digest(path)

    def run(name, command, expected_exit=0):
        result = sandbox.run(command, source, tools, state, 240)
        output = result.pop('output')
        log = name + '.txt'
        (evidence / log).write_text(output)
        result.update(name=name, command=command, expected_exit=expected_exit, log=log)
        receipt['results'].append(result)
        (evidence / 'independent-execution.json').write_text(json.dumps(receipt, indent=2) + '\n')
        print(json.dumps(result), flush=True)
        assert result['exit_code'] == expected_exit and not result['timed_out'] and not result['output_limited'], output[-5000:]

    cargo = ['/tools/rust/bin/cargo']
    common = ['--locked', '--offline', '--workspace', '--all-features']
    unit = cargo + ['test'] + common + ['--lib']
    run('preflight-final', ['/usr/bin/python3', '-B', 'scripts/verification/preflight.py', '--stage-cargo'])
    run('sqlite-final', ['/usr/bin/python3', '-B', 'scripts/verification/sqlite.py'])
    run('format-final', cargo + ['fmt', '--all', '--', '--check'])
    run('clippy-final', cargo + ['clippy'] + common + ['--all-targets', '--', '-D', 'warnings'])
    for name, destination in [
        ('retained_log_fixture.rs', 'crates/harbormaster/tests/review_recovery.rs'),
        ('independent_review_fixture.rs', 'crates/harbormaster/src/recovery/tests/independent_review.rs'),
    ]:
        shutil.copyfile(evidence / name, source / destination)
        receipt['fixture_sha256'][name] = digest(evidence / name)
    tests = source / 'crates/harbormaster/src/recovery/tests/mod.rs'
    tests.write_text(tests.read_text() + '\nmod independent_review;\n')
    run('recovery-unit-final', unit + ['recovery::tests', '--', '--nocapture'])
    run('recovery-public-final', cargo + ['test'] + common + ['--test', 'recovery', '--test', 'review_recovery', '--', '--nocapture'])

    def mutate(name, relative, old, new, test, expected_exit=101, extra=None):
        path = source / relative
        original = path.read_text()
        assert original.count(old) == 1, (name, original.count(old))
        path.write_text(original.replace(old, new))
        receipt.setdefault('mutations', []).append({'name': name, 'file': relative, 'before': old, 'after': new, 'mutated_sha256': digest(path)})
        try:
            run(name, unit + [test, '--', '--exact', '--nocapture'] + (extra or []), expected_exit)
        finally:
            path.write_text(original)

    prefix = 'crates/harbormaster/src/recovery/'
    mutate('required-source-guard-negative', prefix + 'provenance.rs',
        'if mapping.turn != Source::Metadata {', 'if false {',
        'recovery::tests::privacy::mis_mapped_allowed_turn_and_version_strings_reject_before_temp')
    mutate('replay-source-guard-negative', prefix + 'spool.rs',
        '        provenance::validate_replay(&event, mapping)?;',
        '        let _ = mapping; // Independent mutation: omit replay provenance guard.',
        'recovery::tests::independent_review::replay_revalidates_required_source_provenance')
    mutate('filename-sequence-guard-negative', prefix + 'names.rs',
        'self.same_event_identity(event) && self.sequence == event.seq',
        'self.same_event_identity(event)',
        'recovery::tests::independent_review::filename_sequence_must_match_record_sequence')

    cleanup = source / (prefix + 'cleanup.rs')
    original = cleanup.read_text()
    old = 'if let Err(error) = rustix::fs::fsync(&self.directory.fd) {'
    new = 'if let Err(error) = Err::<(), rustix::io::Errno>(rustix::io::Errno::IO) {'
    assert original.count(old) == 1
    injected = original.replace(old, new)
    cleanup.write_text(injected)
    receipt['directory_sync_fault'] = {'method': 'Explicit source-level fault injection after successful unlink; not a real kernel fsync failure or production hook.', 'file': prefix + 'cleanup.rs', 'before': old, 'after': new, 'sha256': digest(cleanup)}
    test = 'recovery::tests::independent_review::successful_unlink_is_counted_when_directory_sync_fails'
    run('post-unlink-sync-fault-progress-green', unit + [test, '--', '--exact', '--ignored', '--nocapture'])
    old_block = '''            result.removed_files += 1;
            result.removed_bytes += held.entry.bytes();
            if held.entry.spool.is_some() {
                self.discarded_artifacts = self.discarded_artifacts.saturating_add(1);
                self.unknown_gap = true;
            }
'''
    assert injected.count(old_block) == 1
    broken = injected.replace(old_block, '')
    anchor = '''                result.failure = Some(error.into());
                self.unknown_gap = true;
                break;
            }
'''
    assert broken.count(anchor) == 1
    broken = broken.replace(anchor, anchor + old_block)
    cleanup.write_text(broken)
    receipt['directory_sync_progress_mutation'] = {'method': 'Move actual unlink counters after directory sync, reproducing prior lost-progress behavior under the identical injected sync failure.', 'sha256': digest(cleanup)}
    run('post-unlink-sync-progress-negative', unit + [test, '--', '--exact', '--ignored', '--nocapture'], 101)
    cleanup.write_text(original)
    run('recovery-unit-restored', unit + ['recovery::tests', '--', '--nocapture'])
    run('recovery-public-restored', cargo + ['test'] + common + ['--test', 'recovery', '--', '--nocapture'])
    receipt['production_sources_restored'] = all(digest(source / path) == sha for path, sha in receipt['source_sha256'].items() if path != 'crates/harbormaster/src/recovery/tests/mod.rs')
    assert receipt['production_sources_restored']
    receipt['verdict'] = 'PASS: corrected source and unchanged independent regression; all four deliberate guard/progress regressions failed at runtime as expected.'
    (evidence / 'independent-execution.json').write_text(json.dumps(receipt, indent=2) + '\n')
