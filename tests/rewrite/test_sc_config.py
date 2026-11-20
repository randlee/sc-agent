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
            local = root / 'repo' / '.sc'
            global_dir = root / 'home' / '.sc-agent'
            local.mkdir(parents=True)
            global_dir.mkdir(parents=True)
            files = [local / 'decidr.json', local / 'sc-agent.json', global_dir / 'decidr.json', global_dir / 'sc-agent.json']
            values = ['local-decidr', 'local-shared', 'global-decidr', 'global-shared']
            for path, value in zip(files, values):
                path.write_text(json.dumps({'project': value}))
            env = {'PATH': os.defpath, 'HOME': str(root / 'home'), 'SC_PROJECT': 'environment',
                   'SC_AGENT_TEST_REPO_ROOT': str(root / 'repo')}
            def doctor(*args):
                done = subprocess.run([str(binary), 'doctor', *args], cwd=root, env=env, text=True, capture_output=True, timeout=10)
                self.assertEqual(done.returncode, 0, done.stderr)
                return json.loads(done.stdout)['value']['project']['value']
            self.assertEqual(doctor('--project', 'flag'), 'flag')
            self.assertEqual(doctor(), 'environment')
            del env['SC_PROJECT']
            for path, value in zip(files, values):
                self.assertEqual(doctor(), value)
                path.unlink()
