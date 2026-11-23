"""Keep checked-in CLI metadata synchronized with the Cargo workspace release."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class VersionSync(unittest.TestCase):
    def test_checked_in_cli_versions_match_workspace(self):
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        checks = {
            "Cargo.toml": [r'sc-agent\s*=.*?version\s*=\s*"([^"]+)"',
                           r'sc-treesitter\s*=.*?version\s*=\s*"([^"]+)"'],
            # The release kit (sync-python-version) rewrites a literal
            # [project].version, so the wheel version stays a literal, not dynamic.
            "crates/sc-agent-py/pyproject.toml": [r'^version\s*=\s*"([^"]+)"'],
            "fixtures/cli/decidr-cli-model.json": [r'"version"\s*:\s*"([^"]+)"'],
            "fixtures/cli/sitr-cli-model.json": [r'"version"\s*:\s*"([^"]+)"'],
            "site/cli/decidr/index.html": [r'Version ([0-9][^< ]*)', r'"version":"([^"]+)"'],
            "site/cli/sitr/index.html": [r'Version ([0-9][^< ]*)', r'"version":"([^"]+)"'],
        }
        mismatches = []
        for relative, patterns in checks.items():
            path = ROOT / relative
            for pattern in patterns:
                matches = [(number, match.group(1))
                           for number, line in enumerate(path.read_text().splitlines(), 1)
                           if (match := re.search(pattern, line))]
                self.assertTrue(matches, f"{relative}: no version field matched {pattern}")
                mismatches.extend(f"{relative}:{number}: {actual!r} != workspace {version!r}"
                                  for number, actual in matches if actual != version)
        self.assertFalse(mismatches, "Version literals are out of sync:\n" + "\n".join(mismatches))
