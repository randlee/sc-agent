"""Controlled release-script planning and wheel metadata checks; no tags or uploads are made."""
import importlib.util
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("prerelease_tag", ROOT / "scripts/prerelease_tag.py")
tag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tag)


class ReleaseContract(unittest.TestCase):
    def test_version_bump_regenerates_and_stages_release_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## 1.2.3 (unreleased)\n\n- Existing note.\n')
            (root / "release").mkdir()
            (root / "release/publish-preflight.toml").write_text(
                '[docli]\ngenerate_script = "scripts/generate-cli-docs.sh"\ndrift_paths = ["fixtures/cli/", "site/cli/"]\n')

            def run(command, **kwargs):
                self.assertEqual(kwargs, dict(cwd=root, check=True))
                self.assertEqual(tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"], "1.2.4")

            with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run", side_effect=run) as commands, patch.object(tag, "git") as git:
                tag.bump_version("1.2.3", "1.2.4")
            self.assertEqual([call.args[0] for call in commands.call_args_list], [
                ["cargo", "update", "--workspace"], ["bash", "scripts/generate-cli-docs.sh"]])
            self.assertEqual(git.call_args_list[0].args, (
                "add", "--", "Cargo.toml", "Cargo.lock", "CHANGELOG.md", "fixtures/cli/", "site/cli/"))
            self.assertEqual(git.call_args_list[1].args, ("commit", "-m", "release: prepare prerelease 1.2.4"))
            self.assertEqual((root / "CHANGELOG.md").read_text(),
                '# Changelog\n\n## 1.2.4 (unreleased)\n\n## 1.2.3 (unreleased)\n\n- Existing note.\n')

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
