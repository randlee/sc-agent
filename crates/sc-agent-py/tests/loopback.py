"""Run rendered consumer modules against the just-installed sc-decidr wheel."""
import importlib.resources
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import types
import subprocess
import ast
import unittest
from contextlib import nullcontext, chdir
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import MagicMock, patch

import sc_agent

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "scripts"))
from check_consumer_template import render_consumers


REQUEST = {"state": "ok", "questions": {"q": {"type": "noul", "instructions": "score", "criteria": ["score"]}}}
ANSWER = {"model": "loopback", "answers": {"q": {"type": "noul", "noul": 0.75}}}


class Runner:
    def __init__(self, expected_count):
        self.expected_count = expected_count
        self.received = []
        self.lock = threading.Lock()

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(handler):
                body = json.loads(handler.rfile.read(int(handler.headers["Content-Length"])))
                with self.lock:
                    ordinal = len(self.received)
                    self.received.append((handler.path, body))
                if ordinal >= self.expected_count:
                    status, result = 500, {"error": "unexpected surplus request"}
                elif body.get("state") == "unauthorized":
                    status, result = 401, {"error": "controlled auth refusal"}
                elif body.get("state") == "busy":
                    status, result = 429, {"error": "controlled busy"}
                else:
                    status, result = 200, ANSWER
                encoded = json.dumps(result).encode()
                handler.send_response(status)
                handler.send_header("Content-Length", str(len(encoded)))
                if status == 429:
                    handler.send_header("Retry-After", "0")
                handler.end_headers()
                handler.wfile.write(encoded)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever)

    @property
    def address(self):
        return self.server.server_address

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.server.shutdown()
        self.thread.join(5)
        self.server.server_close()


def rendered(path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load rendered consumer {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[path.stem] = module
    spec.loader.exec_module(module)
    return module


def allow_local_runner(client_name, repository_root):
    module_name = client_name.replace("-", "_") + "_settings"
    settings = types.ModuleType(module_name)
    settings.find_repo_root = lambda path: types.SimpleNamespace(ok=True, value=Path(path))
    settings.remote_judge_consent = lambda _path: types.SimpleNamespace(ok=True, value=True)
    return patch.dict(sys.modules, {module_name: settings})


class UnexpectedSitrAccess:
    def __getattr__(self, name):
        raise AssertionError(f"sitr=false called sc_agent.sitr.{name}")


class RenderedWheelLoopback(unittest.TestCase):
    def load_clients(self, root):
        output = root / "rendered"
        output.mkdir(exist_ok=True)
        return [rendered(path) for path in render_consumers(output)]

    def test_rendered_clients_run_against_the_installed_wheel(self):
        with tempfile.TemporaryDirectory() as temporary:
            for client in self.load_clients(Path(temporary)):
                with self.subTest(client=client.CLIENT_NAME), tempfile.TemporaryDirectory() as work:
                    root = Path(work)
                    env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                           "PLAN_ANCHOR_LOG_ROOT": work, "JUDGE_JEV_LOG_ROOT": work}
                    subprocess.run(["git", "init", "--quiet", str(root)], check=True, timeout=20, env=env)
                    with Runner(2) as runner, patch.dict(os.environ, env, clear=True), chdir(root):
                        host, port = runner.address
                        (root / "sc-agent.json").write_text(json.dumps({"project": "loopback", "laya": {"host": host, "port": port}}))
                        opened = client.open_run(model="laya", project="loopback", config_dir=str(root), cache=False)
                        self.assertTrue(opened.ok)
                        with opened.value as run:
                            success = client.call_result(run, REQUEST)
                            self.assertTrue(success.ok, success)
                            self.assertEqual(success.value.answers["q"].noul, .75)
                            failure = client.call_result(run, {**REQUEST, "state": "unauthorized"}, category_codes={"auth": "TEST.AUTH"})
                            self.assertFalse(failure.ok)
                            self.assertEqual(failure.error.code, "TEST.AUTH")
                        self.assertEqual(len(runner.received), 2)
                        rows = [json.loads(line)["fields"] for path in (root / "sessions").rglob("sessions.jsonl") for line in path.read_text().splitlines()]
                        self.assertEqual(len(rows), 1)
                        self.assertEqual(rows[0]["agent_role"], client.CLIENT_NAME)
                        log = (root / "logs" / f"{client.CLIENT_NAME}.log.jsonl").read_text()
                        self.assertIn("decision.result", log)
                        self.assertIn("http.auth", log)

    def test_consent_only_for_typesafe(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[0]
            fake = MagicMock()
            fake.sends_to.return_value = {"ok": True, "value": True}
            module_name = client.CLIENT_NAME.replace("-", "_") + "_settings"
            settings = types.ModuleType(module_name)
            settings.find_repo_root = lambda path: types.SimpleNamespace(ok=True, value=path)
            settings.remote_judge_consent = lambda path: types.SimpleNamespace(ok=True, value=False)
            with patch.dict(sys.modules, {module_name: settings}):
                denied = client.call_result(fake, REQUEST)
            self.assertEqual(denied.error.code, "POLICY.REMOTE_JUDGE_DISABLED")
            fake.send.assert_not_called()

    def test_keyboard_interrupt_cancels_the_run(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[0]
            env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1", "PLAN_ANCHOR_LOG_ROOT": str(root)}
            with Runner(1) as runner, patch.dict(os.environ, env, clear=True), chdir(root):
                host, port = runner.address
                (root / "sc-agent.json").write_text(json.dumps({"project": "loopback", "laya": {"host": host, "port": port}}))
                def interrupt(_delay):
                    raise KeyboardInterrupt
                opened = client.open_run(model="laya", project="loopback", config_dir=str(root), workers=1,
                                         cache=False, seams={"sleep": interrupt})
                with opened.value as run:
                    result = client.call_batch_results(run, [{**REQUEST, "state": "busy"}, REQUEST])
                self.assertTrue(result.ok)
                self.assertEqual([entry.error.code for entry in result.value], ["CANCELED", "CANCELED"])
                self.assertEqual(len(runner.received), 1)

    def test_on_line_error_stops_further_sends(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[0]
            env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1", "PLAN_ANCHOR_LOG_ROOT": str(root)}
            jobs = [client.BatchJob("item", None, str(i), 1, str(i), REQUEST, {}, {}, "directive") for i in range(3)]
            def refuse(_line, _answers):
                raise client.ClientError("LOG.WRITE", "disk full")
            with Runner(3) as runner, patch.dict(os.environ, env, clear=True), chdir(root):
                host, port = runner.address
                (root / "sc-agent.json").write_text(json.dumps({"project": "loopback", "laya": {"host": host, "port": port}}))
                opened = client.open_run(model="laya", project="loopback", config_dir=str(root), workers=1, cache=False)
                with opened.value as run:
                    result = client.run_batch(run, jobs, client.BatchSettings("plan", 1, "p", .6, False), on_line=refuse)
                self.assertFalse(result.ok)
                self.assertEqual((result.error.code, result.error.message), ("LOG.WRITE", "disk full"))
                self.assertEqual(len(runner.received), 1)

    def test_sitr_extract_logs_under_the_tool(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[1]
            with patch.dict(os.environ, {"JUDGE_JEV_LOG_ROOT": str(root)}):
                result = client.extract("python", "def f(): pass", "(function_definition name: (identifier) @name)")
                self.assertTrue(result.ok, result)
                self.assertEqual(result.value[0]["text"], "f")
            self.assertTrue((root / "logs" / "judge-jev.log.jsonl").exists())

    def test_fakerun_conforms_to_real_run(self):
        from sc_agent.testing import FakeRun
        fake = FakeRun({"provider": "laya", "run_id": "test", "envelopes": []})
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {"PLAN_ANCHOR_LOG_ROOT": temporary}):
            with sc_agent.Run(tool="plan-anchor", tool_version="test", agent_role="plan-anchor", project="test") as real:
                fake_cancel = fake.cancel()
                real_cancel = real.cancel()
                self.assertEqual(set(fake_cancel), set(real_cancel))
                self.assertEqual(set(fake.send({})), set(real.send(json.dumps(REQUEST))))
                self.assertEqual(fake.run_id, "test")
                self.assertIsInstance(real.run_id, str)

    def test_rendered_modules_only_reference_the_permitted_api(self):
        with tempfile.TemporaryDirectory() as folder:
            for path in render_consumers(Path(folder)):
                source = path.read_text()
                for forbidden in ("import hashlib", "import threading", "sc_agent.testing", "general_log=logger"):
                    self.assertNotIn(forbidden, source)
                ast.parse(source)


if __name__ == "__main__":
    unittest.main()
