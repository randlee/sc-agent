"""Product contracts. Only loopback HTTP, controlled synchronization and temporary files."""
import importlib
import json
import os
import socket
import sys
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
                received.append((self.path, {key.lower(): value for key, value in self.headers.items()}, body))
            status, headers, answer = respond(body) if respond else replies[index]
            if status is None:
                self.close_connection = True
                self.connection.shutdown(socket.SHUT_RDWR)
                return
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


class Isolated(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        previous = os.getcwd()
        os.chdir(self.directory.name)
        self.addCleanup(os.chdir, previous)
        environment = {"PATH": os.defpath, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
                       "DECIDR_LOG_ROOT": self.directory.name, "SITR_LOG_ROOT": self.directory.name,
                       "SC_LOG_ROOT": self.directory.name, "SC_COMPOSE_LOG_ROOT": self.directory.name}
        self.environment = patch.dict(os.environ, environment, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)


class Wheel(Isolated):
    @classmethod
    def setUpClass(cls):
        cls.api = importlib.import_module("sc_agent")

    def error(self, value):
        self.assertIsInstance(value, dict)
        self.assertIs(value.get("ok"), False)
        for field in ("id", "message", "action"):
            self.assertIsInstance(value["error"][field], str)
        return value["error"]

    def test_default_logs_and_option_errors_once(self):
        code = r'''
import json, sys
from pathlib import Path
import sc_agent
root = Path(sys.argv[1])
send = lambda: sc_agent.send("not JSON", project="test", agent_role="test", config_dir=str(root))
calls = [send, sc_agent.sitr.languages]
if sys.argv[2] == "sitr-first": calls.reverse()
for call in calls: call()
sc_agent.send("{}", timeout_secs=float("nan"), agent_role="test", config_dir=str(root))
sc_agent.send_batch([], workers=0, agent_role="test", config_dir=str(root))
for name in ("decidr", "sitr"):
    assert (root / "logs" / (name + ".log.jsonl")).is_file()
rows = [json.loads(line) for line in (root / "logs/decidr.log.jsonl").read_text().splitlines()]
for identity in ("decision.request_parse", "decidr.invalid_request_timeout", "decidr.invalid_workers"):
    assert sum(row["fields"].get("id") == identity for row in rows) == 1, rows
'''
        for order in ("decidr-first", "sitr-first"):
            with self.subTest(order=order), tempfile.TemporaryDirectory() as folder:
                env = {"PATH": os.defpath, "DECIDR_LOG_ROOT": folder, "SITR_LOG_ROOT": folder}
                got = subprocess.run([sys.executable, "-c", code, folder, order], env=env, cwd=folder, text=True, capture_output=True, timeout=20)
                self.assertEqual(got.returncode, 0, got.stderr)

    def test_key_precedence_and_redaction_through_send(self):
        keyring_calls = []
        def keyring(name):
            keyring_calls.append(name)
            return "keyring-key"
        logger = self.api.logger("decidr")["value"]
        dotenv = Path.cwd() / ".env"
        dotenv.write_text("TYPESAFE_API_KEY=dotenv-key\n")
        for source, expected in (("env", "env-key"), ("dotenv", "dotenv-key"), ("keyring", "keyring-key")):
            if source == "env":
                os.environ["TYPESAFE_API_KEY"] = expected
            else:
                os.environ.pop("TYPESAFE_API_KEY", None)
            if source == "keyring":
                dotenv.unlink()
            with server([(200, {}, ANSWER)]) as (url, received):
                value = self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract",
                                      config_dir=self.directory.name, general_log=logger, base_url=url, keyring_lookup=keyring)
            self.assertTrue(value["ok"], value)
            self.assertEqual(received[0][1]["authorization"], "Bearer " + expected)
            self.assertNotIn(expected, json.dumps(value))
        self.assertEqual(keyring_calls, ["TYPESAFE_API_KEY"])
        for log in Path.cwd().rglob("*.jsonl"):
            for secret in ("env-key", "dotenv-key", "keyring-key"):
                self.assertNotIn(secret, log.read_text())

    def test_project_precedence_through_session_paths(self):
        initialized = subprocess.run(["git", "init", "--quiet"], capture_output=True, text=True)
        self.assertEqual(initialized.returncode, 0, initialized.stderr)
        root = Path.cwd()
        local = root / ".sc/sc-agent"
        global_dir = root / "global"
        local.mkdir(parents=True)
        global_dir.mkdir()
        places = [(local / "decidr.json", "local-cli"), (local / "sc-agent.json", "local-shared"),
                  (global_dir / "decidr.json", "global-cli"), (global_dir / "sc-agent.json", "global-shared")]
        for path, project in places:
            path.write_text(json.dumps({"project": project}))
        os.environ["TYPESAFE_API_KEY"] = "contract-secret"
        logger = self.api.logger("decidr")["value"]
        observed = []
        with server([(200, {}, ANSWER)] * 6) as (url, received):
            for i, project in enumerate(("flag", "environment", "local-cli", "local-shared", "global-cli", "global-shared")):
                if i < 2: os.environ["SC_PROJECT"] = "environment"
                else: os.environ.pop("SC_PROJECT", None)
                if i >= 3: places[i - 3][0].unlink()
                result = self.api.send(json.dumps(REQUEST), project="flag" if i == 0 else None,
                                       agent_role="contract", config_dir=str(global_dir), general_log=logger, base_url=url)
                self.assertTrue(result["ok"], result)
                observed.append(project)
            places[-1][0].unlink()
            error = self.error(self.api.send(json.dumps(REQUEST), agent_role="contract", config_dir=str(global_dir), general_log=logger, base_url=url))
        for place in ("--project", "SC_PROJECT", "decidr.json", "sc-agent.json"):
            self.assertIn(place, error["message"])
        rows = [json.loads(line) for path in root.rglob("sessions.jsonl") for line in path.read_text().splitlines()]
        self.assertEqual(sorted(row["project"] for row in rows), sorted(observed))

    def test_bad_inputs_return_values(self):
        for name, args in (("send", (None,)), ("send_batch", (None,)),
                           ("digest", (None,)), ("logger", (None,)),
                           ("log", (None, None))):
            with self.subTest(name=name):
                self.error(getattr(self.api, name)(*args))
        self.error(self.api.sitr.languages(general_log=42))
        self.error(self.api.sitr.query(None, None, None))

    def test_sitr_capture_contract(self):
        with patch.dict(os.environ, {"SITR_LOG_ROOT": self.directory.name}):
            logger = self.api.logger("sitr")["value"]
            source = "fn choose(x: Option<i32>) { match x { Some(0) => (), None => () } }"
            result = self.api.sitr.run("rust", "match-arms", source, general_log=logger)
            self.assertTrue(result["ok"], result)
            self.assertEqual([c["text"] for c in result["value"]], ["Some(0)", "None"])
            self.assertEqual(set(result["value"][0]), {"name", "text", "start_line", "end_line", "start_byte", "end_byte"})
            self.assertEqual(result["value"][0]["start_line"], 1)
            arbitrary = self.api.sitr.query("rust", source, "(function_item name: (identifier) @name)", general_log=logger)
            self.assertTrue(arbitrary["ok"], arbitrary)
            self.assertEqual(arbitrary["value"][0]["text"], "choose")
            self.assertEqual(self.error(self.api.sitr.queries("cobol", general_log=logger))["id"], "treesitter.unknown_language")
        self.error(self.api.sitr.queries(None))
        self.error(self.api.sitr.query(None,None,None))
        self.error(self.api.sitr.run(None,None,None))

    def test_bad_provider_and_options(self):
        for options in ({"provider": "unknown"}, {"timeout_secs": -1},
                        {"timeout_secs": float("nan")}, {"model": "unknown"}):
            self.error(self.api.send(json.dumps(REQUEST), project="test", agent_role="contract", config_dir=self.directory.name, **options))
        for workers in (-1, 0):
            self.error(self.api.send_batch([], workers=workers))
        self.error(self.api.send(json.dumps(REQUEST), sleep=42))

    def test_typed_answers(self):
        cases = (({"type": "noul"}, {"type": "noul", "noul": 0.1234567890123}),
                 ({"type": "score", "criteria": ["low", "high"]}, {"type": "score", "score": 0.73123456789, "confidence": 0.8123456789, "legend": {"0": "low", "1": "high"}, "probabilities": {"0": 0.3, "1": 0.7}}),
                 ({"type": "box"}, {"type": "box", "box": {"top": 0.123456789, "bottom": 0.9, "left": 0.2, "right": 0.8}, "confidence": None}))
        for question, answer in cases:
            with self.subTest(question=question), server([(200, {}, {"answers": {"q": answer}})]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
                got = self.api.send(json.dumps({"state": "evidence", "questions": {"q": question}}),
                                    project="contract", agent_role="contract", config_dir=self.directory.name,
                                    general_log=self.api.logger("decidr")["value"], base_url=url)
                self.assertTrue(got["ok"], got)
                self.assertEqual(got["value"]["answers"]["q"], answer)

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
                                       (401, {"error": "bad contract-secret"}, "http.auth")):
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

    def test_busy_holdoff_is_per_host_and_exponential(self):
        entered, finished = threading.Event(), threading.Event()
        waits, completed_during_holdoff, results = [], [], []
        logger = self.api.logger("decidr")["value"]
        def sleep(delay):
            waits.append(delay)
            entered.set()
            completed_during_holdoff.append(finished.wait(10))
        def send(url, wait):
            return self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract",
                                 config_dir=self.directory.name, general_log=logger, base_url=url, sleep=wait)
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}), server(
                [(429, {}, {}), (429, {}, {}), (200, {}, ANSWER)]) as (first, _), server(
                [(200, {}, ANSWER)]) as (second, _):
            worker = threading.Thread(target=lambda: results.append(send(first, sleep)))
            worker.start()
            try:
                self.assertTrue(entered.wait(10), "busy reply did not reach holdoff")
                other = send(second.replace("127.0.0.1", "localhost"), lambda _: None)
                self.assertTrue(other["ok"], other)
            finally:
                finished.set()
                worker.join(15)
        self.assertFalse(worker.is_alive())
        self.assertEqual(completed_during_holdoff, [True, True])
        self.assertEqual(waits, [1, 2])
        self.assertTrue(results[0]["ok"], results)

    def test_session_append_does_not_wait_for_busy_holdoff(self):
        in_flight, holding, persisted = threading.Event(), threading.Event(), threading.Event()
        observed = []
        busy_replies = 0
        def respond(body):
            nonlocal busy_replies
            if body["state"] == "finished":
                in_flight.set()
                if not holding.wait(10):
                    return 500, {}, {}
                return 200, {}, ANSWER
            if not in_flight.wait(10):
                return 500, {}, {}
            busy_replies += 1
            return (429, {}, {}) if busy_replies == 1 else (200, {}, ANSWER)
        def sleep(_):
            holding.set()
            observed.append(persisted.wait(10))
        def completed(index, value):
            if index == 0 and value["ok"]:
                rows = list(Path(self.directory.name).rglob("sessions.jsonl"))
                if any('"state":"finished"' in path.read_text() for path in rows):
                    persisted.set()
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}), server([None] * 3, respond) as (url, _):
            got = self.api.send_batch([json.dumps({**REQUEST, "state": state}) for state in ("finished", "busy")],
                                      workers=2, project="contract", agent_role="contract",
                                      config_dir=self.directory.name, base_url=url, sleep=sleep,
                                      general_log=self.api.logger("decidr")["value"], on_result=completed)
        self.assertTrue(got["ok"], got)
        self.assertTrue(all(item["result"]["ok"] for item in got["value"]), got)
        self.assertEqual(observed, [True])

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
        with server([(None, {}, None)] * 5) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract",
                                config_dir=self.directory.name, general_log=self.api.logger("decidr")["value"],
                                base_url=url, sleep=waits.append)
        error = self.error(got)
        self.assertEqual(error["id"], "http.network")
        self.assertEqual(error["attempts"], 5)
        self.assertEqual(len(waits), 4)
        self.assertEqual(len(received), 5)


class Cli(Isolated):
    def test_sitr_query(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/sitr"
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "input.rs"
            source.write_text("fn f() { match 1 { 1 => (), _ => () } }")
            env = {"PATH": os.defpath, "SITR_LOG_ROOT": folder}
            got = subprocess.run([str(binary), "query", "rust", "match-arms", str(source)], env=env, cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertEqual(got.returncode, 0, got.stderr)
            self.assertEqual([item["text"] for item in json.loads(got.stdout)["value"]], ["1", "_"])

    def test_send_and_doctor(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        with tempfile.TemporaryDirectory() as folder, server([(200, {}, ANSWER)]) as (url, received):
            port = int(url.split(":")[2].split("/")[0])
            config = Path(folder) / "decidr.json"
            config.write_text(json.dumps({"project": "contract", "workers": 2, "laya": {"host": "127.0.0.1", "port": port, "model": "laya"}}))
            env = {"PATH": os.defpath, "DECIDR_LOG_ROOT": folder}
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
        root = Path(__file__).resolve().parents[2]
        for binary, args in (("decidr", ["models"]), ("sitr", ["languages"]), ("sitr", ["queries", "--language", "rust"])):
            with self.subTest(binary=binary, args=args), tempfile.TemporaryDirectory() as folder:
                env = {"PATH": os.defpath, f"{binary.upper()}_LOG_ROOT": folder}
                got = subprocess.run([str(root / "target/debug" / binary), *args],
                                     env=env, cwd=folder, text=True, capture_output=True, timeout=20)
                self.assertEqual(got.returncode, 0, got.stderr)
                self.assertTrue(json.loads(got.stdout)["ok"])
