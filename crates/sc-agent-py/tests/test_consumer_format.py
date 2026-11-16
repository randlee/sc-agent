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
            variables.write_text(json.dumps({"general_log": False, "sitr": True, "models": ["dec"],
                                             "sc_decidr_version": "1.2.3", "sitr_version": "4.5.6"}))
            subprocess.run(["sc-compose", "render", "--root", str(TEMPLATE.parent), "--file", str(TEMPLATE),
                            "--var-file", str(variables), "--strict", "--output", str(output)], check=True)
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
            wheel = types.SimpleNamespace(run_query=mock.Mock(return_value=captures))
            with mock.patch.dict(sys.modules, {"sc_treesitter": wheel}):
                self.assertIs(namespace["extract"]("python", "query", "source", captures=["c"]), captures)
            wheel.run_query.assert_called_once_with("python", "query", "source", captures=["c"])

    def test_rendered_indentation(self):
        for general_log in (False, True):
            for sitr in (False, True):
                with self.subTest(general_log=general_log, sitr=sitr), tempfile.TemporaryDirectory() as directory:
                    variables = Path(directory) / "variables.json"
                    output = Path(directory) / "consumer.py"
                    variables.write_text(json.dumps({
                        "general_log": general_log, "sitr": sitr, "models": ["dec"],
                        "sc_decidr_version": "0.2.2", "sitr_version": "0.2.2",
                    }))
                    subprocess.run([
                        "sc-compose", "render", "--root", str(TEMPLATE.parent),
                        "--file", str(TEMPLATE), "--var-file", str(variables),
                        "--strict", "--output", str(output),
                    ], check=True)
                    text = output.read_text()
                    self.assertIn("Rendered from sc-decidr 0.2.2 sc_agent/templates/consumer.py.j2", text)
                    tree = ast.parse(text)
                    functions = {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}
                    for name in ("call_result", "call_batch_results"):
                        arguments = {arg.arg for arg in functions[name].args.kwonlyargs}
                        self.assertIn("timeout_secs", arguments)
                        self.assertEqual("general_log" in arguments, general_log)
                    self.assertNotIn("general_log=general_log,)", text)


if __name__ == "__main__":
    unittest.main()
