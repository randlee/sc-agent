"""Credential export uses current provider metadata without reading credentials."""
import importlib.util
import contextlib
import io
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import call, patch

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/integration.py"
spec = importlib.util.spec_from_file_location("integration_runner", SCRIPT)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class IntegrationExportTests(unittest.TestCase):
    def test_removed_suite_mode_is_rejected_before_credential_lookup(self):
        stderr = io.StringIO()
        with patch.object(sys, "argv", [str(SCRIPT), "--export-env", "unused", "--venv", "unused"]), \
             patch.object(runner, "key_for", side_effect=AssertionError("unexpected credential lookup")), \
             contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit) as stopped:
            runner.main()
        self.assertEqual(stopped.exception.code, 2)
        self.assertIn("unrecognized arguments: --venv", stderr.getvalue())

    def test_export_uses_current_provider_variables_and_injected_lookup(self):
        keys = {"TYPESAFE_API_KEY": "fixture-typesafe", "OPENAI_API_KEY": "fixture-openai", "NOUS_API_KEY": "fixture-nous"}
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
            ])
            self.assertEqual(output.read_text(), "TYPESAFE_API_KEY=fixture-typesafe\nOPENAI_API_KEY=fixture-openai\nNOUS_API_KEY=fixture-nous\n")
            if os.name == "posix":
                self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)

    def test_export_requires_one_of_typesafe_anthropic_nous(self):
        for keys, accepted in [({"NOUS_API_KEY": "n"}, True), ({"ANTHROPIC_API_KEY": "a"}, True),
                               ({"OPENAI_API_KEY": "o", "OPENROUTER_API_KEY": "r"}, False)]:
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "keys.env"
                with patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                     patch.object(runner, "key_for", side_effect=lambda provider, variable: keys.get(variable)):
                    if accepted:
                        self.assertEqual(runner.main(), 0)
                        self.assertEqual(output.read_text(), "".join(f"{k}={v}\n" for k, v in keys.items()))
                    else:
                        with self.assertRaisesRegex(RuntimeError, "NOUS_API_KEY is required"):
                            runner.main()


if __name__ == "__main__":
    unittest.main()
