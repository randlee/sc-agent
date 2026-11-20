"""Additional checks required before accepting the rewrite."""
import ast
import importlib.resources
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Reply(BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers['Content-Length']))
        data = b'{"answers":{"q":{"type":"noul","noul":0.8}}}'
        self.send_response(200)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_):
        pass


class SchemaReply(BaseHTTPRequestHandler):
    def do_POST(self):
        data = self.rfile.read(int(self.headers['Content-Length']))
        self.server.requests.append(json.loads(data))
        answer = {'type': 'choice', 'choice': 'yes', 'confidence': 0.9}
        if len(self.server.requests) > 1:
            answer['probabilities'] = {'yes': 0.9, 'no': 0.1}
        content = json.dumps({'choices': [{'message': {'content': json.dumps({'answers': {'q': answer}})}}]}).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(content)))
        self.end_headers()
        self.wfile.write(content)

    def log_message(self, *_):
        pass


class Required(unittest.TestCase):
    def test_generated_client_and_template(self):
        template = importlib.resources.files('sc_agent').joinpath('templates/consumer.py.j2')
        with importlib.resources.as_file(template) as path, tempfile.TemporaryDirectory() as render_home:
            render_env = os.environ.copy()
            render_env.update(HOME=render_home, USERPROFILE=render_home)
            rendered = subprocess.run(['sc-compose', 'render', '--root', str(path.parent), '--file', path.name, '--var', 'client_name=example', '--var', 'sc_decidr_version=0.1.0'], env=render_env, text=True, capture_output=True, timeout=30)
        self.assertEqual(rendered.returncode, 0, rendered.stderr)
        source = rendered.stdout
        tree = ast.parse(source)
        self.assertFalse(any(isinstance(node, ast.Raise) for node in ast.walk(tree)))
        server = ThreadingHTTPServer(('127.0.0.1', 0), Reply)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as home:
                config = Path(home) / 'config'
                config.mkdir()
                (config / 'default.json').write_text('{}')
                script = Path(home) / 'client.py'
                script.write_text(source + '''\nimport json\nassert send(json.dumps({"questions": {}}), "jev")["ok"]\nassert extract("rust", "(identifier) @id", "fn main() {} ")["ok"]\n''')
                env = {'PATH': os.defpath, 'HOME': home, 'USERPROFILE': home, 'SC_PROJECT': 'example',
                       'TYPESAFE_API_KEY': 'test-key', 'SC_AGENT_TEST_REPO_ROOT': home, 'SC_AGENT_TEST_BASE_URL': f'http://127.0.0.1:{server.server_port}'}
                result = subprocess.run([sys.executable, str(script)], env=env, text=True, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr)
                root = Path(home) / '.sc-agent'
                rows = [json.loads(line) for line in (root / 'logs/example.log.jsonl').read_text().splitlines()]
                self.assertEqual({r['action'] for r in rows}, {'decidr.send', 'sitr.query'})
                sessions = list((root / 'sessions').rglob('sessions.jsonl'))
                self.assertEqual(len(sessions), 1)
                session = json.loads(sessions[0].read_text().splitlines()[0])
                self.assertEqual(session['fields']['agent_role'], 'example')
                self.assertEqual(session['fields']['project'], 'example')
                self.assertTrue(session['fields']['session_id'])
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_bad_wheel_input_returns_values(self):
        import sc_agent
        import sc_treesitter
        with tempfile.TemporaryDirectory() as home:
            env = os.environ.copy()
            env.update(HOME=home, USERPROFILE=home, SC_PROJECT='example', SC_AGENT_TEST_REPO_ROOT=home)
            (Path(home) / 'config').mkdir()
            (Path(home) / 'config/default.json').write_text('{}')
            script = '''import sc_agent,sc_treesitter\nassert sc_agent.logger(42)["ok"] is False\nassert sc_agent.send(None, "jev", "example")["ok"] is False\nassert sc_agent.send_batch(None, "jev", "example")["ok"] is False\nassert sc_treesitter.run_query(None, "x", "x")["ok"] is False\n'''
            result = subprocess.run([sys.executable, '-c', script], env=env, text=True, capture_output=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_llm_schema_mismatch_is_retried(self):
        server = ThreadingHTTPServer(('127.0.0.1', 0), SchemaReply)
        server.requests = []
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as home:
                env = {'PATH': os.defpath, 'HOME': home, 'OPENAI_API_KEY': 'test-key',
                       'SC_AGENT_TEST_BASE_URL': f'http://127.0.0.1:{server.server_port}'}
                program = """import json, sc_agent
request = {'state': 'a', 'questions': {'q': {'type': 'choice', 'criteria': {'yes': 'yes', 'no': 'no'}}}}
result = sc_agent.send(json.dumps(request), 'luna', 'example', project='example')
assert result['ok'], result
assert result['value']['answers']['q']['probabilities'] == {'yes': 0.9, 'no': 0.1}
"""
                done = subprocess.run([sys.executable, '-c', program], env=env, text=True, capture_output=True, timeout=30)
                self.assertEqual(done.returncode, 0, done.stderr)
                self.assertEqual(len(server.requests), 2)
                self.assertEqual(server.requests[0], server.requests[1])
                self.assertEqual(server.requests[0]['response_format']['type'], 'json_schema')
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_question_hash(self):
        import hashlib
        import sc_agent
        expected = hashlib.sha256('question'.encode()).hexdigest()
        self.assertEqual(sc_agent.question_hash('question'), {'ok': True, 'value': expected})
        self.assertFalse(sc_agent.question_hash(None)['ok'])

    def test_all_loaded_grammars_and_invalid_sitr(self):
        with tempfile.TemporaryDirectory() as home:
            env = {'PATH': os.defpath, 'HOME': home, 'USERPROFILE': home}
            program = """import sc_treesitter
for language, source in [('python','x = 1'), ('rust','fn main() {}'), ('go','package p\\nfunc f() {}'), ('csharp','class C {}')]:
    result = sc_treesitter.run_query(language, '(identifier) @id', source)
    assert result['ok'] and result['value'], (language, result)
assert not sc_treesitter.run_query('unsupported', '(identifier) @id', 'x')['ok']
assert not sc_treesitter.run_query('python', 'invalid query', 'x')['ok']
"""
            result = subprocess.run([sys.executable, '-c', program], env=env, text=True, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_doctor_reports_winning_source(self):
        binary = Path(__file__).resolve().parents[2] / 'rewrite/target/debug/decidr'
        with tempfile.TemporaryDirectory() as home:
            local = Path(home) / '.sc'
            local.mkdir()
            path = local / 'decidr.json'
            path.write_text(json.dumps({'project': 'from-file', 'workers': 3}))
            env = {'PATH': os.defpath, 'HOME': home, 'SC_AGENT_TEST_REPO_ROOT': home}
            result = subprocess.run([str(binary), 'doctor'], env=env, text=True, capture_output=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            rows = json.loads(result.stdout)['value']
            self.assertEqual(rows['project'], {'value': 'from-file', 'source': str(path)})
            self.assertEqual(rows['workers'], {'value': '3', 'source': str(path)})

    def test_network_retry_without_sleep(self):
        root = Path(__file__).resolve().parents[2]
        result = subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'rewrite/Cargo.toml'), '-p', 'sc-agent', '--test', 'network_retry'], text=True, capture_output=True, timeout=90)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
