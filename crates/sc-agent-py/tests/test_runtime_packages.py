"""The runtime panic rule covers every production workspace package."""
from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[3]


class RuntimePackagesTest(unittest.TestCase):
    def test_lint_packages_match_workspace(self):
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
        members = workspace["workspace"]["members"]
        names = {
            tomllib.loads((ROOT / member / "Cargo.toml").read_text())["package"]["name"]
            for member in members
        }
        # The test-only package deliberately contains helpers that panic.
        names.remove("sc-agent-testkit")
        lint = tomllib.loads((ROOT / "sc-lint.toml").read_text())
        packages = lint["tool"]["sc-lint"]["analyzers"]["runtime"]["panic_rule_packages"]
        self.assertEqual(len(packages), len(set(packages)), "duplicate lint package")
        self.assertEqual(set(packages), names)


if __name__ == "__main__":
    unittest.main()
