"""Controlled release-script planning and wheel metadata checks; no tags or uploads are made."""
import importlib.util
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("prerelease_tag", ROOT / "scripts/prerelease_tag.py")
tag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tag)


class ReleaseContract(unittest.TestCase):
    def test_version_selection_never_reuses_stable_or_prerelease(self):
        self.assertEqual(tag.next_version("0.1.0", set()), "0.1.0")
        self.assertEqual(tag.next_version("0.1.0", {"v0.1.0", "prerelease/v0.1.1"}), "0.1.2")
        self.assertEqual(tag.next_version("0.1.9", {"0.1.9", "prerelease/v0.1.10"}), "0.1.11")
        with self.assertRaises(ValueError):
            tag.next_version("0.1.0-beta", set())

    def test_both_wheel_metadata_and_ci_use_python_311_floor(self):
        for package in ["sc-agent-py", "sc-treesitter-py"]:
            data = tomllib.loads((ROOT / "crates" / package / "pyproject.toml").read_text())
            self.assertEqual(data["project"]["requires-python"], ">=3.11")
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertEqual(ci.count('python-version: "3.11"'), 2)
        self.assertNotIn('python-version: "3.12"', ci)


if __name__ == "__main__":
    unittest.main()
