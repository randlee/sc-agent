"""Product contracts. Only loopback HTTP, controlled synchronization and temporary files."""
import importlib
import json
import os
import socket
from pathlib import Path
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from contextlib import contextmanager
from unittest.mock import patch
import unittest


REQUEST = {"state": "evidence", "questions": {"q": {"type": "choice", "instructions": "True?", "criteria": {"yes": "true", "no": "false"}}}}
ANSWER = {"model": "jev-1.14.0", "answers": {"q": {"type": "choice", "choice": "yes", "confidence": 0.73123456789, "probabilities": {"yes": 0.73123456789, "no": 0.26876543211}}}}


@contextmanager
def server(replies, respond=None):
    received = []
    lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            with lock:
                index = len(received)
                received.append((self.path, dict(self.headers), body))
            status, headers, answer = respond(body) if respond else replies[index]
            data = json.dumps(answer).encode()
            self.send_response(status)
            self.send_header("Content-Length", str(len(data)))
            for key, value in headers.items():
                self.send_header(key, value)
            self.end_headers()
            self.wfile.write(data)

    # handle_request has no polling loop or shutdown sleep.
    http = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    http.timeout = 10

    def serve():
        for _ in replies:
            http.handle_request()

    thread = threading.Thread(target=serve)
    thread.start()
    try:
        yield f"http://127.0.0.1:{http.server_port}/v1", received
    finally:
        thread.join(15)
        http.server_close()
        if thread.is_alive():
            raise AssertionError("contract server did not finish")


class Wheel(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.environment = patch.dict(os.environ, {"DECIDR_LOG_ROOT": self.directory.name})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    @classmethod
    def setUpClass(cls):
        cls.api = importlib.import_module("sc_agent")
        # A globally installed legacy wheel cannot supply rewrite evidence.
        if "rewrite" not in str(Path(cls.api.__file__).resolve()):
            raise AssertionError("contract requires the replacement wheel")

    def error(self, value):
        self.assertIsInstance(value, dict)
        self.assertIs(value.get("ok"), False)
        for field in ("id", "message", "action"):
            self.assertIsInstance(value["error"][field], str)
        return value["error"]

    def test_bad_inputs_return_values(self):
        for name, args in (("send", (None,)), ("send_batch", (None,)),
                           ("digest", (None,)), ("logger", (None,)),
                           ("log", (None, None)), ("category", (None,)),
                           ("retryable", (None,))):
            with self.subTest(name=name):
                self.error(getattr(self.api, name)(*args))

    def test_bad_provider_and_options(self):
        for options in ({"provider": "unknown"}, {"timeout_secs": 9.99},
                        {"timeout_secs": float("nan")}, {"model": "unknown"}):
            self.error(self.api.send(json.dumps(REQUEST), project="test", agent_role="contract", config_dir=self.directory.name, **options))
        for workers in (-1, 0, 65):
            self.error(self.api.send_batch([], workers=workers))

    def test_digest_contains_information_and_only_selected_question(self):
        got = self.api.digest(REQUEST, "q")
        self.assertTrue(got["ok"])
        changed = {**REQUEST, "state": "different"}
        self.assertNotEqual(got, self.api.digest(changed, "q"))
        with_other = {**REQUEST, "questions": {**REQUEST["questions"], "other": REQUEST["questions"]["q"]}}
        self.assertEqual(got, self.api.digest(with_other, "q"))

    def test_native_logger_shared_with_sitr(self):
        with tempfile.TemporaryDirectory() as folder:
            previous = os.environ.get("T_LOG_ROOT")
            os.environ["T_LOG_ROOT"] = folder
            try:
                logger = self.api.logger("t")["value"]
                self.assertTrue(self.api.log(logger, {"action": "python.own", "fields": {"source": "python"}})["ok"])
                self.error(self.api.send("not JSON", config_dir=folder, project="contract", agent_role="contract", general_log=logger))
                self.assertTrue(self.api.sitr.languages(general_log=logger)["ok"])
                self.assertTrue(self.api.flush(logger)["ok"])
                text = (Path(folder) / "logs/t.log.jsonl").read_text()
                self.assertIn("python.own", text)
                self.assertIn("sitr", text)
                self.assertIn("decision.request_parse", text)
                self.assertFalse((Path(folder) / "logs/decidr.log.jsonl").exists())
                self.assertFalse((Path(folder) / "logs/sitr.log.jsonl").exists())
            finally:
                if previous is None:
                    del os.environ["T_LOG_ROOT"]
                else:
                    os.environ["T_LOG_ROOT"] = previous

    def test_provider_wire_shapes(self):
        cases = (("typesafe", "jev", ANSWER, "/v1/systemone"),
                 ("anthropic", "haiku", {"content": [{"type": "text", "text": json.dumps(ANSWER)}]}, "/v1/messages"),
                 ("openai", "luna", {"choices": [{"message": {"content": json.dumps(ANSWER)}}]}, "/v1/chat/completions"),
                 ("openrouter", "haiku", {"choices": [{"message": {"content": json.dumps(ANSWER)}}]}, "/v1/chat/completions"),
                 ("together", "luna", {"choices": [{"message": {"content": json.dumps(ANSWER)}}]}, "/v1/chat/completions"),
                 ("nous", "custom-id", {"choices": [{"message": {"content": json.dumps(ANSWER)}}]}, "/v1/chat/completions"))
        for provider, model, answer, path in cases:
            with self.subTest(provider=provider), tempfile.TemporaryDirectory() as directory, server([(200, {}, answer)]) as (url, received):
                with patch.dict(os.environ, {"DECIDR_LOG_ROOT": directory, provider.upper() + "_API_KEY": "contract-secret"}):
                    result = self.api.send(json.dumps(REQUEST), model=model, provider=provider,
                                           project="contract", agent_role="contract", config_dir=directory, general_log=self.api.logger("decidr")["value"], base_url=url)
                self.assertTrue(result["ok"], result)
                self.assertEqual(result["value"]["answers"], ANSWER["answers"])
                self.assertEqual(received[0][0], path)
                body = received[0][2]
                expected = {"typesafe": "jev-1.14.0", "anthropic": "claude-haiku-5-5", "openai": "gpt-6-luna", "openrouter": "anthropic/claude-haiku-5.5", "together": "openai/gpt-6-luna", "nous": "custom-id"}
                self.assertEqual(body["model"], expected[provider])
                if provider == "anthropic":
                    self.assertIn("output_config", body)
                elif provider != "typesafe":
                    self.assertIn("response_format", body)
                self.assertNotIn("contract-secret", json.dumps(result))
                for log in Path(directory).rglob("*.jsonl"):
                    self.assertNotIn("contract-secret", log.read_text())

    def test_busy_retry_and_auth_quota_redaction(self):
        for status, body, expected in ((429, {"error": {"type": "insufficient_quota"}}, "http.quota_exhausted"),
                                       (401, {"error": "bad contract-secret"}, "decision.api_status")):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as directory, server([(status, {}, body)]) as (url, received), patch.dict(os.environ, {"DECIDR_LOG_ROOT": directory, "TYPESAFE_API_KEY": "contract-secret"}):
                result = self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract", config_dir=directory, general_log=self.api.logger("decidr")["value"], base_url=url)
                error = self.error(result)
                self.assertEqual(error["id"], expected)
                self.assertEqual(error["category"], "auth" if status == 401 else "quota")
                self.assertEqual(len(received), 1)
                self.assertNotIn("contract-secret", json.dumps(result))
        waits = []
        with tempfile.TemporaryDirectory() as directory, server([(429, {"Retry-After": "0.125"}, {"error": "busy"}), (200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {"DECIDR_LOG_ROOT": directory, "TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract", config_dir=directory, general_log=self.api.logger("decidr")["value"], base_url=url, sleep=waits.append)
            self.assertTrue(got["ok"], got)
            self.assertEqual(waits, [0.125])
            self.assertEqual(received[0][2], received[1][2])

    def test_batch_indices_and_finish_callbacks(self):
        first_started = threading.Event()
        release_first = threading.Event()
        callbacks = []

        def respond(body):
            if body["state"] == "0":
                first_started.set()
                if not release_first.wait(10):
                    return 500, {}, {"error": "first result was never released"}
            elif not first_started.wait(10):
                return 500, {}, {"error": "workers did not overlap"}
            return 200, {}, {**ANSWER, "marker": body["state"]}

        def completed(index, result):
            callbacks.append((index, result))
            if index == 1:
                release_first.set()

        with server([None, None], respond) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send_batch([json.dumps({**REQUEST, "state": str(i)}) for i in range(2)],
                                      workers=2, project="contract", agent_role="contract",
                                      config_dir=self.directory.name, base_url=url,
                                      general_log=self.api.logger("decidr")["value"], on_result=completed)
        self.assertTrue(got["ok"], got)
        self.assertEqual([item["index"] for item in got["value"]], [0, 1])
        self.assertEqual([item["result"]["value"]["marker"] for item in got["value"]], ["0", "1"])
        self.assertEqual([index for index, _ in callbacks], [1, 0])
        self.assertEqual([result["value"]["marker"] for _, result in callbacks], ["1", "0"])
        rows = [json.loads(line) for path in Path(self.directory.name).rglob("sessions.jsonl") for line in path.read_text().splitlines()]
        self.assertEqual(len(rows), 2)
        for row in rows:
            self.assertEqual(row["project"], "contract")
            self.assertEqual(row["agent_role"], "contract")
            self.assertIn(row["request"]["state"], ["0", "1"])
            self.assertEqual(len(row["session_id"]), 26)
            self.assertIn("q", row["question_digests"])

    def test_cancel_stops_new_requests_and_keeps_finished(self):
        cancel = self.api.Cancel()
        def completed(index, value):
            cancel.cancel()
        with server([(200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send_batch([json.dumps(REQUEST)] * 3, workers=1,
                                      project="contract", agent_role="contract", config_dir=self.directory.name,
                                      general_log=self.api.logger("decidr")["value"], base_url=url,
                                      cancel=cancel, on_result=completed)
        self.assertTrue(got["ok"], got)
        self.assertEqual(len(received), 1)
        self.assertEqual([item["index"] for item in got["value"]], [0])

    def test_network_errors_are_returned_and_attempts_bounded(self):
        waits = []
        with socket.socket() as reserved, patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            reserved.bind(("127.0.0.1", 0))  # Bound, never listening: no port-reuse race.
            url = f"http://127.0.0.1:{reserved.getsockname()[1]}/v1"
            got = self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract",
                                config_dir=self.directory.name, general_log=self.api.logger("decidr")["value"],
                                base_url=url, sleep=waits.append)
        error = self.error(got)
        self.assertEqual(error["id"], "http.network")
        self.assertEqual(error["attempts"], 5)
        self.assertEqual(len(waits), 4)


class Cli(unittest.TestCase):
    def test_send_and_doctor(self):
        binary = Path(__file__).resolve().parents[1] / "target/debug/decidr"
        with tempfile.TemporaryDirectory() as folder, server([(200, {}, ANSWER)]) as (url, received):
            port = int(url.split(":")[2].split("/")[0])
            config = Path(folder) / "decidr.json"
            config.write_text(json.dumps({"project": "contract", "workers": 2, "local_runners": {"laya": {"host": "127.0.0.1", "port": port, "model": "laya"}}}))
            env = {**os.environ, "DECIDR_LOG_ROOT": folder}
            doctor = subprocess.run([str(binary), "--config-dir", folder, "doctor"], env=env, cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertEqual(doctor.returncode, 0, doctor.stderr)
            self.assertFalse((Path(folder) / "logs").exists())
            project = next(row for row in json.loads(doctor.stdout)["value"]["settings"] if row["path"] == "project")
            self.assertEqual(project["source"], str(config))
            sent = subprocess.run([str(binary), "--config-dir", folder, "send", "--model", "laya"], input=json.dumps(REQUEST), env=env, cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertEqual(sent.returncode, 0, sent.stderr + sent.stdout)
            self.assertTrue(json.loads(sent.stdout)["ok"])
            self.assertEqual(received[0][0], "/v1/systemone")

    def test_models_and_sitr_queries(self):
        root = Path(__file__).resolve().parents[1]
        for binary, args in (("decidr", ["models"]), ("sitr", ["languages"]), ("sitr", ["queries", "--language", "rust"])):
            with self.subTest(binary=binary, args=args), tempfile.TemporaryDirectory() as folder:
                env = {**os.environ, f"{binary.upper()}_LOG_ROOT": folder}
                got = subprocess.run([str(root / "target/debug" / binary), *args],
                                     env=env, cwd=folder, text=True, capture_output=True, timeout=20)
                self.assertEqual(got.returncode, 0, got.stderr)
                self.assertTrue(json.loads(got.stdout)["ok"])


if __name__ == "__main__":
    unittest.main()
