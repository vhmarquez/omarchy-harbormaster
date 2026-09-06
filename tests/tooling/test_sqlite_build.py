"""Runtime/linkage identity and build lifecycle fail closed before acceptance."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from verification.sqlite import OPTIONS, build, validate_runtime


class SqliteBuild(unittest.TestCase):
    def setUp(self):
        self.pins = {"version": "3.53.4", "version_number": 3053004, "source_id": "expected source"}
        self.output = "version=3.53.4\nnumber=3053004\nsource=expected source\n" + "".join(
            "option=" + option + "\n" for option in OPTIONS)
        self.dynamic = "0x0000000000000001 (NEEDED) Shared library: [libc.so.6]\n"

    def test_exact_static_identity_passes(self):
        self.assertEqual(validate_runtime(self.output, self.dynamic, self.pins)["sqlite_linkage"], "static")

    def test_other_version_number_and_source_fail(self):
        for before, after in (("version=3.53.4", "version=3.53.2"), ("number=3053004", "number=3053002"),
                              ("source=expected source", "source=foreign source")):
            with self.subTest(field=before), self.assertRaisesRegex(ValueError, "identity"):
                validate_runtime(self.output.replace(before, after), self.dynamic, self.pins)

    def test_required_compile_option_loss_fails(self):
        for option in OPTIONS:
            with self.subTest(option=option), self.assertRaisesRegex(ValueError, "options"):
                validate_runtime(self.output.replace("option=" + option + "\n", ""), self.dynamic, self.pins)

    def test_dynamic_host_sqlite_and_unreadable_section_fail(self):
        for dynamic in ("", self.dynamic + "0x1 (NEEDED) Shared library: [libsqlite3.so.0]\n"):
            with self.subTest(dynamic=dynamic), self.assertRaisesRegex(ValueError, "linkage"):
                validate_runtime(self.output, dynamic, self.pins)

    def test_invalid_source_never_creates_or_compiles_output(self):
        with patch("verification.sqlite.verify_sqlite", side_effect=ValueError("bad source")), \
                patch("verification.sqlite.run") as run, patch.object(Path, "mkdir") as mkdir:
            with self.assertRaisesRegex(ValueError, "bad source"):
                build(Path("/fixture"), Path("/tools"), Path("/state/sqlite"))
            run.assert_not_called()
            mkdir.assert_not_called()


if __name__ == "__main__":
    unittest.main()
