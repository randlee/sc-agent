"""Credential export uses current provider metadata without reading credentials."""
import importlib.util
import contextlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import call, patch

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/integration.py"
spec = importlib.util.spec_from_file_location("integration_runner", SCRIPT)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class IntegrationExportTests(unittest.TestCase):
    def setUp(self):
        self.providers = [
            {"name": name, "key": variable}
            for name, variable in [
                ("typesafe", "TYPESAFE_API_KEY"),
                ("anthropic", "ANTHROPIC_API_KEY"),
                ("openai", "OPENAI_API_KEY"),
                ("openrouter", "OPENROUTER_API_KEY"),
                ("together", "TOGETHER_API_KEY"),
                ("nous", "NOUS_API_KEY"),
                ("local", ""),
            ]
        ]
        query = patch.object(runner.subprocess, "run", side_effect=lambda *args, **kwargs:
                             subprocess.CompletedProcess(args[0], 0, json.dumps({"ok": True, "value": self.providers}), ""))
        self.query = query.start()
        self.addCleanup(query.stop)

    def test_credential_variables_use_the_model_catalog(self):
        self.providers = [{"name": "reseller", "key": "BILLING_CREDENTIAL"}, {"name": "local", "key": ""}]
        self.assertEqual(runner.credential_variables(), {"reseller": "BILLING_CREDENTIAL"})
        self.assertEqual(self.query.call_args.args[0], ["decidr", "models"])

    def test_removed_suite_mode_is_rejected_before_credential_lookup(self):
        stderr = io.StringIO()
        with patch.object(sys, "argv", [str(SCRIPT), "--export-env", "unused", "--venv", "unused"]), \
             patch.object(runner, "key_for", side_effect=AssertionError("unexpected credential lookup")), \
             contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit) as stopped:
            runner.main()
        self.assertEqual(stopped.exception.code, 2)
        self.assertIn("unrecognized arguments: --venv", stderr.getvalue())

    def test_export_uses_current_provider_variables_and_injected_lookup(self):
        self.providers.append({"name": "reseller", "key": "BILLING_CREDENTIAL"})
        keys = {"TYPESAFE_API_KEY": "fixture-typesafe", "OPENAI_API_KEY": "fixture-openai", "NOUS_API_KEY": "fixture-nous", "BILLING_CREDENTIAL": "fixture-reseller"}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keys.env"
            with patch.dict(os.environ, {}, clear=True), \
                 patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                 patch.object(runner, "key_for", side_effect=lambda provider, variable: keys.get(variable)) as lookup:
                self.assertEqual(runner.main(), 0)
            self.assertEqual(lookup.call_args_list, [
                call("typesafe", "TYPESAFE_API_KEY"),
                call("anthropic", "ANTHROPIC_API_KEY"),
                call("openai", "OPENAI_API_KEY"),
                call("openrouter", "OPENROUTER_API_KEY"),
                call("together", "TOGETHER_API_KEY"),
                call("nous", "NOUS_API_KEY"),
                call("reseller", "BILLING_CREDENTIAL"),
            ])
            self.assertEqual(output.read_text(), "TYPESAFE_API_KEY=fixture-typesafe\nOPENAI_API_KEY=fixture-openai\nNOUS_API_KEY=fixture-nous\nBILLING_CREDENTIAL=fixture-reseller\n")
            if os.name == "posix":
                self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)

    def test_export_with_only_an_anthropic_key(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keys.env"
            with patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                 patch.object(runner, "key_for", side_effect=lambda provider, variable:
                              "fixture-key" if provider == "anthropic" else None):
                self.assertEqual(runner.main(), 0)
            self.assertEqual(output.read_text(), "ANTHROPIC_API_KEY=fixture-key\n")

    def test_export_refuses_before_writing_without_credentials(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keys.env"
            with patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                 patch.object(runner, "key_for", return_value=None), \
                 contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as stopped:
                runner.main()
            self.assertEqual(stopped.exception.code, 2)
            self.assertFalse(output.exists())

    def test_export_accepts_any_catalog_provider(self):
        self.providers = [{"name":"reseller", "key":"BILLING_CREDENTIAL"}]
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keys.env"
            with patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                 patch.object(runner, "key_for", return_value="fixture-key"):
                self.assertEqual(runner.main(), 0)
            self.assertEqual(output.read_text(), "BILLING_CREDENTIAL=fixture-key\n")

    def test_required_credential_names_come_from_the_catalog(self):
        self.providers[0]["key"] = "CUSTOM_TYPESAFE_CREDENTIAL"
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keys.env"
            with patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                 patch.object(runner, "key_for", side_effect=lambda provider, variable:
                              "fixture-key" if variable == "CUSTOM_TYPESAFE_CREDENTIAL" else None):
                self.assertEqual(runner.main(), 0)
            self.assertEqual(output.read_text(), "CUSTOM_TYPESAFE_CREDENTIAL=fixture-key\n")

    def test_catalog_failure_keeps_the_cli_error(self):
        output = '{"ok":false,"error":{"id":"catalog.unavailable","message":"catalog failure"}}'
        self.query.side_effect = None
        self.query.return_value = subprocess.CompletedProcess(["decidr", "models"], 1, output, "native detail")
        with self.assertRaises(RuntimeError) as stopped:
            runner.credential_variables()
        self.assertIn(output, str(stopped.exception))
        self.assertIn("native detail", str(stopped.exception))


if __name__ == "__main__":
    unittest.main()
