"""Exercise the candidate CLI against an isolated loopback provider."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        self.server.requests.append((self.path, self.headers.get('Authorization'), self.rfile.read(int(self.headers['Content-Length']))))
        content = b'{"answers":{"q":{"type":"noul","noul":0.8}}}'
        self.send_response(200)
        self.send_header('Content-Length', str(len(content)))
        self.end_headers()
        self.wfile.write(content)

    def log_message(self, *_):
        pass


class CliCore(unittest.TestCase):
    def test_send_and_missing_project(self):
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.requests = []
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as home:
                request = Path(home) / 'request.json'
                request.write_text('{"questions": {"q": {"type": "noul"}}}')
                (Path(home) / 'config').mkdir()
                (Path(home) / 'config/default.json').write_text('{}')
                env = {'SC_AGENT_TEST_REPO_ROOT': home, 'HOME': home, 'PATH': os.defpath, 'TYPESAFE_API_KEY': 'test-key',
                       'SC_AGENT_TEST_BASE_URL': f'http://127.0.0.1:{server.server_port}'}
                binary = Path(__file__).resolve().parents[2] / 'rewrite/target/debug/decidr'
                args = [str(binary), 'send', '--model', 'jev', '--agent-role', 'test', str(request)]
                missing = subprocess.run(args, env=env, text=True, capture_output=True, timeout=15)
                self.assertNotEqual(missing.returncode, 0)
                self.assertEqual(json.loads(missing.stdout)['error']['id'], 'missing_project')
                self.assertEqual(server.requests, [])
                env['SC_PROJECT'] = 'contract'
                sent = subprocess.run(args, env=env, text=True, capture_output=True, timeout=15)
                self.assertEqual(sent.returncode, 0, sent.stderr)
                self.assertTrue(json.loads(sent.stdout)['ok'])
                self.assertEqual(len(server.requests), 1)
                self.assertEqual(server.requests[0][0], '/systemone')
                self.assertEqual(server.requests[0][1], 'Bearer test-key')
                self.assertNotIn('test-key', sent.stdout + sent.stderr)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_local_laya_uses_config_without_key(self):
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.requests = []
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as home:
                config = Path(home) / '.sc'
                config.mkdir()
                (config / 'decidr.json').write_text(json.dumps({'project': 'local', 'local_runners': {'laya': {'host': '127.0.0.1', 'port': server.server_port, 'model': 'local-model'}}}))
                request = Path(home) / 'request.json'
                request.write_text('{"questions": {}}')
                env = {'PATH': os.defpath, 'HOME': home, 'SC_AGENT_TEST_REPO_ROOT': home}
                binary = Path(__file__).resolve().parents[2] / 'rewrite/target/debug/decidr'
                done = subprocess.run([str(binary), 'send', '--model', 'laya', '--agent-role', 'test', str(request)], env=env, text=True, capture_output=True, timeout=15)
                self.assertEqual(done.returncode, 0, done.stderr + done.stdout)
                self.assertTrue(json.loads(done.stdout)['ok'])
                self.assertEqual(server.requests[0][0], '/v1/systemone')
                self.assertIsNone(server.requests[0][1])
                self.assertEqual(json.loads(server.requests[0][2])['model'], 'local-model')
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
