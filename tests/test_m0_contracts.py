"""Tests for the M0 documentation/fixture checker, not the future daemon."""
from pathlib import Path
import importlib.util
import unittest

ROOT = Path(__file__).resolve().parents[1]
PATH = ROOT / 'scripts' / 'verify-m0.py'


def load_checker():
    if not PATH.exists():
        return None
    spec = importlib.util.spec_from_file_location('m0_checker', PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FixtureContractTests(unittest.TestCase):
    def assert_fixture_error(self, data, expected):
        checker = load_checker()
        assert checker is not None
        try:
            errors = checker.validate_fixtures(data)
        except (AttributeError, TypeError) as error:
            self.fail(f'validation must return errors, not raise {type(error).__name__}')
        self.assertIn(expected, errors)

    def test_fixture_root_requires_an_object(self):
        for data in [None, [], 'fixtures', 1, True]:
            with self.subTest(data=data):
                self.assert_fixture_error(data, 'fixtures must be an object')

    def test_scenarios_requires_a_list(self):
        data = {'schema_version': 1, 'evidence_kind': 'synthetic-specification'}
        with self.subTest(scenarios='missing'):
            self.assert_fixture_error(data, 'scenarios must be a list')
        for scenarios in [None, {}, '', 'scenarios', 1, True]:
            with self.subTest(scenarios=scenarios):
                data['scenarios'] = scenarios
                self.assert_fixture_error(data, 'scenarios must be a list')
        self.assert_fixture_error({'scenarios': None}, 'schema_version must be integer 1')
        self.assert_fixture_error({'scenarios': None}, 'fixtures must be labeled synthetic-specification')

    def test_scenario_records_require_objects(self):
        for item in [None, [], 'scenario', 1, True]:
            with self.subTest(item=item):
                data = {'schema_version': 1, 'evidence_kind': 'synthetic-specification',
                        'scenarios': [item, {'id': 'incomplete'}]}
                self.assert_fixture_error(data, 'scenario at index 0 must be an object')
                self.assert_fixture_error(data, 'incomplete scenario: incomplete')
                self.assert_fixture_error(data, 'missing required scenarios')

    def test_scenario_ids_require_nonempty_strings_before_set_checks(self):
        import json
        data = json.loads((ROOT / 'contracts/failure-fixtures.json').read_text())
        for scenario_id in [None, [], {}, 1, True, '', ' \t\n']:
            with self.subTest(scenario_id=scenario_id):
                data['scenarios'][0]['id'] = scenario_id
                self.assert_fixture_error(data, 'scenario id at index 0 must be a nonempty string')
                self.assert_fixture_error(data, 'missing required scenarios')
        del data['scenarios'][0]['id']
        self.assert_fixture_error(data, 'scenario id at index 0 must be a nonempty string')

    def test_each_of_the_fourteen_specified_scenarios_is_required(self):
        import json
        required_ids = {
            'crash-leftovers', 'diagnostic-policy', 'active-self-events',
            'late-start-after-end', 'prune-outbox', 'zombie-process',
            'reasoning-versus-verbosity', 'invalid-frame', 'launch-retry',
            'notification-uncertain', 'wrong-target', 'slow-consumer',
            'hook-install-rollback', 'review-is-not-open',
        }
        data = json.loads((ROOT / 'contracts/failure-fixtures.json').read_text())
        self.assertLessEqual(required_ids, {item['id'] for item in data['scenarios']})
        for scenario_id in sorted(required_ids):
            with self.subTest(removed=scenario_id):
                damaged = dict(data, scenarios=[item for item in data['scenarios']
                                               if item['id'] != scenario_id])
                self.assert_fixture_error(damaged, 'missing required scenarios')

    def assert_checker_failure(self, read_options, expected):
        from contextlib import redirect_stderr, redirect_stdout
        import io
        import json
        import runpy
        from unittest.mock import patch
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(Path, 'read_text', **read_options) as reader:
            with redirect_stdout(stdout), redirect_stderr(stderr):
                try:
                    with self.assertRaises(SystemExit) as exited:
                        runpy.run_path(str(PATH), run_name='__main__')
                except (OSError, ValueError, RecursionError) as error:
                    self.fail(f'checker must emit JSON, not raise {type(error).__name__}')
        self.assertEqual(exited.exception.code, 1)
        self.assertEqual(json.loads(stdout.getvalue()), {'passed': False, 'errors': [expected]})
        self.assertEqual(stderr.getvalue(), '')
        reader.assert_called_once_with(encoding='utf-8')

    def test_checker_reports_file_errors_as_json(self):
        for error in [FileNotFoundError('fixture missing'), PermissionError('fixture denied'),
                      IsADirectoryError('not a file'), OSError('read failed'),
                      UnicodeDecodeError('utf-8', b'\xff', 0, 1, 'invalid UTF-8')]:
            with self.subTest(error=type(error).__name__):
                self.assert_checker_failure({'side_effect': error}, 'cannot read fixtures')

    def test_checker_reports_json_parse_errors_as_json(self):
        import sys
        cases = {'empty': '', 'truncated': '{"scenarios":', 'trailing': '{} trailing'}
        digit_limit = sys.get_int_max_str_digits()
        if digit_limit:
            cases['excessive_integer'] = '9' * (digit_limit + 1)
        for label, text in cases.items():
            with self.subTest(input=label):
                self.assert_checker_failure({'return_value': text}, 'cannot parse fixtures JSON')

    def test_schema_version_requires_exact_integer_one(self):
        import json
        checker = load_checker()
        assert checker is not None
        data = json.loads((ROOT / 'contracts/failure-fixtures.json').read_text())
        for version in [None, True, False, 0, 999, 1.0, '1', [], {}]:
            with self.subTest(version=version):
                data['schema_version'] = version
                self.assertIn('schema_version must be integer 1', checker.validate_fixtures(data))
        del data['schema_version']
        self.assertIn('schema_version must be integer 1', checker.validate_fixtures(data))
        data['schema_version'] = 1
        self.assertEqual(checker.validate_fixtures(data), [])

    def test_required_failure_scenarios_cannot_be_missing(self):
        checker = load_checker()
        self.assertIsNotNone(checker, 'M0 checker is not implemented yet')
        assert checker is not None
        self.assertIn('missing required scenarios', checker.validate_fixtures({'schema_version': 1, 'evidence_kind': 'synthetic-specification', 'scenarios': []}))


    def test_fixture_records_are_explicit_unique_and_nonempty(self):
        import json
        import copy
        checker = load_checker()
        self.assertIsNotNone(checker)
        assert checker is not None
        data = json.loads((ROOT / 'contracts/failure-fixtures.json').read_text())
        self.assertEqual(checker.validate_fixtures(data), [])
        damaged = copy.deepcopy(data)
        damaged['evidence_kind'] = 'live-verified'
        damaged['scenarios'].append(copy.deepcopy(damaged['scenarios'][0]))
        damaged['scenarios'][0]['verification'] = ''
        errors = checker.validate_fixtures(damaged)
        self.assertIn('fixtures must be labeled synthetic-specification', errors)
        self.assertIn('duplicate scenario ids', errors)
        self.assertIn('incomplete scenario: crash-leftovers', errors)


if __name__ == '__main__':
    unittest.main()
