"""Offline, stdlib-only drift gates for the reviewed CI trust boundary.

verify.yml deliberately uses JSON (a YAML subset) to avoid an ambient YAML
parser or YAML 1.1's coercion of the GitHub Actions `on` key to a boolean.
These are configuration gates, not evidence that Docker or hosted CI ran.
"""

import hashlib
import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
CHECKOUT = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
UPLOAD = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate configuration key: {key}")
        result[key] = value
    return result


def load_json(path):
    return json.loads(path.read_text(), object_pairs_hook=unique_object)


class CIContractTests(unittest.TestCase):
    def test_workflow_uses_pinned_least_privilege_single_entrypoint(self):
        path = ROOT / ".github/workflows/verify.yml"
        self.assertTrue(path.is_file(), "required CI workflow is missing")
        workflow = load_json(path)
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        self.assertEqual(workflow["on"], {
            "pull_request": {}, "push": {"branches": ["main"]}
        })
        self.assertTrue(workflow["concurrency"]["cancel-in-progress"])
        self.assertEqual(set(workflow["jobs"]), {"verify"})
        job = workflow["jobs"]["verify"]
        self.assertEqual(job["runs-on"], "ubuntu-24.04")
        self.assertEqual(job["timeout-minutes"], 30)
        self.assertEqual(job["permissions"], {"contents": "read"})
        self.assertFalse(set(job) & {"container", "services", "environment", "continue-on-error", "if"})
        self.assertEqual(len(job["steps"]), 3, "required bounded report upload is missing")
        checkout, execution, upload = job["steps"]
        self.assertEqual(checkout["uses"], CHECKOUT)
        self.assertEqual(checkout["with"], {
            "persist-credentials": False, "fetch-depth": 1, "fetch-tags": False,
            "submodules": False, "lfs": False, "set-safe-directory": False,
            "allow-unsafe-pr-checkout": False,
        })
        self.assertEqual(execution["shell"], "bash")
        self.assertFalse(set(execution) & {"continue-on-error", "if", "env"})
        run = execution["run"]
        self.assertIn("set -euo pipefail", run)
        self.assertIn("env -i", run)
        self.assertIn('DOCKER_CONFIG="$bootstrap_home/docker"', run)
        self.assertIn("--file tools/ci.Dockerfile tools", run)
        self.assertIn("--iidfile", run)
        self.assertIn("^sha256:[0-9a-f]{64}$", run)
        self.assertIn("python3 scripts/prepare-tools.py", run)
        self.assertIn('python3 scripts/verify.py --backend docker --image "$image"', run)
        self.assertEqual(run.count("scripts/verify.py"), 1)
        self.assertIsNone(re.search(r"cargo (test|check|clippy|deny)|qmllint|qmltestrunner", run))
        for forbidden in ("sudo", "--privileged", "seccomp=unconfined", "--network=host",
                          "--secret", "--ssh", "docker.sock", "|| true", "pull_request_target",
                          "secrets.", "actions/cache", "self-hosted"):
            self.assertNotIn(forbidden, path.read_text())

    def test_report_upload_is_pinned_and_scoped_even_after_failure(self):
        workflow = load_json(ROOT / ".github/workflows/verify.yml")
        steps = workflow["jobs"]["verify"]["steps"]
        self.assertEqual(len(steps), 3, "required bounded report upload is missing")
        upload = steps[2]
        self.assertEqual(upload["uses"], UPLOAD)
        self.assertEqual(upload["if"], "always()")
        self.assertNotIn("continue-on-error", upload)
        self.assertEqual(upload["with"], {
            "name": "verification-reports", "path": ".verify/**",
            "retention-days": 7, "if-no-files-found": "warn",
            "include-hidden-files": True, "overwrite": False,
            "archive": True, "compression-level": 6,
        })

    def test_container_pins_public_snapshot_and_nonroot_runtime(self):
        lock_path = ROOT / "tools/platform-lock.json"
        docker_path = ROOT / "tools/ci.Dockerfile"
        self.assertTrue(lock_path.is_file(), "required platform lock is missing")
        self.assertTrue(docker_path.is_file(), "required CI image recipe is missing")
        lock = load_json(lock_path)
        docker = docker_path.read_text()
        self.assertEqual(lock["schema_version"], 1)
        self.assertEqual(lock["architecture"], "x86_64")
        self.assertEqual(lock["archive"]["date"], "2026/09/05")
        self.assertRegex(lock["container"]["base"], r"^docker.io/library/archlinux@sha256:[0-9a-f]{64}$")
        self.assertIn("FROM " + lock["container"]["base"], docker)
        self.assertIn("USER 65532:65532", docker)
        self.assertIn('ENTRYPOINT []', docker)
        self.assertIn("COPY platform-lock.json /opt/harbormaster-platform-lock.json", docker)
        self.assertEqual(re.findall(r"^COPY .+$", docker, re.M), [
            "COPY platform-lock.json /opt/harbormaster-platform-lock.json"
        ])
        for repo, record in lock["archive"]["databases"].items():
            self.assertRegex(record["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(record["url"], f"https://archive.archlinux.org/repos/2026/09/05/{repo}/os/x86_64/{repo}.db")
            self.assertIn(record["sha256"], docker)
        expected = {"qt6-base": "6.11.2-3", "qt6-declarative": "6.11.2-1",
                    "python": "3.14.7-1", "nodejs": "26.8.1-2"}
        for package, version in expected.items():
            self.assertEqual(lock["packages"][package]["version"], version)
            self.assertIn(package + " " + version, docker)
        for package, record in lock["packages"].items():
            self.assertRegex(record["sha256"], r"^[0-9a-f]{64}$")
            self.assertTrue(record["url"].startswith("https://archive.archlinux.org/"))
            self.assertTrue(record["licenses"])
            self.assertEqual(record["install"], "snapshot")
            self.assertNotIn("snapshot_version", record)
        self.assertIn("sha256sum --check --strict", docker)
        self.assertIn("SigLevel = Required DatabaseOptional", docker)
        self.assertIn("LocalFileSigLevel = Required", docker)
        self.assertIn("--proto '=https'", docker)
        self.assertIn("--max-time", docker)
        self.assertIn("pacman -Suu", docker)
        self.assertNotIn("pacman -U", docker)
        self.assertNotIn("archive.archlinux.org/packages/", docker)
        self.assertIn("test \"$(/usr/lib/qt6/bin/qmllint --version)\" = 'qmllint 6.11.2'", docker)
        self.assertIn("test \"$(/usr/lib/qt6/bin/qmlformat --version)\" = 'qmlformat 6.11.2'", docker)
        for forbidden in ("SigLevel = Never", "TrustAll", "--noconfirm -Sy", "--privileged",
                          "--mount=", "ADD ", "curl |", "sudo", "ENTRYPOINT [\"/bin/bash\"]"):
            self.assertNotIn(forbidden, docker)

    def test_bootstrap_requires_explicit_online_consent_and_private_root(self):
        workflow = load_json(ROOT / ".github/workflows/verify.yml")
        run = workflow["jobs"]["verify"]["steps"][1]["run"]
        self.assertIn("python3 scripts/prepare-tools.py --online --tools-root .tools", run)
        self.assertIn("--platform linux/amd64", run)

    def test_license_baseline_distinguishes_tools_and_preserves_assets(self):
        docs_path = ROOT / "docs/DEPENDENCIES.md"
        self.assertTrue(docs_path.is_file(), "required dependency/license review is missing")
        docs = docs_path.read_text()
        packages = load_json(ROOT / "tools/platform-lock.json")["packages"]
        qt = ["GPL-3.0-only", "LGPL-3.0-only", "LicenseRef-Qt-Commercial", "Qt-GPL-exception-1.0"]
        expected = {
            "archlinux-keyring": ["GPL-3.0-or-later"],
            "gcc": ["GPL-3.0-or-later WITH GCC-exception-3.1", "GFDL-1.3-or-later"],
            "git": ["GPL-2.0-only"], "python": ["PSF-2.0"],
            "nodejs": ["MIT"], "qt6-base": qt, "qt6-declarative": qt,
        }
        self.assertEqual({name: r["licenses"] for name, r in packages.items()}, expected)
        for name, record in packages.items():
            self.assertIn(name, docs)
            self.assertIn(record["version"], docs)
        for phrase in ("not a complete SBOM", "LGPL-3.0-only", "Qt-GPL-exception-1.0",
                       "Quickshell", "Omarchy", "OFL-1.1", "no separate license grant",
                       "Redistribution is not approved", "required", "cargo-deny"):
            self.assertIn(phrase, docs)
        frozen = ROOT / "docs/design/original/assets"
        for name, digest in {
            "OFL.txt": "30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad",
            "JetBrainsMono-Regular.ttf": "1767e08e6b207eb57c114e833b817e26c6a7625bbcf474e3b4dd0f73c10e5bdc",
        }.items():
            self.assertEqual(hashlib.sha256((frozen / name).read_bytes()).hexdigest(), digest)
        provenance = (ROOT / "docs/design/PROVENANCE.md").read_text()
        self.assertIn("no separate license grant", provenance)
        self.assertIn("font remains under OFL", provenance)

    def test_duplicate_configuration_keys_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate configuration key"):
            json.loads('{"permissions":{},"permissions":{"contents":"write"}}',
                       object_pairs_hook=unique_object)


if __name__ == "__main__":
    unittest.main()
