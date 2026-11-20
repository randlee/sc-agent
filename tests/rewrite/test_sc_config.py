"""Configuration precedence at the CLI boundary."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class Configuration(unittest.TestCase):
    def test_project_precedence(self):
        binary = Path(__file__).resolve().parents[2] / 'rewrite/target/debug/decidr'
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = root / 'config'
            config.mkdir()
            default = config / 'default.json'
            local = config / 'local.json'
            default.write_text(json.dumps({'project': 'default'}))
            local.write_text(json.dumps({'project': 'local'}))
            env = {'PATH': os.defpath, 'HOME': temp, 'DECIDR__PROJECT': 'environment',
                   'SC_AGENT_TEST_REPO_ROOT': temp}
            def doctor(*args):
                done = subprocess.run([str(binary), 'doctor', *args], cwd=root, env=env, text=True, capture_output=True, timeout=10)
                self.assertEqual(done.returncode, 0, done.stderr)
                return json.loads(done.stdout)['value']['project']['value']
            self.assertEqual(doctor('--project', 'flag'), 'flag')
            self.assertEqual(doctor(), 'environment')
            del env['DECIDR__PROJECT']
            self.assertEqual(doctor(), 'local')
            local.unlink()
            self.assertEqual(doctor(), 'default')
