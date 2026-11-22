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
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch

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
                else:
                    status, result = 200, ANSWER
                encoded = json.dumps(result).encode()
                handler.send_response(status)
                handler.send_header("Content-Length", str(len(encoded)))
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
    spec.loader.exec_module(module)
    return module


def allow_local_runner(client_name, repository_root):
    module_name = client_name.replace("-", "_") + "_settings"
    settings = types.ModuleType(module_name)
    settings.find_repo_root = lambda path: types.SimpleNamespace(ok=True, value=Path(path))
    settings.remote_judge_consent = lambda _path: types.SimpleNamespace(ok=True, value=True)
    return patch.dict(sys.modules, {module_name: settings})


class RenderedWheelLoopback(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        package = importlib.resources.files("sc_agent")
        for name in ("__init__.pyi", "_native.pyi", "py.typed", "templates/consumer.py.j2"):
            if not (package / name).is_file():
                raise AssertionError(f"built wheel is missing sc_agent/{name}")

    def config(self, root, address):
        host, port = address
        config = {"project": "phase-i-loopback", "laya": {"host": host, "port": port, "model": "loopback"}}
        (root / "decidr.json").write_text(json.dumps(config))

    def load_clients(self, root):
        return [rendered(path) for path in render_consumers(root)]

    def call_options(self, client, root, logger):
        return {
            "model": "laya",
            "project": "phase-i-loopback",
            "repository_root": str(root),
            "config_dir": str(root),
            "general_log": logger,
            "timeout_secs": 5,
        }

    def test_rendered_clients_run_against_the_installed_wheel(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            clients = self.load_clients(root)
            for client in clients:
                with self.subTest(client=client.CLIENT_NAME), tempfile.TemporaryDirectory() as work:
                    root = Path(work)
                    with Runner(2) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(
                        os.environ,
                        {
                            "DECIDR_LOG_ROOT": str(root),
                            "CONSUMER_TEST_LOG_ROOT": str(root),
                        },
                    ):
                        self.config(root, runner.address)
                        logger = sc_agent.logger("consumer-test")
                        self.assertTrue(logger["ok"], logger)
                        options = self.call_options(client, root, logger["value"])
                        success = client.call_result(REQUEST, **options)
                        self.assertTrue(success["ok"], success)
                        self.assertEqual(success["value"]["q"].noul, 0.75)
                        failure = client.call_result(
                            {**REQUEST, "state": "unauthorized"},
                            category_codes={"auth": "TEST.AUTH"},
                            **options,
                        )
                        self.assertFalse(failure["ok"])
                        self.assertEqual(failure["error"].code, "TEST.AUTH")
                        self.assertEqual(
                            runner.received,
                            [("/v1/systemone", {**REQUEST, "model": "loopback"}),
                             ("/v1/systemone", {**REQUEST, "state": "unauthorized", "model": "loopback"})],
                            "loopback runner received unexpected requests",
                        )
                        self.assertEqual(callable(getattr(client, "extract", None)), client.CLIENT_NAME == "judge-jev")
                        self.assertTrue(sc_agent.flush(logger["value"])["ok"])

                    rows = [
                        json.loads(line)
                        for path in (root / "sessions").rglob("sessions.jsonl")
                        for line in path.read_text().splitlines()
                    ]
                    self.assertEqual(len(rows), 2)
                    for row in rows:
                        self.assertEqual((row["project"], row["agent_role"]), ("phase-i-loopback", client.CLIENT_NAME))
                    self.assertEqual({row["request"]["state"] for row in rows}, {"ok", "unauthorized"})
                    log = (root / "logs" / "consumer-test.log.jsonl").read_text()
                    self.assertIn("decision.result", log)
                    self.assertIn("http.auth", log)

    def test_keyboard_interrupt_cancels_the_passed_cancel(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[0]
            with Runner(1) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(
                os.environ,
                {
                    "DECIDR_LOG_ROOT": str(root),
                    "CONSUMER_TEST_LOG_ROOT": str(root),
                },
            ):
                self.config(root, runner.address)
                logger = sc_agent.logger("consumer-test")
                self.assertTrue(logger["ok"], logger)
                cancel = sc_agent.Cancel()

                def interrupt_after_first_result(_result):
                    raise KeyboardInterrupt

                batch = client.call_batch_results(
                    [REQUEST] * 3,
                    cancel=cancel,
                    workers=1,
                    on_result=interrupt_after_first_result,
                    **self.call_options(client, root, logger["value"]),
                )
                self.assertEqual(
                    runner.received,
                    [("/v1/systemone", {**REQUEST, "model": "loopback"})],
                    "cancelled batch sent a surplus loopback request",
                )
                self.assertTrue(batch["ok"], batch)
                self.assertEqual(len(batch["value"]), 3)
                self.assertTrue(batch["value"][0]["ok"], batch["value"][0])
                self.assertEqual([item["error"].code for item in batch["value"][1:]], ["CANCELED", "CANCELED"])

                later = client.call_result(REQUEST, cancel=cancel, **self.call_options(client, root, logger["value"]))
                self.assertFalse(later["ok"])
                self.assertEqual(later["error"].code, "CANCELED")
                self.assertTrue(sc_agent.flush(logger["value"])["ok"])

    def test_wheel_keyboard_interrupt_reaches_each_consumer_cancel_handler(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[0]
            with Runner(0) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(
                os.environ,
                {
                    "DECIDR_LOG_ROOT": str(root),
                    "CONSUMER_TEST_LOG_ROOT": str(root),
                },
            ):
                self.config(root, runner.address)
                logger = sc_agent.logger("consumer-test")
                self.assertTrue(logger["ok"], logger)
                options = self.call_options(client, root, logger["value"])

                for method, native_call in ((client.call_result, "send"),
                                            (client.call_batch_results, "send_batch")):
                    with self.subTest(native_call=native_call):
                        cancel = sc_agent.Cancel()
                        with patch.object(sc_agent, native_call, side_effect=KeyboardInterrupt):
                            if native_call == "send":
                                result = method(REQUEST, cancel=cancel, **options)
                                self.assertFalse(result["ok"], result)
                                self.assertEqual(result["error"].code, "CANCELED")
                            else:
                                result = method([REQUEST], cancel=cancel, **options)
                                self.assertEqual(result, {"ok": True, "value": []})

                        probe = sc_agent.send(
                            REQUEST,
                            model="laya",
                            project="phase-i-loopback",
                            agent_role=client.CLIENT_NAME,
                            config_dir=str(root),
                            general_log=logger["value"],
                            timeout_secs=5,
                            cancel=cancel,
                        )
                        self.assertFalse(probe["ok"], probe)
                        self.assertEqual(probe["error"]["id"], "decision.cancelled")

                self.assertEqual(runner.received, [], "KeyboardInterrupt handlers sent unexpected requests")
                self.assertTrue(sc_agent.flush(logger["value"])["ok"])


if __name__ == "__main__":
    unittest.main()
