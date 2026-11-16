"""Check the exact Python emitted by the consumer template."""
import ast
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


TEMPLATE = Path(__file__).resolve().parents[1] / "python/sc_agent/templates/consumer.py.j2"


class ConsumerFormatTest(unittest.TestCase):
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
                    tree = ast.parse(text)
                    functions = {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}
                    for name in ("call_result", "call_batch_results"):
                        arguments = {arg.arg for arg in functions[name].args.kwonlyargs}
                        self.assertIn("timeout_secs", arguments)
                        self.assertEqual("general_log" in arguments, general_log)
                    self.assertNotIn("general_log=general_log,)", text)


if __name__ == "__main__":
    unittest.main()
