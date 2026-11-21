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
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("prerelease_tag", ROOT / "scripts/prerelease_tag.py")
tag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tag)


class ReleaseContract(unittest.TestCase):
    def test_remote_tag_refresh_selects_next_version_before_heading_check(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## Unreleased\n\n- Pending note.\n\n## 1.2.3\n\n- Shipped note.\n')
            (root / "release").mkdir()
            (root / "release/publish-artifacts.toml").write_text('[prerelease]\nprotected_branches = ["main"]\ntag_prefix = "prerelease/v"\n')
            fetched = False

            def git(*args, capture=True):
                nonlocal fetched
                if args == ("branch", "--show-current"):
                    return "fix/release"
                if args == ("status", "--porcelain"):
                    return ""
                if args == ("fetch", "origin", "--tags"):
                    fetched = True
                    return ""
                if args == ("tag", "--list"):
                    return "prerelease/v1.2.3" if fetched else ""
                return ""

            error = io.StringIO()
            with patch.object(tag, "ROOT", root), patch.object(sys, "argv", ["prerelease_tag.py"]), patch.object(tag, "git", side_effect=git) as calls, patch.object(tag.subprocess, "run", return_value=SimpleNamespace(stdout='[{"tagName":"prerelease/v1.2.3"}]')) as commands, contextlib.redirect_stderr(error), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(tag.main(), 0, error.getvalue())
            self.assertTrue(fetched)
            self.assertEqual(tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"], "1.2.4")
            self.assertIn('## 1.2.4\n\n- Pending note.', (root / "CHANGELOG.md").read_text())
            self.assertIn('## 1.2.3\n\n- Shipped note.', (root / "CHANGELOG.md").read_text())
            self.assertEqual([call.args for call in calls.call_args_list][-2:], [
                ("tag", "-a", "prerelease/v1.2.4", "-m", "Prerelease 1.2.4"),
                ("push", "origin", "refs/tags/prerelease/v1.2.4")])
            self.assertEqual([call.args[0] for call in commands.call_args_list], [
                ["gh", "release", "list", "--limit", "1000", "--json", "tagName"],
                ["cargo", "update", "--workspace"]])

    def test_current_version_freezes_notes_without_updating_release_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payloads = {"Cargo.toml": '[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n',
                        "Cargo.lock": "unchanged lockfile\n"}
            for name, text in payloads.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)
            changelog = root / "CHANGELOG.md"
            history = '## 1.2.2\n\n- Shipped note.\n'
            changelog.write_text('# Changelog\n\n## Unreleased\n\n- Pending note.\n\n' + history)
            with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run") as commands, patch.object(tag, "git") as git:
                tag.bump_version("1.2.3", "1.2.3")
            self.assertEqual(changelog.read_text(),
                '# Changelog\n\n## Unreleased\n\n## 1.2.3\n\n- Pending note.\n\n' + history)
            self.assertEqual({name: (root / name).read_text() for name in payloads}, payloads)
            commands.assert_not_called()
            self.assertEqual([call.args for call in git.call_args_list], [
                ("add", "--", "CHANGELOG.md"), ("commit", "-m", "release: prepare prerelease 1.2.3")])

    def test_existing_selected_version_refuses_after_refresh_before_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## Unreleased\n\n- Pending note.\n\n## 1.2.3\n\n- Shipped note.\n')
            (root / "release").mkdir()
            (root / "release/publish-artifacts.toml").write_text('[prerelease]\nprotected_branches = ["main"]\n')
            before = {path: path.read_bytes() for path in root.rglob("*") if path.is_file()}
            with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run") as commands, patch.object(tag, "git") as git:
                with self.assertRaises(tag.ReleaseNotesError) as caught:
                    tag.bump_version("1.2.3", "1.2.3")
            self.assertEqual(caught.exception.code, "prerelease.changelog_version_exists")
            commands.assert_not_called()
            git.assert_not_called()
            error = io.StringIO()
            with patch.object(tag, "ROOT", root), patch.object(sys, "argv", ["prerelease_tag.py"]), patch.object(tag, "git", side_effect=["fix/release", "", "", ""]) as git, patch.object(tag.subprocess, "run", return_value=SimpleNamespace(stdout="[]")) as commands, contextlib.redirect_stderr(error):
                self.assertEqual(tag.main(), 2)
            self.assertEqual(json.loads(error.getvalue())["error"]["id"], "prerelease.changelog_version_exists")
            self.assertEqual([call.args for call in git.call_args_list], [
                ("branch", "--show-current"), ("status", "--porcelain"), ("fetch", "origin", "--tags"), ("tag", "--list")])
            self.assertEqual(commands.call_count, 1)
            self.assertEqual(commands.call_args.args[0][:3], ["gh", "release", "list"])
            self.assertEqual({path: path.read_bytes() for path in before}, before)

    def test_dry_run_describes_note_only_and_version_bump_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.2.3"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## Unreleased\n\n- Pending note.\n\n## 1.2.2\n\n- Shipped note.\n')
            (root / "release").mkdir()
            (root / "release/publish-artifacts.toml").write_text('[prerelease]\nprotected_branches = ["main"]\ntag_prefix = "prerelease/v"\n')
            for tags, expected in [("", "keep manifests and Cargo.lock unchanged"),
                                   ("prerelease/v1.2.3", "update manifests and Cargo.lock")]:
                output = io.StringIO()
                with patch.object(tag, "ROOT", root), patch.object(sys, "argv", ["prerelease_tag.py", "--dry-run"]), patch.object(tag, "git", side_effect=["fix/release", "", tags]), patch.object(tag.subprocess, "run") as commands, contextlib.redirect_stdout(output):
                    self.assertEqual(tag.main(), 0)
                self.assertIn(expected, output.getvalue())
                self.assertIn("freeze Unreleased", output.getvalue())
                self.assertIn("commit release notes", output.getvalue())
                self.assertNotIn("version bump", output.getvalue())
                commands.assert_not_called()

    def test_version_bump_regenerates_and_stages_release_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            history = '## 1.2.3\n\n- Existing shipped note.\n\n## 1.2.2\n\n- Older note.\n'
            changelog = root / "CHANGELOG.md"
            changelog.write_text('# Changelog\n\n## Unreleased\n\n- First pending change.\n\n' + history)
            (root / "release").mkdir()

            selected = "1.2.4"

            def run(command, **kwargs):
                self.assertEqual(kwargs, dict(cwd=root, check=True))
                self.assertEqual(tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"], selected)

            with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run", side_effect=run) as commands, patch.object(tag, "git") as git:
                tag.bump_version("1.2.3", "1.2.4")
            self.assertEqual([call.args[0] for call in commands.call_args_list], [
                ["cargo", "update", "--workspace"]])
            self.assertEqual(git.call_args_list[0].args, (
                "add", "--", "Cargo.toml", "Cargo.lock", "CHANGELOG.md"))
            self.assertEqual(git.call_args_list[1].args, ("commit", "-m", "release: prepare prerelease 1.2.4"))
            first_release = '## 1.2.4\n\n- First pending change.\n\n' + history
            self.assertEqual(changelog.read_text(), '# Changelog\n\n## Unreleased\n\n' + first_release)

            before = {p: p.read_bytes() for p in (root / "Cargo.toml", changelog)}
            with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run") as commands, patch.object(tag, "git") as git:
                with self.assertRaises(tag.ReleaseNotesError) as caught:
                    tag.bump_version("1.2.4", "1.2.5")
            self.assertEqual(caught.exception.code, "prerelease.changelog_empty")
            self.assertEqual({p: p.read_bytes() for p in before}, before)
            commands.assert_not_called()
            git.assert_not_called()

            changelog.write_text('# Changelog\n\n## Unreleased\n\n- Second pending change.\n\n' + first_release)
            selected = "1.2.5"
            with patch.object(tag, "ROOT", root), patch.object(tag.subprocess, "run", side_effect=run), patch.object(tag, "git"):
                tag.bump_version("1.2.4", selected)
            self.assertEqual(changelog.read_text(),
                '# Changelog\n\n## Unreleased\n\n## 1.2.5\n\n- Second pending change.\n\n' + first_release)

    def test_empty_notes_refuse_bump_before_any_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo = root / "Cargo.toml"
            cargo.write_text('[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n')
            changelog = root / "CHANGELOG.md"
            for body in ("", "\n### Fixes\n"):
                changelog.write_text(f'# Changelog\n\n## Unreleased\n{body}\n## 1.2.3\n\n- Shipped note.\n')
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
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## Unreleased\n\n## 1.2.3\n\n- Shipped note.\n')
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
