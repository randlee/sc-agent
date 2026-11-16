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
                    ast.parse(text)
                    lines = text.splitlines()
                    self.assertEqual(lines.count("    timeout_secs: Optional[float] = None,"), 2)
                    self.assertEqual(lines.count("    general_log: Optional[LogSink] = None,"), 2 if general_log else 0)
                    self.assertNotIn("general_log=general_log,)", text)


if __name__ == "__main__":
    unittest.main()
