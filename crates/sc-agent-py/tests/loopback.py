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


class UnexpectedSitrAccess:
    def __getattr__(self, name):
        raise AssertionError(f"sitr=false called sc_agent.sitr.{name}")


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

    def test_client_default_and_injected_logger_owns_every_call(self):
        with tempfile.TemporaryDirectory() as temporary:
            for injected in (False, True):
                clients = self.load_clients(Path(temporary))
                for client in clients:
                    with self.subTest(client=client.CLIENT_NAME, injected=injected), tempfile.TemporaryDirectory() as work:
                        root = Path(work)
                        env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                               "DECIDR_LOG_ROOT": str(root / "wrong-root"), "SITR_LOG_ROOT": str(root / "wrong-root"),
                               "GIT_CEILING_DIRECTORIES": str(root.parent.resolve()),
                               "PLAN_ANCHOR_LOG_ROOT": work, "JUDGE_JEV_LOG_ROOT": work, "CONSUMER_TEST_LOG_ROOT": work}
                        subprocess.run(["git", "init", "--quiet", str(root)], check=True, timeout=20, env=env)
                        subprocess.run(["git", "-C", str(root), "remote", "add", "origin", "https://example.invalid/test/client.git"], check=True, timeout=20, env=env)
                        with Runner(1) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(os.environ, env, clear=True), patch.object(sc_agent, "logger", wraps=sc_agent.logger) as create, chdir(root):
                            self.config(root, runner.address)
                            handle = sc_agent.logger("consumer-test")["value"] if injected else None
                            result = client.call_result(REQUEST, **self.call_options(client, root, handle))
                            self.assertTrue(result["ok"], result)
                            if client.CLIENT_NAME == "judge-jev":
                                self.assertTrue(client.extract("python", "def f(): pass", "(function_definition) @f", general_log=handle)["ok"])
                                failed = client.extract("missing-language", "", "", general_log=handle)
                                self.assertFalse(failed["ok"])
                            self.assertEqual(create.call_count, 1)
                            self.assertEqual(create.call_args.args, ("consumer-test" if injected else client.CLIENT_NAME,))
                            owned = handle if injected else client._logger_handle
                            self.assertTrue(sc_agent.flush(owned)["ok"])
                        name = "consumer-test" if injected else client.CLIENT_NAME
                        files = list((root / "logs").glob("*.jsonl"))
                        self.assertEqual([file.name for file in files], [name + ".log.jsonl"])
                        events = [json.loads(line) for line in files[0].read_text().splitlines()]
                        self.assertTrue(any(event["action"] == "decision.result" for event in events), events)
                        if client.CLIENT_NAME == "judge-jev":
                            self.assertTrue(any(event["action"] == "sitr.query" for event in events), events)
                            self.assertEqual(sum(event["fields"].get("id") == failed["error"]["id"] and "missing-language" in event["fields"].get("message", "") for event in events), 1)
                        expected = root / "sessions/example.invalid/test/client" / client.CLIENT_NAME / "phase-i-loopback/sessions.jsonl"
                        self.assertEqual(list((root / "sessions").rglob("sessions.jsonl")), [expected])
                        self.assertFalse((root / "wrong-root").exists())
                        rows = [json.loads(line) for line in expected.read_text().splitlines()]
                        self.assertEqual(len(rows), 1)
                        self.assertEqual(rows[0]["agent_role"], client.CLIENT_NAME)

    def test_rendered_modules_only_reference_the_permitted_api(self):
        allowed = {"send", "send_batch", "digest", "logger", "log", "flush", "Cancel", "sitr"}
        with tempfile.TemporaryDirectory() as folder:
            for path in render_consumers(Path(folder)):
                refs = {node.attr for node in ast.walk(ast.parse(path.read_text()))
                        if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == "sc_agent"}
                self.assertTrue(refs)
                self.assertLessEqual(refs, allowed)
                self.assertEqual("sitr" in refs, path.stem == "judge_jev_client")

    def test_logger_initialization_is_serialized(self):
        with tempfile.TemporaryDirectory() as folder:
            for client in self.load_clients(Path(folder)):
                entered, contended, release = threading.Event(), threading.Event(), threading.Event()
                mutex = threading.Lock()
                calls, results = [], []
                class Guard:
                    def __enter__(self):
                        if entered.is_set():
                            contended.set()
                        mutex.acquire()
                    def __exit__(self, *_):
                        mutex.release()
                def create(name):
                    calls.append(name)
                    entered.set()
                    if not release.wait(5):
                        return {"ok": False, "error": "test release absent"}
                    return {"ok": True, "value": object()}
                client._logger_lock = Guard()
                threads = [threading.Thread(target=lambda: results.append(client._logger(create, None))) for _ in range(2)]
                try:
                    threads[0].start()
                    self.assertTrue(entered.wait(5), "factory not reached")
                    threads[1].start()
                    self.assertTrue(contended.wait(5), "second caller did not take the lock")
                finally:
                    release.set()
                    for thread in threads:
                        if thread.ident is not None:
                            thread.join(5)
                            self.assertFalse(thread.is_alive())
                self.assertEqual(calls, [client.CLIENT_NAME])
                self.assertEqual(len(results), 2)
                self.assertIs(results[0]["value"], results[1]["value"])

    def test_rendered_clients_run_against_the_installed_wheel(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            clients = self.load_clients(root)
            for client in clients:
                with self.subTest(client=client.CLIENT_NAME), tempfile.TemporaryDirectory() as work:
                    root = Path(work)
                    sitr_guard = (patch.object(sc_agent, "sitr", UnexpectedSitrAccess())
                                  if client.CLIENT_NAME == "plan-anchor" else nullcontext())
                    with Runner(2) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(
                        os.environ,
                        {
                            "DECIDR_LOG_ROOT": str(root),
                            "CONSUMER_TEST_LOG_ROOT": str(root),
                            "PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                            "GIT_CEILING_DIRECTORIES": str(root.parent.resolve()),
                        },
                        clear=True), sitr_guard, chdir(root):
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

    def test_sitr_extract_reaches_offline_request_prompt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[1]
            self.assertTrue(callable(getattr(client, "extract", None)))
            source_path = Path(__file__).resolve().parent / "fixtures" / "sitr_source.py"
            source = source_path.read_text()
            with Runner(1) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(
                os.environ,
                {
                    "DECIDR_LOG_ROOT": str(root),
                    "CONSUMER_TEST_LOG_ROOT": str(root),
                    "PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_CEILING_DIRECTORIES": str(root.parent.resolve()),
                },
                clear=True), chdir(root):
                self.config(root, runner.address)
                logger = sc_agent.logger("consumer-test")
                self.assertTrue(logger["ok"], logger)
                extracted = client.extract(
                    "python",
                    source,
                    "(function_definition name: (identifier) @name)",
                    general_log=logger["value"],
                )
                self.assertTrue(extracted["ok"], extracted)
                names = [capture["text"] for capture in extracted["value"]]
                self.assertEqual(set(names), {"validate_claim", "summarize_claim"})
                prompt = "Extracted functions: " + ", ".join(names)
                request = {
                    "state": "offline-dry-run",
                    "questions": {
                        "q": {"type": "noul", "instructions": prompt, "criteria": ["score"]},
                    },
                }
                # The local runner is the dry-run decision endpoint; no remote provider is contacted.
                result = client.call_result(request, **self.call_options(client, root, logger["value"]))
                self.assertTrue(result["ok"], result)
                self.assertEqual(
                    runner.received,
                    [("/v1/systemone", {**request, "model": "loopback"})],
                    "extracted sitr text did not reach the offline prompt",
                )
                received_prompt = runner.received[0][1]["questions"]["q"]["instructions"]
                for name in names:
                    self.assertIn(name, received_prompt, f"extracted name {name!r} is missing from the sent prompt")
                self.assertTrue(sc_agent.flush(logger["value"])["ok"])

    def test_keyboard_interrupt_cancels_the_passed_cancel(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            client = self.load_clients(root)[0]
            with Runner(1) as runner, allow_local_runner(client.CLIENT_NAME, root), patch.dict(
                os.environ,
                {
                    "DECIDR_LOG_ROOT": str(root),
                    "CONSUMER_TEST_LOG_ROOT": str(root),
                    "PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_CEILING_DIRECTORIES": str(root.parent.resolve()),
                },
                clear=True), chdir(root):
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
                    "PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_CEILING_DIRECTORIES": str(root.parent.resolve()),
                },
                clear=True), chdir(root):
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
