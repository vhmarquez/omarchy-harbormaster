"""Check M0 contract artifacts; this is not a Harbormaster implementation."""
from pathlib import Path
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {'crash-leftovers', 'diagnostic-policy', 'active-self-events',
            'late-start-after-end', 'prune-outbox', 'zombie-process',
            'reasoning-versus-verbosity', 'invalid-frame', 'launch-retry',
            'notification-uncertain', 'wrong-target', 'slow-consumer',
            'hook-install-rollback', 'review-is-not-open'}


def validate_fixtures(data):
    if not isinstance(data, dict):
        return ['fixtures must be an object']
    errors = []
    if type(data.get('schema_version')) is not int or data['schema_version'] != 1:
        errors.append('schema_version must be integer 1')
    if data.get('evidence_kind') != 'synthetic-specification':
        errors.append('fixtures must be labeled synthetic-specification')
    scenarios = data.get('scenarios')
    if not isinstance(scenarios, list):
        errors.append('scenarios must be a list')
        return errors
    ids = []
    for index, item in enumerate(scenarios):
        if not isinstance(item, dict):
            errors.append(f'scenario at index {index} must be an object')
            continue
        scenario_id = item.get('id')
        if not isinstance(scenario_id, str) or not scenario_id.strip():
            errors.append(f'scenario id at index {index} must be a nonempty string')
            continue
        ids.append(scenario_id)
        if any(not isinstance(item.get(key), str) or not item[key].strip()
               for key in ['id', 'setup', 'operation', 'expected', 'invariant', 'verification']):
            errors.append('incomplete scenario: ' + str(item.get('id')))
    if not REQUIRED <= set(ids):
        errors.append('missing required scenarios')
    if len(ids) != len(set(ids)):
        errors.append('duplicate scenario ids')
    return errors


if __name__ == '__main__':
    try:
        text = (ROOT / 'contracts/failure-fixtures.json').read_text(encoding='utf-8')
    except (OSError, UnicodeError):
        errors = ['cannot read fixtures']
    else:
        try:
            data = json.loads(text)
        except ValueError:
            errors = ['cannot parse fixtures JSON']
        else:
            errors = validate_fixtures(data)
    print(json.dumps({'passed': not errors, 'errors': errors}))
    sys.exit(bool(errors))
