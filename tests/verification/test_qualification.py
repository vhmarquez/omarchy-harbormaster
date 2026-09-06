"""Evidence parser tests use synthetic records, not claimed CI execution."""
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class RecordTests(unittest.TestCase):
    def test_duplicate_json_keys_are_rejected(self):
        self.assertTrue((SCRIPTS / "verification/qualification.py").is_file(),
                        "paired qualification module missing")
        from verification.qualification import read_json
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "report.json"
            path.write_text('{"passed":true,"passed":false}')
            with self.assertRaisesRegex(ValueError, "duplicate"):
                read_json(path)
    def test_only_bounded_regular_records_are_read(self):
        from verification.qualification import read_json
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            valid = root / "valid.json"
            valid.write_text('{"ok":true}')
            link = root / "link.json"
            link.symlink_to(valid)
            oversized = root / "oversized.json"
            oversized.write_text('"' + 'x' * (2 * 1024 * 1024) + '"')
            for path in (link, oversized):
                with self.subTest(path=path.name):
                    with self.assertRaises((OSError, ValueError)):
                        read_json(path)
            self.assertEqual(read_json(valid), {"ok": True})

    def test_fifo_is_rejected_without_blocking(self):
        import os
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            fifo = Path(tmp) / "report.json"
            os.mkfifo(fifo)
            code = (f"import sys;sys.path.insert(0,{str(SCRIPTS)!r});"
                    "from pathlib import Path;"
                    "from verification.qualification import read_json;"
                    f"read_json(Path({str(fifo)!r}))")
            try:
                result = subprocess.run([sys.executable, "-B", "-c", code],
                                        capture_output=True, timeout=1,
                                        env={"PATH": "/usr/bin"})
            except subprocess.TimeoutExpired:
                self.fail("evidence reader blocked on FIFO")
            self.assertNotEqual(result.returncode, 0)

    def test_deeply_nested_json_fails_cleanly(self):
        from verification.qualification import read_json
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "report.json"
            path.write_text("[" * 2000 + "0" + "]" * 2000)
            with self.assertRaises(ValueError):
                read_json(path)

    def test_nonfinite_json_numbers_are_not_valid_evidence(self):
        from verification.qualification import read_json
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "report.json"
            for value in ("NaN", "Infinity", "-Infinity", "1e10000"):
                with self.subTest(value=value):
                    path.write_text('{"metadata":' + value + '}')
                    with self.assertRaises(ValueError):
                        read_json(path)


class ScopeReports(unittest.TestCase):
    def test_portable_pass_does_not_qualify_native(self):
        from verification import qualification
        from verification.checks import plan
        self.assertTrue(hasattr(qualification, "summarize"), "scoped qualification reporting missing")
        checks = [{"name": n, "required": True, "status": "PASS"} for n, _, _ in plan(scope="portable")]
        self.assertEqual(qualification.summarize(checks, "portable"),
                         {"portable": "PASS", "native": "NOT_RUN"})

    def test_missing_duplicate_or_optional_native_cannot_qualify(self):
        from verification import qualification
        from verification.checks import plan
        self.assertTrue(hasattr(qualification, "summarize"), "scoped qualification reporting missing")
        checks = [{"name": n, "required": True, "status": "PASS"} for n, _, _ in plan(scope="native")]
        self.assertEqual(qualification.summarize(checks, "native"),
                         {"portable": "NOT_RUN", "native": "PASS"})
        for mutation in (checks[:-1], checks + [checks[-1]],
                         checks[:-1] + [dict(checks[-1], required=False)],
                         checks[:-1] + [dict(checks[-1], status="SKIP")]):
            with self.subTest(mutation=mutation):
                self.assertEqual(qualification.summarize(mutation, "native")["native"], "FAIL")


class PairedQualificationTests(unittest.TestCase):
    def setUp(self):
        from qualification_fixtures import SyntheticPair
        self.pair = SyntheticPair(self)

    def test_complete_synthetic_pair_is_consistent_with_exact_commit(self):
        from verification import qualification
        self.assertTrue(hasattr(qualification, "qualify"), "paired qualification API missing")
        result = self.pair.qualify()
        self.assertEqual(result["revision"], self.pair.revision)
        self.assertEqual(result["qualifications"], {"portable": "PASS", "native": "PASS"})
        self.assertIs(result["qualification_complete"], True)
        self.assertIs(result["passed"], True)

    def test_pair_requires_exact_source_maps_from_both_lanes(self):
        import json
        for scope in ("portable", "native"):
            path = self.pair.directories[scope] / "source-sha256.json"
            original = path.read_bytes()
            for bad in (None, [], {}, {"README.md": "0" * 64},
                        dict(self.pair.hashes, unexpected="0" * 64)):
                with self.subTest(scope=scope, bad=bad):
                    path.write_text(json.dumps(bad))
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
            path.write_bytes(original)

    def test_each_report_requires_explicit_successful_single_scope_schema(self):
        import copy
        for scope in ("portable", "native"):
            original = copy.deepcopy(self.pair.reports[scope])
            mutations = [None, [], 2, "report", {},
                         *[dict(original, **{key: value}) for key, values in {
                             "schema": (None, 1, 2.0, "2", True),
                             "scope": (None, "all", "native" if scope == "portable" else "portable"),
                             "backend": (None, "bwrap" if scope == "portable" else "docker"),
                             "passed": (None, False, 1, "true"),
                             "qualification_complete": (None, True, 0),
                             "qualifications": (None, {}, [], {"portable": "PASS", "native": "PASS"}),
                         }.items() for value in values],
                         *[{key: value for key, value in original.items() if key != missing}
                           for missing in original if missing != "checks"]]
            for bad in mutations:
                with self.subTest(scope=scope, bad=bad):
                    self.pair.reports[scope] = bad
                    self.pair.write_reports()
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
            self.pair.reports[scope] = original
            self.pair.write_reports()

    def test_scoped_inventory_rejects_missing_duplicate_unknown_or_optional_required_rows(self):
        import copy
        for scope in ("portable", "native"):
            report = self.pair.reports[scope]
            original = copy.deepcopy(report["checks"])
            mutations = [None, {}, "checks", [], [None], [0], [[]], [{}],
                         original[1:], original + [original[0]],
                         original + [dict(original[0], required=False)],
                         original + [dict(original[-1])],
                         original + [dict(original[0], name="unknown")],
                         original + [dict(original[0], name="unknown", required=False)],
                         [dict(original[0], required=False)] + original[1:],
                         [dict(original[0], name=[])] + original[1:],
                         [dict(original[0], name="../outside")] + original[1:],
                         original[1:2] + original[:1] + original[2:],
                         [dict(row, required=True) if not row["required"] else row for row in original]]
            for index, bad in enumerate(mutations):
                with self.subTest(scope=scope, mutation=index):
                    report["checks"] = bad
                    self.pair.write_reports()
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
            del report["checks"]
            self.pair.write_reports()
            with self.assertRaises((OSError, ValueError)):
                self.pair.qualify()
            report["checks"] = original
            self.pair.write_reports()

    def test_required_execution_metadata_must_be_explicit_success(self):
        import copy
        mutations = {
            "required": (False, None, 1, "true"),
            "status": (None, "FAIL", "SKIP", "NOT_RUN", True),
            "exit_code": (None, False, True, 0.0, "0", 1, -1),
            "timed_out": (None, True, 0, "false"),
            "output_limited": (None, True, 0, "false"),
            "elapsed_seconds": (None, False, -1, "0", float("inf"), float("nan")),
        }
        for scope in ("portable", "native"):
            rows = self.pair.reports[scope]["checks"]
            original = copy.deepcopy(rows[0])
            for field, values in mutations.items():
                for value in values:
                    with self.subTest(scope=scope, field=field, value=value):
                        rows[0] = dict(original, **{field: value})
                        self.pair.write_reports()
                        with self.assertRaises((OSError, ValueError)):
                            self.pair.qualify()
                with self.subTest(scope=scope, missing=field):
                    rows[0] = {key: value for key, value in original.items() if key != field}
                    self.pair.write_reports()
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
            rows[0] = original
            self.pair.write_reports()

    def test_each_required_command_matches_current_scoped_plan(self):
        import copy
        for scope in ("portable", "native"):
            rows = self.pair.reports[scope]["checks"]
            for index, row in enumerate(copy.deepcopy(rows)):
                if not row["required"]:
                    continue
                argv = row["command"]
                mutations = (None, {}, " ".join(argv), [], argv[:-1], argv + ["extra"],
                             ["/untrusted/python"] + argv[1:])
                if row["name"] == "isolation-probe":
                    mutations += tuple(argv[:-1] + [value] for value in
                                       (None, 1, [], "relative", "", "\x00/invalid"))
                for mutation in mutations:
                    with self.subTest(scope=scope, name=row["name"], command=mutation):
                        rows[index] = dict(row, command=mutation)
                        self.pair.write_reports()
                        with self.assertRaises((OSError, ValueError)):
                            self.pair.qualify()
                with self.subTest(scope=scope, name=row["name"], missing="command"):
                    rows[index] = {key: value for key, value in row.items() if key != "command"}
                    self.pair.write_reports()
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
                rows[index] = row
                self.pair.write_reports()

    def test_required_logs_use_only_plan_filenames(self):
        for scope in ("portable", "native"):
            row = self.pair.reports[scope]["checks"][0]
            original = dict(row)
            for value in (None, [], 1, "../outside.txt", "/tmp/outside.txt", "other.txt"):
                with self.subTest(scope=scope, log=value):
                    row["log"] = value
                    self.pair.write_reports()
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
            del row["log"]
            self.pair.write_reports()
            with self.assertRaises((OSError, ValueError)):
                self.pair.qualify()
            row.update(original)
            self.pair.write_reports()

    def test_actual_logs_are_revalidated_instead_of_trusting_pass_metadata(self):
        from verification.checks import plan
        for scope in ("portable", "native"):
            for name, _, kind in plan(scope=scope):
                path = self.pair.directories[scope] / (name + ".txt")
                original = path.read_bytes()
                with self.subTest(scope=scope, name=name, missing=True):
                    path.unlink()
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
                if kind != "exit":
                    with self.subTest(scope=scope, name=name, content="invalid"):
                        path.write_text("synthetic failure with no executed tests")
                        with self.assertRaises((OSError, ValueError)):
                            self.pair.qualify()
                with self.subTest(scope=scope, name=name, content="non-utf8"):
                    path.write_bytes(b"\xff" + original)
                    with self.assertRaises((OSError, ValueError)):
                        self.pair.qualify()
                path.write_bytes(original)

    def test_evidence_directories_reject_symlinks_including_ancestors(self):
        from verification.qualification import qualify
        for scope in ("portable", "native"):
            directory = self.pair.directories[scope]
            for ancestor in (False, True):
                alias = self.pair.root / (scope + ("-parent" if ancestor else "-link"))
                alias.symlink_to(directory.parent if ancestor else directory, target_is_directory=True)
                values = dict(self.pair.directories)
                values[scope] = alias / scope if ancestor else alias
                with self.subTest(scope=scope, ancestor=ancestor):
                    with self.assertRaises((OSError, ValueError)):
                        qualify(self.pair.root, values["portable"], values["native"], self.pair.revision)

    def test_api_accepts_path_strings_but_rejects_malformed_path_arguments(self):
        from verification.qualification import qualify
        args = [self.pair.root, self.pair.directories["portable"], self.pair.directories["native"]]
        self.assertIs(qualify(*map(str, args), self.pair.revision)["passed"], True)
        for index in range(len(args)):
            for value in (None, [], {}, 1):
                values = list(args)
                values[index] = value
                with self.subTest(index=index, value=value):
                    with self.assertRaises((OSError, ValueError)):
                        qualify(*values, self.pair.revision)

    def test_required_file_reads_reject_symlinks_directories_fifo_and_oversize(self):
        import os
        import subprocess
        for scope in ("portable", "native"):
            for filename, limit in (("report.json", 2 * 1024 * 1024),
                                    ("source-sha256.json", 2 * 1024 * 1024),
                                    ("isolation-probe.txt", 4 * 1024 * 1024)):
                path = self.pair.directories[scope] / filename
                original = path.read_bytes()
                backup = path.with_suffix(".backup")
                backup.write_bytes(original)
                for kind in ("missing", "symlink", "directory", "oversize", "fifo"):
                    with self.subTest(scope=scope, filename=filename, kind=kind):
                        path.unlink()
                        if kind == "symlink":
                            path.symlink_to(backup)
                        elif kind == "directory":
                            path.mkdir()
                        elif kind == "oversize":
                            path.write_bytes(b" " * (limit + 1))
                        elif kind == "fifo":
                            os.mkfifo(path)
                        if kind == "fifo":
                            code = (f"import sys;sys.path.insert(0,{str(SCRIPTS)!r});"
                                    "from verification.qualification import qualify;"
                                    f"qualify({str(self.pair.root)!r},"
                                    f"{str(self.pair.directories['portable'])!r},"
                                    f"{str(self.pair.directories['native'])!r},"
                                    f"{self.pair.revision!r})")
                            result = subprocess.run([sys.executable, "-B", "-c", code],
                                                    capture_output=True, timeout=2,
                                                    env={"PATH": "/usr/bin", "LANG": "C.UTF-8"})
                            self.assertNotEqual(result.returncode, 0)
                            self.assertIn(b"ValueError", result.stderr)
                        else:
                            with self.assertRaises((OSError, ValueError)):
                                self.pair.qualify()
                        if kind == "directory":
                            path.rmdir()
                        elif kind != "missing":
                            path.unlink()
                        path.write_bytes(original)

    def test_json_files_reject_duplicate_keys_and_invalid_encoding(self):
        for scope in ("portable", "native"):
            for filename in ("report.json", "source-sha256.json"):
                path = self.pair.directories[scope] / filename
                original = path.read_bytes()
                for data in (b"{", b"\xff", b'{"same":true,"same":false}',
                             b'{"nested":{"same":1,"same":2}}'):
                    with self.subTest(scope=scope, filename=filename, data=data):
                        path.write_bytes(data)
                        with self.assertRaises((OSError, ValueError)):
                            self.pair.qualify()
                path.write_bytes(original)

    def test_pair_binds_literal_commit_and_rejects_changed_source(self):
        import subprocess
        from verification.qualification import qualify
        pair = self.pair
        for revision in (None, [], 1, "main", pair.revision[:8], pair.revision.upper(),
                         " " + pair.revision, "0" * 40, pair.git("rev-parse", "HEAD^{tree}").strip()):
            with self.subTest(revision=revision):
                with self.assertRaises((OSError, ValueError, subprocess.SubprocessError)):
                    qualify(pair.root, pair.directories["portable"], pair.directories["native"], revision)
        (pair.root / "README.md").write_text("changed checkout source")
        with self.assertRaises(ValueError):
            pair.qualify()
        (pair.root / "README.md").unlink()
        with self.assertRaises(ValueError):
            pair.qualify()

    def test_canary_path_is_syntactic_and_never_read(self):
        import os
        canary = self.pair.root / "evidence" / "unread-canary"
        os.mkfifo(canary)
        for report in self.pair.reports.values():
            report["checks"][0]["command"][-1] = str(canary)
        self.pair.write_reports()
        self.assertIs(self.pair.qualify()["passed"], True)

    def test_optional_rows_cannot_change_scoped_qualification(self):
        for report in self.pair.reports.values():
            report["checks"] = [row for row in report["checks"] if row["required"]]
            for row in report["checks"]:
                row["elapsed_seconds"] = 0
        self.pair.write_reports()
        self.assertIs(self.pair.qualify()["passed"], True)

    def test_exact_file_size_limits_are_allowed(self):
        for directory in self.pair.directories.values():
            for filename, limit in (("report.json", 2 * 1024 * 1024),
                                    ("source-sha256.json", 2 * 1024 * 1024),
                                    ("isolation-probe.txt", 4 * 1024 * 1024)):
                path = directory / filename
                content = path.read_bytes()
                path.write_bytes(content + b" " * (limit - len(content)))
        self.assertIs(self.pair.qualify()["passed"], True)

    def test_declared_report_revision_cannot_contradict_requested_commit(self):
        for scope in ("portable", "native"):
            report = self.pair.reports[scope]
            for value in (None, False, [], "main", self.pair.revision.upper(), "0" * 40):
                with self.subTest(scope=scope, revision=value):
                    report["revision"] = value
                    self.pair.write_reports()
                    with self.assertRaises(ValueError):
                        self.pair.qualify()
            report["revision"] = self.pair.revision
            self.pair.write_reports()
        self.assertIs(self.pair.qualify()["passed"], True)

    def test_deep_json_reports_fail_cleanly_at_pair_boundary(self):
        for scope in ("portable", "native"):
            for filename in ("report.json", "source-sha256.json"):
                path = self.pair.directories[scope] / filename
                original = path.read_bytes()
                path.write_text("[" * 2000 + "0" + "]" * 2000)
                with self.subTest(scope=scope, filename=filename):
                    with self.assertRaises(ValueError):
                        self.pair.qualify()
                path.write_bytes(original)

    def test_every_required_row_is_mandatory_in_its_scope(self):
        for scope in ("portable", "native"):
            report = self.pair.reports[scope]
            original = report["checks"]
            for index, row in enumerate(original):
                if not row["required"]:
                    continue
                mutations = (original[:index] + original[index + 1:],
                             original + [dict(row, required=False)],
                             original[:index] + [dict(row, required=False)] + original[index + 1:],
                             original[:index] + [dict(row, status="FAIL")] + original[index + 1:])
                for mutation_index, rows in enumerate(mutations):
                    with self.subTest(scope=scope, name=row["name"], mutation=mutation_index):
                        report["checks"] = rows
                        self.pair.write_reports()
                        with self.assertRaises(ValueError):
                            self.pair.qualify()
            report["checks"] = original
            self.pair.write_reports()

    def test_optional_rows_still_require_well_formed_status(self):
        for scope in ("portable", "native"):
            row = self.pair.reports[scope]["checks"][-1]
            for value in (None, [], {}, True, "UNKNOWN"):
                with self.subTest(scope=scope, status=value):
                    row["status"] = value
                    self.pair.write_reports()
                    with self.assertRaises(ValueError):
                        self.pair.qualify()
            del row["status"]
            self.pair.write_reports()
            with self.assertRaises(ValueError):
                self.pair.qualify()
            row["status"] = "SKIP"
            self.pair.write_reports()

    def test_metric_log_recursion_is_a_clean_evidence_failure(self):
        path = self.pair.directories["portable"] / "maintainability.txt"
        path.write_text('{"coverage":{"complete":true,"files_analyzed":1},"extra":'
                        + "[" * 100000 + "0" + "]" * 100000 + "}")
        with self.assertRaises(ValueError):
            self.pair.qualify()


if __name__ == "__main__":
    unittest.main()
