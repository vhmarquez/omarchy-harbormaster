"""Unit-only safety fixtures, not a production runtime test suite."""
import importlib.util
import pathlib
import unittest


class IdentityTests(unittest.TestCase):
    def test_exact_identity_accepts_only_matching_window(self):
        path = pathlib.Path(__file__).with_name("identity.py")
        self.assertTrue(path.exists(), "identity helper not implemented")
        spec = importlib.util.spec_from_file_location("identity", path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        expected = dict(address="0x123", pid=123, app_id="org.harbormaster.probe.test",
                        start_ticks=42, boot_id="boot", compositor="instance")
        windows = [dict(address="0x999", pid=456, **{"class": "other"}),
                   dict(address="0x123", pid=123, **{"class": expected["app_id"]})]
        process = dict(pid=123, start_ticks=42, boot_id="boot", state="S")
        self.assertEqual(module.exact_window(expected, windows, process, "instance"), "0x123")


    def test_stale_or_ambiguous_identity_is_rejected(self):
        import identity
        expected = dict(address="0x123", pid=123, app_id="org.harbormaster.probe.test",
                        start_ticks=42, boot_id="boot", compositor="instance")
        window = dict(address="0x123", pid=123, **{"class": expected["app_id"]})
        process = dict(pid=123, start_ticks=42, boot_id="boot", state="S")
        for field, wrong in [("pid", 124), ("start_ticks", 43), ("boot_id", "old"),
                             ("state", "Z"), ("state", "X")]:
            with self.subTest(field=field, wrong=wrong):
                self.assertIsNone(identity.exact_window(expected, [window],
                                                       {**process, field: wrong}, "instance"))
        self.assertIsNone(identity.exact_window(expected, [window], None, "instance"))
        self.assertIsNone(identity.exact_window(expected, [window], process, "other"))
        self.assertIsNone(identity.exact_window(expected, [window, window], process, "instance"))
        for field, wrong in [("address", "0x456"), ("pid", 222), ("class", "wrong")]:
            with self.subTest(window_field=field):
                self.assertIsNone(identity.exact_window(expected, [{**window, field: wrong}],
                                                       process, "instance"))
        self.assertIsNone(identity.exact_window(expected, [], process, "instance"))
        malformed = "0x123\"}); evil() --"
        self.assertIsNone(identity.exact_window({**expected, "address": malformed},
                                               [{**window, "address": malformed}], process, "instance"))


if __name__ == "__main__":
    unittest.main()
