"""Controlled release-script planning and wheel metadata checks; no tags or uploads are made."""
import importlib.util
import contextlib
import io
import json
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("prerelease_tag", ROOT / "scripts/prerelease_tag.py")
tag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tag)


class ReleaseContract(unittest.TestCase):
    def test_version_bump_regenerates_and_stages_release_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## 1.2.3 (unreleased)\n\n- Existing note.\n\n## 1.2.2\n\n- Older note.\n')
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
                '# Changelog\n\n## 1.2.4 (unreleased)\n\n- Existing note.\n\n## 1.2.2\n\n- Older note.\n')

    def test_empty_notes_refuse_bump_before_any_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo = root / "Cargo.toml"
            cargo.write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            changelog = root / "CHANGELOG.md"
            for body in ("", "\n### Fixes\n"):
                changelog.write_text(f'# Changelog\n\n## 1.2.3 (unreleased)\n{body}\n## 1.2.2\n\n- Older note.\n')
                before = {p: p.read_bytes() for p in (cargo, changelog)}
                with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run") as commands, patch.object(tag, "git") as git:
                    with self.assertRaises(tag.ReleaseNotesError) as caught:
                        tag.bump_version("1.2.3", "1.2.4")
                self.assertEqual(caught.exception.code, "prerelease.changelog_empty")
                self.assertEqual({p: p.read_bytes() for p in before}, before)
                commands.assert_not_called()
                git.assert_not_called()

    def test_main_refuses_empty_notes_without_tagging_or_pushing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.2.3"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## 1.2.3 (unreleased)\n')
            (root / "release").mkdir()
            (root / "release/publish-artifacts.toml").write_text('[prerelease]\nprotected_branches = ["main"]\n')
            error = io.StringIO()
            with patch.object(tag, "ROOT", root), patch.object(sys, "argv", ["prerelease_tag.py"]), patch.object(tag, "git", side_effect=["fix/release", ""]) as git, patch.object(tag.subprocess, "run") as commands, contextlib.redirect_stderr(error):
                self.assertEqual(tag.main(), 2)
            self.assertEqual(json.loads(error.getvalue())["error"]["id"], "prerelease.changelog_empty")
            self.assertEqual([call.args for call in git.call_args_list], [
                ("branch", "--show-current"), ("status", "--porcelain")])
            commands.assert_not_called()

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
