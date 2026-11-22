"""Credential export uses current provider metadata without reading credentials."""
import importlib.util
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
    def test_export_uses_current_provider_variables_and_injected_lookup(self):
        keys = {"typesafe": "fixture-typesafe", "anthropic": None, "openai": "fixture-openai"}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keys.env"
            with patch.dict(os.environ, {}, clear=True), \
                 patch.object(sys, "argv", [str(SCRIPT), "--export-env", str(output)]), \
                 patch.object(runner, "key_for", side_effect=lambda provider, variable: keys[provider]) as lookup:
                self.assertEqual(runner.main(), 0)
            self.assertEqual(lookup.call_args_list, [
                call("typesafe", "TYPESAFE_API_KEY"),
                call("anthropic", "ANTHROPIC_API_KEY"),
                call("openai", "OPENAI_API_KEY"),
            ])
            self.assertEqual(output.read_text(), "TYPESAFE_API_KEY=fixture-typesafe\nOPENAI_API_KEY=fixture-openai\n")
            if os.name == "posix":
                self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)


if __name__ == "__main__":
    unittest.main()
