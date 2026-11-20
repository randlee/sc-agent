"""Check the exact Python emitted by the consumer template."""
import ast
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock


TEMPLATE = Path(__file__).resolve().parents[1] / "python/sc_agent/templates/consumer.py.j2"


class ConsumerFormatTest(unittest.TestCase):
    def test_extract_reports_missing_sitr_and_passes_through_captures(self):
        with tempfile.TemporaryDirectory() as directory:
            variables = Path(directory) / "variables.json"
            output = Path(directory) / "consumer.py"
            variables.write_text(json.dumps({"client_name": "plan-anchor", "sitr": True, "batch_records": False,
                                             "settings_module": "consumer_settings", "settings_file": ".sc/app/settings.yaml",
                                             "settings_set_command": "app-settings set allow_remote_judge true|false",
                                             "models": ["dec"], "sc_decidr_version": "1.2.3", "sitr_version": "4.5.6"}))
            subprocess.run(["sc-compose", "render", "--root", str(TEMPLATE.parent), "--file", str(TEMPLATE),
                            "--var-file", str(variables), "--strict", "--output", str(output)],
                           check=True, stdin=subprocess.DEVNULL, timeout=60)
            namespace = {}
            exec(compile(output.read_text(), str(output), "exec"), namespace)
            with mock.patch.dict(sys.modules, {"sc_treesitter": None}):
                with self.assertRaises(namespace["ClientError"]) as caught:
                    namespace["extract"]("python", "query", "source")
            self.assertEqual(caught.exception.code, "DEPENDENCY.SITR")
            self.assertIn("sitr==4.5.6", caught.exception.message)
            self.assertIn("sitr==4.5.6", caught.exception.action)
            self.assertIsInstance(caught.exception.__cause__, ImportError)
            captures = [object()]
            logger = namespace["_CLIENT_LOGGER"] = object()
            wheel = types.SimpleNamespace(run_query=mock.Mock(return_value=captures))
            with mock.patch.dict(sys.modules, {"sc_treesitter": wheel}):
                self.assertIs(namespace["extract"]("python", "query", "source", captures=["c"]), captures)
            wheel.run_query.assert_called_once_with("python", "query", "source", captures=["c"], general_log=logger)

    def test_client_name_is_required_at_strict_render(self):
        with tempfile.TemporaryDirectory() as directory:
            variables = Path(directory) / "variables.json"
            variables.write_text(json.dumps({"models":["dec"],"sitr":False,"batch_records":False,
                "settings_module":"consumer_settings","settings_file":".sc/app/settings.yaml",
                "settings_set_command":"app-settings set allow_remote_judge true|false",
                "sc_decidr_version":"0.2.8","sitr_version":"0.2.8"}))
            result = subprocess.run(["sc-compose","render","--root",str(TEMPLATE.parent),"--file",str(TEMPLATE),
                "--var-file",str(variables),"--strict","--output",str(Path(directory) / "consumer.py")],
                stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=60)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("client_name", result.stdout + result.stderr)

    def test_rendered_indentation(self):
        for client_name in ("plan-anchor", "judge-jev"):
            for sitr in (False, True):
                with self.subTest(client_name=client_name, sitr=sitr), tempfile.TemporaryDirectory() as directory:
                    variables = Path(directory) / "variables.json"
                    output = Path(directory) / "consumer.py"
                    variables.write_text(json.dumps({
                        "client_name": client_name, "sitr": sitr, "batch_records": False,
                        "settings_module": "consumer_settings", "settings_file": ".sc/app/settings.yaml",
                        "settings_set_command": "app-settings set allow_remote_judge true|false", "models": ["dec"],
                        "sc_decidr_version": "0.2.2", "sitr_version": "0.2.2",
                    }))
                    subprocess.run([
                        "sc-compose", "render", "--root", str(TEMPLATE.parent),
                        "--file", str(TEMPLATE), "--var-file", str(variables),
                        "--strict", "--output", str(output),
                    ], check=True, stdin=subprocess.DEVNULL, timeout=60)
                    text = output.read_text()
                    self.assertIn("Rendered from sc-decidr 0.2.2 sc_agent/templates/consumer.py.j2", text)
                    tree = ast.parse(text)
                    functions = {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}
                    for name in ("call_result", "call_batch_results"):
                        arguments = {arg.arg for arg in functions[name].args.kwonlyargs}
                        self.assertIn("timeout_secs", arguments)
                        self.assertIn("logger", arguments)
                        self.assertNotIn("agent_role", arguments)
                    self.assertNotIn("general_log=general_log,)", text)
                    if sitr:
                        lines = text.splitlines()
                        index = functions["extract"].lineno - 1
                        self.assertEqual(lines[index - 2:index], ["", ""])


if __name__ == "__main__":
    unittest.main()
