"""Exercise typed integration refusals and safe subprocess diagnostics, without providers."""
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
import configuration
import integration_errors as errors
import preflight
import run


class Errors(unittest.TestCase):
    def invoke(self, *extra):
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            argv = ["run", "--run-id", "controlled", "--run-dir", directory,
                    "--tag", "prerelease/v0.1.5", "--platform", "macos-aarch64", *extra]
            with patch.object(sys, "argv", argv), contextlib.redirect_stdout(output):
                self.assertEqual(run.main(), 1)
        return json.loads(output.getvalue())

    def test_public_runner_refusals_have_distinct_ids_and_recovery(self):
        for option, value, code in [("--run-id", "bad id", "integration.invalid_run_id"),
                                    ("--tag", "not-a-tag", "integration.invalid_tag")]:
            with self.subTest(code=code):
                result = self.invoke(option, value)
                self.assertEqual(result["error"], code)
                self.assertEqual(result["detail"]["id"], code)
                self.assertTrue(result["reason"])
                self.assertTrue(result["remediation"])

    def test_case_configuration_refusal_is_typed_before_argument_choices_are_built(self):
        error = errors.Refusal("duplicate case ids", code="integration.duplicate_case_id", remediation="Give each case a unique id.")
        with patch.object(run, "required_cases", side_effect=error):
            result = self.invoke()
        self.assertEqual(result["error"], "integration.duplicate_case_id")
        self.assertEqual(result["remediation"], error.remediation)

    def test_configuration_and_report_failures_keep_their_own_remediation(self):
        for operation, code in [(lambda: configuration.platform_target({"release_targets": [], "python_distributions": []}, "unknown"), "integration.invalid_platform"),
                                (lambda: preflight.report_path("../escape.html"), "integration.invalid_report_path")]:
            with self.subTest(code=code), self.assertRaises(errors.Refusal) as caught:
                operation()
            detail = errors.failure_detail(caught.exception)
            self.assertEqual(detail["id"], code)
            self.assertTrue(detail["remediation"])

    def test_real_helper_child_stderr_is_captured_with_argv(self):
        args = [sys.executable, "-c", "import sys; sys.stderr.write('controlled child failure'); sys.exit(9)"]
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            preflight.prerelease_helper().command(args)
        detail = errors.failure_detail(caught.exception)
        self.assertEqual(detail["id"], "integration.subprocess_failed")
        self.assertEqual(detail["returncode"], 9)
        self.assertEqual(detail["stderr_tail"], "controlled child failure")
        self.assertEqual(detail["argv"], [sys.executable, "-c", "<python-code>"])

    def test_runner_subprocess_failure_redacts_argv_and_bounds_stderr(self):
        failure = subprocess.CalledProcessError(4, ["gh", "release", "view", "--token", "not-in-environment"],
            output="PRIVATE PROVIDER STDOUT", stderr="x" * 5000 + " set GH_TOKEN private-token https://user:password@github.com Bearer hidden")
        with patch.dict(os.environ, {"GH_TOKEN": "private-token"}), patch.object(run, "prepare", side_effect=failure):
            result = self.invoke()
        detail = result["detail"]
        self.assertEqual(result["error"], "integration.subprocess_failed")
        self.assertEqual(detail["argv"][-1], "[redacted]")
        self.assertLessEqual(len(detail["stderr_tail"]), 4096)
        self.assertIn("set GH_TOKEN", detail["stderr_tail"])
        for secret in ["not-in-environment", "private-token", "user:password", "hidden", "PRIVATE PROVIDER"]:
            self.assertNotIn(secret, json.dumps(result))

    def test_validator_input_and_provider_text_stay_private(self):
        failure = subprocess.CalledProcessError(1, ["python", "-c", "sc_agent.validate_integration_results(secret_input)"],
            output="PROVIDER STDOUT", stderr="ValueError: PROVIDER STDERR")
        detail = errors.failure_detail(failure)
        self.assertEqual(detail["argv"][-1], "<integration-validator>")
        self.assertIn("withheld", detail["stderr_tail"])
        self.assertNotIn("PROVIDER", json.dumps(detail))
        self.assertNotIn("secret_input", json.dumps(detail))

    def test_preflight_reports_per_platform_typed_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "site/reports/integration/run.json"
            report.parent.mkdir(parents=True)
            report.write_text(json.dumps({"report": "site/reports/integration/run.html"}))
            failure = errors.Refusal("stale source", code="integration.stale_source", remediation="Run the new prerelease.")
            output = io.StringIO()
            with patch.object(preflight, "ROOT", root), patch.object(preflight, "git", return_value="candidate"), \
                    patch.object(preflight, "pushed_ref_contains", return_value=True), \
                    patch.object(preflight, "validate_run", side_effect=failure), \
                    patch.object(sys, "argv", ["preflight"]), contextlib.redirect_stdout(output):
                self.assertEqual(preflight.main(), 1)
            result = json.loads(output.getvalue())
            self.assertEqual(result["error"], "integration.no_passing_platform")
            self.assertEqual(result["reports"], ["run.json: stale source"])
            self.assertEqual(result["failures"][0]["error"], {"id": "integration.stale_source", "message": "stale source", "remediation": "Run the new prerelease."})

    def test_lower_json_parse_error_is_preserved_with_typed_recovery(self):
        try:
            json.loads("invalid JSON")
        except json.JSONDecodeError as error:
            detail = errors.failure_detail(error)
            self.assertEqual(detail["id"], "integration.invalid_json")
            self.assertEqual(detail["cause"]["message"], str(error))
            self.assertTrue(detail["remediation"])


if __name__ == "__main__":
    unittest.main()
