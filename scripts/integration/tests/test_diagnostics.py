"""Infrastructure errors remain actionable without exposing provider/credential data."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import preflight


class Diagnostics(unittest.TestCase):
    def test_main_reports_the_failed_git_command_and_stderr(self):
        args = ["git", "ls-files", "--error-unmatch", "--", "site/reports/integration/run.json"]
        failure = subprocess.CalledProcessError(1, args, output="PRIVATE PROVIDER STDOUT", stderr="error: pathspec is not tracked")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "site/reports/integration/run.json"
            report.parent.mkdir(parents=True)
            report.write_text("{}")
            def git(*args, **kwargs):
                if args[0] == "ls-files":
                    raise failure
                return "candidate"
            output = io.StringIO()
            with patch.object(preflight, "ROOT", root), patch.object(preflight, "git", side_effect=git), \
                    patch.object(sys, "argv", ["preflight"]), contextlib.redirect_stdout(output):
                self.assertEqual(preflight.main(), 1)
            data = json.loads(output.getvalue())
            self.assertEqual(data["error"], "integration.no_passing_platform")
            self.assertIn("git ls-files --error-unmatch", data["reports"][0])
            self.assertIn("stderr: error: pathspec is not tracked", data["reports"][0])
            self.assertNotIn(failure.stdout, output.getvalue())

    def test_initial_fetch_failure_has_the_same_safe_diagnostics(self):
        failure = subprocess.CalledProcessError(128, ["git", "fetch", "--prune", "origin"], stderr="fatal: origin is unavailable")
        def git(*args):
            if args[0] == "fetch":
                raise failure
            return "candidate"
        output = io.StringIO()
        with patch.object(preflight, "git", side_effect=git), patch.object(sys, "argv", ["preflight"]), contextlib.redirect_stdout(output):
            self.assertEqual(preflight.main(), 1)
        data = json.loads(output.getvalue())
        self.assertEqual(data["error"], "integration.command_failed")
        self.assertIn("git fetch --prune origin", data["reason"])
        self.assertIn("fatal: origin is unavailable", data["reason"])

    def test_gh_auth_failure_is_actionable_but_tokens_and_url_credentials_are_redacted(self):
        failure = subprocess.CalledProcessError(4, ["gh", "release", "view", "prerelease/v0.1.4"],
            stderr="Set GH_TOKEN; token secret-token; https://user:other-password@github.com/repo Bearer auth-secret")
        with patch.dict(os.environ, {"GH_TOKEN": "secret-token"}):
            message = preflight.subprocess_failure(failure)
        self.assertIn("gh release view prerelease/v0.1.4 failed (exit 4)", message)
        self.assertIn("Set GH_TOKEN", message)
        for secret in ["secret-token", "other-password", "auth-secret"]:
            self.assertNotIn(secret, message)
        self.assertIn("[redacted]", message)

    def test_validator_stderr_keeps_import_failures_but_never_provider_evidence(self):
        args = ["python", "-I", "-c", "sc_agent.validate_integration_results(private_payload)"]
        for stderr, expected in [("ModuleNotFoundError: No module named 'sc_agent'", "ModuleNotFoundError"),
                                 ("ValueError: PRIVATE PROVIDER EVIDENCE", "withheld")]:
            with self.subTest(stderr=stderr):
                failure = subprocess.CalledProcessError(1, args, output="PRIVATE PROVIDER STDOUT", stderr=stderr)
                message = preflight.subprocess_failure(failure)
                self.assertIn(expected, message)
                self.assertIn("<integration-validator>", message)
                self.assertNotIn("private_payload", message)
                self.assertNotIn("PRIVATE PROVIDER", message)


if __name__ == "__main__":
    unittest.main()
