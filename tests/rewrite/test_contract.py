"""Product-boundary checks against a real loopback server and isolated home."""
import importlib
import json
import os
import subprocess
import sys
from pathlib import Path
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch

REQUEST = {"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}}
ANSWER = {"model": "test-model", "answers": {"q": {"type": "noul", "noul": 0.8}}}


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.server.requests.append((self.path, dict(self.headers), json.loads(body)))
        status, headers, response = self.server.replies.pop(0) if self.server.replies else (200, {}, self.server.response)
        data = json.dumps(response).encode()
        self.send_response(status)
        for key, value in headers.items():
            self.send_header(key, value)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_args):
        pass


class Contract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.agent = importlib.import_module("sc_agent")
        cls.sitr = importlib.import_module("sc_treesitter")

    def setUp(self):
        self.home = tempfile.TemporaryDirectory()
        config = Path(self.home.name) / "repo" / "config"
        config.mkdir(parents=True)
        (config / "default.json").write_text("{}")
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.server.requests = []
        self.server.replies = []
        self.server.response = ANSWER
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        path = os.environ.get("PATH", os.defpath)
        self.env = patch.dict(os.environ, {
            "PATH": path,
            "HOME": self.home.name, "USERPROFILE": self.home.name,
            "DECIDR__PROJECT": "contract", "TYPESAFE_API_KEY": "test-secret-never-log",
            "SC_AGENT_TEST_REPO_ROOT": str(config.parent),
            "SC_AGENT_TEST_BASE_URL": f"http://127.0.0.1:{self.server.server_port}",
        }, clear=True)
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.home.cleanup()

    def send(self, **kwargs):
        return self.agent.send(json.dumps(REQUEST), model="jev", agent_role="contract", **kwargs)

    def test_send_uses_real_loopback_and_redacts_credentials(self):
        result = self.send()
        self.assertTrue(result["ok"], result)
        self.assertEqual(len(self.server.requests), 1)
        path, headers, body = self.server.requests[0]
        self.assertIn("systemone", path.lower())
        self.assertEqual(headers.get("Authorization"), "Bearer test-secret-never-log")
        self.assertIn("questions", body)
        self.assertNotIn("test-secret-never-log", json.dumps(result))

    def test_retry_after_and_quota(self):
        self.server.replies = [(429, {"Retry-After": "0"}, {"error": "busy"}), (200, {}, ANSWER)]
        result = self.send()
        self.assertTrue(result["ok"], result)
        self.assertEqual(len(self.server.requests), 2)
        self.server.requests.clear()
        self.server.replies = [(429, {}, {"error": "quota exhausted"})]
        result = self.send()
        self.assertFalse(result["ok"], result)
        self.assertEqual(len(self.server.requests), 1)

    def test_auth_and_bad_input_return_errors_without_raising(self):
        self.server.replies = [(401, {}, {"error": "unauthorized"})]
        result = self.send()
        self.assertFalse(result["ok"], result)
        self.assertTrue(result["error"]["id"])
        self.assertNotIn("test-secret-never-log", json.dumps(result))
        result = self.agent.send("not json", model="jev", agent_role="contract", project="contract")
        self.assertFalse(result["ok"], result)
        self.assertTrue(result["error"]["id"])

    def test_batch_matches_every_input_by_index(self):
        inputs = [json.dumps(REQUEST), json.dumps(REQUEST)]
        result = self.agent.send_batch(inputs, model="jev", agent_role="contract", project="contract", workers=2)
        self.assertTrue(result["ok"], result)
        self.assertEqual(sorted(row["position"] for row in result["value"]), [0, 1])
        self.assertEqual(len(self.server.requests), 2)

    def test_missing_project_reports_sources_and_sends_nothing(self):
        os.environ.pop("DECIDR__PROJECT", None)
        result = self.send()
        self.assertFalse(result["ok"], result)
        text = json.dumps(result)
        for source in ("--project", "DECIDR__PROJECT", "default.json", "local.json"):
            self.assertIn(source, text)
        self.assertEqual(self.server.requests, [])

    def test_each_provider_uses_its_request_shape(self):
        providers = [
            ("typesafe", "jev", "TYPESAFE_API_KEY", "/systemone"),
            ("anthropic", "haiku", "ANTHROPIC_API_KEY", "/messages"),
            ("openai", "luna", "OPENAI_API_KEY", "/chat/completions"),
            ("openrouter", "haiku", "OPENROUTER_API_KEY", "/chat/completions"),
            ("together", "luna", "TOGETHER_API_KEY", "/chat/completions"),
            ("nous", "custom-model", "NOUS_API_KEY", "/chat/completions"),
        ]
        for provider, model, key, path in providers:
            with self.subTest(provider=provider):
                self.server.requests.clear()
                os.environ[key] = "test-secret-never-log"
                if provider == "anthropic":
                    self.server.response = {"content": [{"type": "text", "text": json.dumps(ANSWER)}]}
                elif provider != "typesafe":
                    self.server.response = {"choices": [{"message": {"content": json.dumps(ANSWER)}}]}
                else:
                    self.server.response = ANSWER
                result = self.agent.send(json.dumps(REQUEST), model=model, provider=provider,
                                         agent_role="contract", project="contract")
                self.assertTrue(result["ok"], result)
                self.assertEqual(len(self.server.requests), 1)
                self.assertTrue(self.server.requests[0][0].endswith(path), self.server.requests)
                if provider == "nous":
                    self.assertEqual(self.server.requests[0][2]["model"], "custom-model")
                self.assertNotIn("test-secret-never-log", json.dumps(result))

    def test_key_priority_env_dotenv_then_keyring_double(self):
        root = Path(self.home.name) / "repo"
        root.mkdir()
        (root / ".env").write_text("TYPESAFE_API_KEY=dotenv-secret\n")
        (root / "config").mkdir()
        (root / "config" / "default.json").write_text("{}")
        os.environ["SC_AGENT_TEST_REPO_ROOT"] = str(root)
        os.environ["SC_AGENT_TEST_KEYRING_VALUE"] = "keyring-secret"
        for expected in ("test-secret-never-log", "dotenv-secret", "keyring-secret"):
            self.server.requests.clear()
            result = self.send()
            self.assertTrue(result["ok"], result)
            self.assertEqual(self.server.requests[0][1].get("Authorization"), f"Bearer {expected}")
            self.assertNotIn(expected, json.dumps(result))
            if expected == "test-secret-never-log":
                os.environ.pop("TYPESAFE_API_KEY")
            elif expected == "dotenv-secret":
                (root / ".env").unlink()

    def test_cli_commands(self):
        target = Path(__file__).resolve().parents[2] / "rewrite" / "target" / "debug"
        request = Path(self.home.name) / "request.json"
        request.write_text(json.dumps(REQUEST))
        for binary, args in (("decidr", ["models"]), ("decidr", ["doctor"]),
                             ("decidr", ["send", "--model", "jev", "--project", "contract", "--agent-role", "contract", str(request)]),
                             ("sitr", ["languages"]), ("sitr", ["queries", "python"])):
            with self.subTest(binary=binary, args=args):
                completed = subprocess.run([str(target / (binary + (".exe" if os.name == "nt" else ""))), *args], text=True,
                                           capture_output=True, timeout=15, check=False)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertNotIn("test-secret-never-log", completed.stdout + completed.stderr)

    def test_sitr_alone_uses_its_default_file(self):
        program = """
import sc_treesitter
result = sc_treesitter.languages()
assert result["ok"], result
"""
        completed = subprocess.run([sys.executable, "-c", program], text=True,
                                   capture_output=True, timeout=30, check=False)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        root = Path(self.home.name) / ".sc-agent" / "logs"
        self.assertTrue((root / "sitr.log.jsonl").is_file())
        self.assertFalse((root / "decidr.log.jsonl").exists())

    def test_two_wheels_share_one_rust_owned_logger(self):
        program = """
import json
import sc_agent, sc_treesitter
request = %r
opened = sc_agent.logger("contract")
assert opened["ok"], opened
handle = opened["value"]
assert callable(handle)
sent = sc_agent.send(json.dumps(request), model="jev", agent_role="contract", project="contract", general_log=handle)
assert sent["ok"], sent
query = sc_treesitter.run_query("python", "(identifier) @id", "x = 1", general_log=handle)
assert query["ok"], query
""" % REQUEST
        completed = subprocess.run([sys.executable, "-c", program], text=True,
                                   capture_output=True, timeout=30, check=False)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        log = Path(self.home.name) / ".sc-agent" / "logs" / "contract.log.jsonl"
        self.assertTrue(log.is_file(), log)
        rows = [json.loads(line) for line in log.read_text().splitlines()]
        actions = [str(row.get("action", "")) for row in rows]
        self.assertTrue(any(action.startswith("decidr.") for action in actions), actions)
        self.assertTrue(any(action.startswith("sitr.") for action in actions), actions)
        self.assertFalse((log.parent / "decidr.log.jsonl").exists())
        self.assertFalse((log.parent / "sitr.log.jsonl").exists())
        self.assertNotIn("test-secret-never-log", log.read_text())
        sessions = list((Path(self.home.name) / ".sc-agent" / "sessions").rglob("sessions.jsonl"))
        self.assertEqual(len(sessions), 1)
