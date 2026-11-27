"""Product contracts. Only loopback HTTP, controlled synchronization and temporary files."""
import importlib
import _thread
import signal
import json
import os
import socket
import sys
from pathlib import Path
import subprocess
import tempfile
import threading
import tomllib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from contextlib import contextmanager
from unittest.mock import patch
import unittest


REQUEST = {"state": "evidence", "questions": {"q": {"type": "choice", "instructions": "True?", "criteria": {"yes": "true", "no": "false"}}}}
ANSWER = {"model": "jev-1.14.0", "answers": {"q": {"type": "choice", "choice": "yes", "confidence": 0.73123456789, "probabilities": {"yes": 0.73123456789, "no": 0.26876543211}}}}
INTERRUPT_CHILD = os.environ.get("SC_INTERRUPT_CHILD") == "1"


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

    http = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=http.serve_forever)
    thread.start()
    try:
        yield f"http://127.0.0.1:{http.server_port}/v1", received
    finally:
        http.shutdown()
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
                       "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()),
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

    def test_sc_agent_home_supplies_wheel_config_and_project(self):
        root = Path(self.directory.name)
        user = root / "user"
        user.mkdir()
        (user / "decidr.local.json").write_text('{"project":"from-home","workers":2}')
        with server([(200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {
                "SC_AGENT_HOME": str(user), "TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send(REQUEST, agent_role="contract", base_url=url,
                                general_log=self.api.logger("decidr")["value"])
        self.assertTrue(got["ok"], got)
        self.assertEqual(len(received), 1)
        rows = [json.loads(line) for path in (root / "sessions").rglob("sessions.jsonl")
                for line in path.read_text().splitlines()]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["project"], "from-home")

    def test_bad_call_signatures_return_errors(self):
        functions = [self.api.logger, self.api.log, self.api.flush, self.api.digest,
                     self.api.send, self.api.send_batch, self.api.sitr.languages,
                     self.api.sitr.queries, self.api.sitr.query, self.api.sitr.run,
                     self.api.Cancel, self.api.Cancel().cancel]
        for function in functions:
            for args, kwargs in [((), {"bogus": 1}), ((None,) * 20, {})]:
                with self.subTest(function=function.__name__, args=args, kwargs=kwargs):
                    self.assertEqual(self.error(function(*args, **kwargs))["id"], "sc_agent_py.invalid_call")
        for function, args, kwargs in [
            (self.api.send, ({},), {"bogus": 1}),
            (self.api.send_batch, ([],), {"bogus": 1}),
            (self.api.log, (None, {}, None), {}),
            (self.api.sitr.languages, (1,), {}),
            (self.api.logger, ("a", "b"), {}),
            (self.api.digest, ({},), {"request": {}}),
        ]:
            with self.subTest(function=function.__name__):
                self.assertEqual(self.error(function(*args, **kwargs))["id"], "sc_agent_py.invalid_call")

    def test_bad_signature_logs_once_to_injected_logger(self):
        logger = self.api.logger("decidr")["value"]
        result = self.api.send({}, bogus=1, general_log=logger)
        self.assertEqual(self.error(result)["id"], "sc_agent_py.invalid_call")
        rows = [json.loads(line) for line in (Path(self.directory.name) / "logs/decidr.log.jsonl").read_text().splitlines()]
        self.assertEqual(sum(row["fields"].get("id") == "sc_agent_py.invalid_call" for row in rows), 1)

    def test_session_and_general_log_share_fixed_root(self):
        root = Path(self.directory.name)
        (root / "decidr.json").write_text(json.dumps({"project": "contract"}))
        code = """
import json, sys
from pathlib import Path
import sc_agent
root = Path(sys.argv[1])
result = sc_agent.send({}, model="jev", project="contract", agent_role="contract", config_dir=str(root), keyring_lookup=lambda *args: "")
assert not result["ok"], result
assert (root / "logs/decidr.log.jsonl").is_file()
rows = [json.loads(line) for path in (root / "sessions").rglob("sessions.jsonl") for line in path.read_text().splitlines()]
assert len(rows) == 1, rows
"""
        got = subprocess.run([sys.executable, "-c", code, str(root)], env=dict(os.environ),
                             cwd=root, text=True, capture_output=True, timeout=20)
        self.assertEqual(got.returncode, 0, got.stderr)

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
                env = {"PATH": os.defpath, "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()), "DECIDR_LOG_ROOT": folder, "SITR_LOG_ROOT": folder}
                got = subprocess.run([sys.executable, "-c", code, folder, order], env=env, cwd=folder, text=True, capture_output=True, timeout=20)
                self.assertEqual(got.returncode, 0, got.stderr)

    def test_callback_exception_is_a_returned_error_with_its_message(self):
        # REQ-SAG-0016: caller exceptions must not vanish behind an ok return.
        def completed(index, value):
            raise ValueError("controlled caller failure")
        with server([(200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send_batch([REQUEST] * 3, workers=1, project="contract", agent_role="contract",
                                      config_dir=self.directory.name, general_log=self.api.logger("decidr")["value"],
                                      base_url=url, on_result=completed)
        error = self.error(got)
        self.assertEqual(error["id"], "sc_agent_py.callback")
        self.assertIn("ValueError: controlled caller failure", error["message"])
        self.assertEqual(len(received), 1)
        rows = [json.loads(line) for path in Path.cwd().rglob("sessions.jsonl") for line in path.read_text().splitlines()]
        self.assertEqual(len(rows), 3)

    def test_a_result_python_cannot_load_is_an_error_value(self):
        # REQ-SAG-0002: the loader's error is returned, not dropped as None.
        with patch("json.loads", side_effect=ValueError("controlled loader failure")):
            got = self.api.sitr.languages()
        error = self.error(got)
        self.assertEqual(error["id"], "sc_agent_py.result_load")
        self.assertIn("controlled loader failure", error["message"])
        self.assertIn('"ok":true', error["message"])

    def test_a_key_is_never_sent_to_a_plain_http_host_that_is_not_loopback(self):
        # REQ-SAG-0013. The refusal comes before any connection or keyring lookup.
        lookups = []
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send(json.dumps(REQUEST), project="contract", agent_role="contract", config_dir=self.directory.name,
                                general_log=self.api.logger("decidr")["value"], base_url="http://192.0.2.1/v1", keyring_lookup=lookups.append)
        error = self.error(got)
        self.assertEqual(error["id"], "http.key_on_plain_http")
        self.assertNotIn("contract-secret", json.dumps(got))
        self.assertEqual(lookups, [])

    def test_cancel_ends_a_send_whose_reply_has_not_arrived(self):
        # REQ-SAG-0011: a send in flight is abandoned on cancel, not waited out to its timeout.
        release, arrived = threading.Event(), threading.Event()
        def respond(body):
            arrived.set()
            release.wait()
            return 200, {}, ANSWER
        cancel, results = self.api.Cancel(), []
        call = None
        with server([None], respond) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            def send():
                results.append(self.api.send(
                    json.dumps(REQUEST), project="contract", agent_role="contract", config_dir=self.directory.name,
                    general_log=self.api.logger("decidr")["value"], base_url=url, timeout_secs=60.0, cancel=cancel))

            call = threading.Thread(target=send)
            try:
                call.start()
                self.assertTrue(arrived.wait(10), f"request did not arrive; received={received!r}, results={results!r}")
                cancel.cancel()
                call.join(5)
                self.assertFalse(call.is_alive(), f"send waited after cancel; received={received!r}, results={results!r}")
            finally:
                cancel.cancel()
                release.set()
                if call.is_alive():
                    call.join(10)
        self.assertEqual(len(received), 1)
        self.assertEqual(len(results), 1)
        self.assertEqual(self.error(results[0])["id"], "decision.cancelled")

    def test_log_returns_an_event_the_logger_refuses(self):
        # REQ-SAG-0002: the logger's refusal is returned, not reported as ok.
        logger = self.api.logger("decidr")["value"]
        error = self.error(self.api.log(logger, {"service": "not-decidr", "action": "contract"}))
        self.assertEqual(error["id"], "log.write")
        self.assertIn("service", error["message"])
        self.assertTrue(self.api.log(logger, {"action": "contract"})["ok"])

    def test_oversized_requests_and_nonpositive_workers_are_typed_refusals(self):
        calls = []
        def keyring(name):
            calls.append(name)
            return "unused"
        kwargs = dict(project="contract", agent_role="contract", config_dir=self.directory.name,
                      general_log=self.api.logger("decidr")["value"], base_url="http://127.0.0.1:1/v1", keyring_lookup=keyring)
        for workers in (-1, 0):
            self.assertEqual(self.error(self.api.send_batch([], workers=workers, **kwargs))["id"], "decidr.invalid_workers")
        bad = self.error(self.api.send(REQUEST, timeout_secs="bad", **kwargs))
        self.assertIn("invalid timeout_secs:", bad["message"])
        self.assertIn("TypeError:", bad["message"])
        self.assertEqual(calls, [])

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

    def test_missing_project_sends_nothing_and_writes_no_session(self):
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}), server([], respond=lambda body: (200, {}, ANSWER)) as (url, received):
            result = self.api.send(REQUEST, agent_role="contract", config_dir=self.directory.name, base_url=url)
        self.assertEqual(self.error(result)["id"], "config.missing_project")
        self.assertEqual(received, [])
        self.assertEqual(list(Path(self.directory.name).rglob("sessions.jsonl")), [])

    def test_project_precedence_through_session_paths(self):
        initialized = subprocess.run(["git", "init", "--quiet"], capture_output=True, text=True, timeout=20)
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
                self.assertTrue(self.api.log(logger, {"action": "python.own", "target": "consumer", "correlation_id": "contract-id", "fields": {"source": "python"}})["ok"])
                self.error(self.api.send("not JSON", config_dir=folder, project="contract", agent_role="contract", general_log=logger))
                self.assertTrue(self.api.sitr.languages(general_log=logger)["ok"])
                self.assertTrue(self.api.flush(logger)["ok"])
                text = (Path(folder) / "logs/t.log.jsonl").read_text()
                own = next(json.loads(line) for line in text.splitlines() if json.loads(line)["action"] == "python.own")
                self.assertEqual((own["target"], own["correlation_id"]), ("consumer", "contract-id"))
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
                expected = {"typesafe": "jev-latest", "anthropic": "claude-haiku-5-5", "openai": "gpt-6-luna", "openrouter": "anthropic/claude-haiku-5.5", "together": "openai/gpt-6-luna", "nous": "custom-id"}
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
        self.assertEqual([item["index"] for item in got["value"]], [0, 1, 2])
        self.assertTrue(got["value"][0]["result"]["ok"])
        for item in got["value"][1:]:
            self.assertEqual(self.error(item["result"])["id"], "decision.cancelled")
        rows = [json.loads(line) for path in Path.cwd().rglob("sessions.jsonl") for line in path.read_text().splitlines()]
        self.assertEqual(len(rows), 3)
        self.assertEqual(sum(row["result"]["ok"] for row in rows), 1)

    def test_keyboard_interrupt_in_callback_cancels_unfinished_results(self):
        def completed(index, result):
            if index == 0:
                raise KeyboardInterrupt
        with server([(200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            got = self.api.send_batch([REQUEST] * 3, workers=1,
                                      project="contract", agent_role="contract", config_dir=self.directory.name,
                                      general_log=self.api.logger("decidr")["value"], base_url=url, on_result=completed)
        self.assertTrue(got["ok"], got)
        self.assertEqual(len(received), 1)
        self.assertTrue(got["value"][0]["result"]["ok"])
        self.assertEqual([self.error(item["result"])["id"] for item in got["value"][1:]], ["decision.cancelled"] * 2)

    def test_pending_python_interrupt_cancels_inflight_send(self):
        if not INTERRUPT_CHILD:
            env = dict(os.environ, SC_INTERRUPT_CHILD="1")
            command = [sys.executable, "-m", "unittest", "discover", "-s", str(Path(__file__).parent),
                       "-p", "test_contract.py", "-k", "test_pending_python_interrupt_cancels_inflight_send"]
            run = subprocess.run(command, env=env, text=True, capture_output=True, timeout=30)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            return
        release = threading.Event()
        def respond(body):
            _thread.interrupt_main()
            if not release.wait(20):
                return 500, {}, {"error": "send_batch did not return while reply was withheld"}
            return 200, {}, ANSWER
        with server([None], respond) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            try:
                got = self.api.send_batch([REQUEST] * 3, workers=1,
                                          project="contract", agent_role="contract", config_dir=self.directory.name,
                                          general_log=self.api.logger("decidr")["value"], base_url=url)
            finally:
                release.set()
        self.assertTrue(got["ok"], got)
        self.assertEqual(len(received), 1)
        self.assertEqual([self.error(item["result"])["id"] for item in got["value"]], ["decision.cancelled"] * 3)
        rows = [json.loads(line) for path in Path.cwd().rglob("sessions.jsonl") for line in path.read_text().splitlines()]
        self.assertEqual(len(rows), 3)
        self.assertTrue(all(row["result"]["error"]["id"] == "decision.cancelled" for row in rows))

    def test_repository_identity_uses_origin_and_real_host(self):
        logger = self.api.logger("decidr")["value"]
        def send(url):
            got = self.api.send(REQUEST, project="contract", agent_role="contract", config_dir=self.directory.name,
                                general_log=logger, base_url=url)
            self.assertTrue(got["ok"], got)
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret", "HOSTNAME": "not-the-host"}), server([(200, {}, ANSWER)] * 5) as (url, _):
            send(url)
            subprocess.run(["git", "init", "--quiet"], check=True, timeout=20)
            subprocess.run(["git", "remote", "add", "origin", "https://github.com/example/project.git"], check=True, timeout=20)
            send(url)
            for origin in ("git@github.com:example/project.git", "https://example.com:8443/o/r.git", "ssh://git@example.com:2222/o/r.git"):
                subprocess.run(["git", "remote", "set-url", "origin", origin], check=True, timeout=20)
                send(url)
        rows = [json.loads(line) for path in Path.cwd().rglob("sessions.jsonl") for line in path.read_text().splitlines()]
        host = subprocess.run(["hostname"], check=True, text=True, capture_output=True, timeout=20).stdout.strip()
        self.assertEqual(sorted(row["repo"] for row in rows), sorted([f"local/{host}/{Path.cwd().name}", "github.com/example/project", "github.com/example/project", "example.com/o/r", "example.com/o/r"]))

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
    def test_decidr_version_and_help_succeed(self):
        self.check_version_and_help("decidr")

    def test_sitr_version_and_help_succeed(self):
        self.check_version_and_help("sitr")

    def check_version_and_help(self, name):
        binary = Path(__file__).resolve().parents[2] / "target/debug" / name
        for flag in ("--version", "--help"):
            got = subprocess.run([str(binary), flag], env={"PATH": os.defpath, "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()), name.upper() + "_LOG_ROOT": self.directory.name},
                                 cwd=self.directory.name, text=True, capture_output=True, timeout=20)
            self.assertEqual(got.returncode, 0, got.stderr + got.stdout)
            self.assertIn(name, got.stdout)
            if flag == "--version":
                manifest = tomllib.loads((Path(__file__).resolve().parents[2] / "Cargo.toml").read_text())
                self.assertEqual(got.stdout.strip(), name + " " + manifest["workspace"]["package"]["version"])

    def test_cli_size_and_worker_refusals_keep_typed_ids(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        Path(self.directory.name, "decidr.json").write_text(json.dumps({"project": "contract", "laya": {"host": "127.0.0.1", "port": 1}}))
        cases = [(["--batch", "--workers", str(workers)], "{}", "decidr.invalid_workers") for workers in (-1, 0)]
        for flags, text, expected in cases:
            got = subprocess.run([str(binary), "--config-dir", self.directory.name, "send", "--model", "laya",
                                 "--agent-role", "contract", *flags],
                                 input=text, env={"PATH": os.defpath, "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()), "DECIDR_LOG_ROOT": self.directory.name},
                                 cwd=self.directory.name, text=True, capture_output=True, timeout=20)
            self.assertNotEqual(got.returncode, 0)
            self.assertEqual(json.loads(got.stdout)["error"]["id"], expected)

    def test_sitr_query(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/sitr"
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "input.rs"
            source.write_text("fn f() { match 1 { 1 => (), _ => () } }")
            env = {"PATH": os.defpath, "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()), "SITR_LOG_ROOT": folder}
            got = subprocess.run([str(binary), "query", "rust", "match-arms", str(source)], env=env, cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertEqual(got.returncode, 0, got.stderr)
            self.assertEqual([item["text"] for item in json.loads(got.stdout)["value"]], ["1", "_"])
            self.assertTrue((Path(folder) / "logs/sitr.log.jsonl").is_file())
            self.assertEqual(list((Path(folder) / "logs").rglob("sessions.jsonl")), [])

    def test_doctor_lists_present_and_missing_local_and_global_layers(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        root = Path(self.directory.name).resolve()
        local, global_config = root / ".sc/sc-agent", root / "global"
        local.mkdir(parents=True)
        global_config.mkdir()
        initialized = subprocess.run(["git", "init", "--quiet", str(root)], env=dict(os.environ), text=True, capture_output=True, timeout=20)
        self.assertEqual(initialized.returncode, 0, initialized.stderr)
        (local / "sc-agent.json").write_text('{"project":"local"}')
        (global_config / "decidr.json").write_text('{"workers":3}')
        got = subprocess.run([str(binary), "--config-dir", str(global_config), "doctor"], cwd=root,
                             env=dict(os.environ), text=True, capture_output=True, timeout=20)
        self.assertEqual(got.returncode, 0, got.stderr + got.stdout)
        value = json.loads(got.stdout)["value"]
        self.assertEqual(value["files"], [{"path": str(path), "exists": exists} for path, exists in [
            (local / "decidr.local.json", False), (local / "decidr.json", False), (local / "sc-agent.json", True),
            (global_config / "decidr.local.json", False), (global_config / "decidr.json", True), (global_config / "sc-agent.json", False)]])
        project = next(row for row in value["settings"] if row["path"] == "project")
        self.assertEqual(project["source"], str(local / "sc-agent.json"))

    def doctor(self, root, environment=None):
        return subprocess.run([str(Path(__file__).resolve().parents[2] / "target/debug/decidr"),
                               "--config-dir", str(root / "global"), "doctor"], cwd=root,
                              env={**dict(os.environ), "DECIDR_LOG_ROOT": str(root / "log-root"),
                                   "GIT_CEILING_DIRECTORIES": str(root.parent), **(environment or {})},
                              text=True, capture_output=True, timeout=20)

    def test_failed_doctor_writes_no_general_log(self):
        root = Path(self.directory.name).resolve()
        (root / "global").mkdir()
        (root / "global/decidr.json").write_text("malformed JSON")
        got = self.doctor(root)
        self.assertNotEqual(got.returncode, 0)
        self.assertEqual(json.loads(got.stdout)["error"]["id"], "config.invalid")
        self.assertEqual(list((root / "log-root").rglob("*")), [])

    def test_doctor_local_worker_precedence_and_source(self):
        root = Path(self.directory.name).resolve()
        subprocess.run(["git", "init", "--quiet", str(root)], check=True, env=dict(os.environ), timeout=20)
        (root / ".sc/sc-agent").mkdir(parents=True)
        (root / "global").mkdir()
        (root / "global/decidr.json").write_text('{"workers":4}')
        local = root / ".sc/sc-agent/decidr.json"
        local.write_text('{"workers":2}')
        got = self.doctor(root)
        self.assertEqual(got.returncode, 0, got.stdout + got.stderr)
        workers = next(row for row in json.loads(got.stdout)["value"]["settings"] if row["path"] == "workers")
        self.assertEqual((workers["value"], workers["source"]), ("2", str(local)))
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        overridden = subprocess.run([str(binary), "--config-dir", str(root / "global"), "--project", "test", "send",
                                     "--batch", "--model", "laya", "--agent-role", "test", "--workers", "0"],
                                    input="{}", cwd=root, env={**dict(os.environ), "DECIDR_WORKERS": "3"},
                                    text=True, capture_output=True, timeout=20)
        self.assertEqual(json.loads(overridden.stdout)["error"]["id"], "decidr.invalid_workers")

    def test_sc_agent_home_relocates_config_without_changing_log_root(self):
        root = Path(self.directory.name).resolve()
        user = root / "user"
        user.mkdir()
        override = user / "decidr.local.json"
        override.write_text('{"project":"relocated","workers":6}')
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        environment = {"PATH": os.defpath, "SC_AGENT_HOME": str(user),
                       "DECIDR_LOG_ROOT": str(root / "logs"),
                       "GIT_CEILING_DIRECTORIES": str(root.parent)}
        got = subprocess.run([str(binary), "doctor"], cwd=root, env=environment,
                             capture_output=True, text=True, timeout=20)
        self.assertEqual(got.returncode, 0, got.stdout + got.stderr)
        value = json.loads(got.stdout)["value"]
        rows = {row["path"]: row for row in value["settings"]}
        self.assertEqual((rows["project"]["value"], rows["project"]["source"]), ("relocated", str(override)))
        self.assertEqual(rows["workers"]["value"], "6")
        self.assertEqual(value["log_root"], str(root / "logs"))
        self.assertEqual(len(value["files"]), 3)
        self.assertEqual(list((root / "logs").rglob("*")), [])

    def test_doctor_environment_sources(self):
        root = Path(self.directory.name).resolve()
        (root / "global").mkdir()
        (root / "global/decidr.json").write_text('{"laya":{"host":"localhost","port":1}}')
        got = self.doctor(root, {"SC_PROJECT": "phase-d", "DECIDR_WORKERS": "3", "DECIDR_LAYA__PORT": "9000"})
        self.assertEqual(got.returncode, 0, got.stdout)
        rows = {row["path"]: row for row in json.loads(got.stdout)["value"]["settings"]}
        for path, value, source in [("project", "phase-d", "SC_PROJECT"), ("workers", "3", "DECIDR_WORKERS"),
                                    ("laya.port", "9000", "DECIDR_LAYA__PORT")]:
            self.assertEqual((rows[path]["value"], rows[path]["source"]), (value, source))

    def test_other_cli_and_decidr_project_are_ignored(self):
        root = Path(self.directory.name).resolve()
        (root / "global").mkdir()
        (root / "global/sitr.json").write_text("invalid JSON must not be read")
        got = self.doctor(root, {"SITR_WORKERS": "0", "DECIDR_PROJECT": "wrong"})
        self.assertEqual(got.returncode, 0, got.stdout)
        rows = {row["path"]: row for row in json.loads(got.stdout)["value"]["settings"]}
        self.assertEqual((rows["project"]["value"], rows["project"]["source"]), ("unset", "default"))
        self.assertEqual((rows["workers"]["value"], rows["workers"]["source"]), ("8", "default"))

    def test_outside_git_ignores_planted_local_file(self):
        root = Path(self.directory.name).resolve()
        (root / ".sc/sc-agent").mkdir(parents=True)
        (root / ".sc/sc-agent/decidr.json").write_text("must not be read")
        got = self.doctor(root)
        self.assertEqual(got.returncode, 0, got.stdout)
        self.assertEqual(len(json.loads(got.stdout)["value"]["files"]), 3)

    def test_local_load_errors_name_full_path_and_key(self):
        root = Path(self.directory.name).resolve()
        subprocess.run(["git", "init", "--quiet", str(root)], check=True, env=dict(os.environ), timeout=20)
        (root / ".sc/sc-agent").mkdir(parents=True)
        file = root / ".sc/sc-agent/decidr.json"
        for text in ("bad-json", '{"workers":0}'):
            file.write_text(text)
            got = self.doctor(root)
            self.assertNotEqual(got.returncode, 0)
            message = json.loads(got.stdout)["error"]["message"]
            self.assertIn(str(file), message)
            if "workers" in text:
                self.assertIn("workers", message)

    def test_default_and_empty_override_resolve_home_without_writes(self):
        root = Path(self.directory.name).resolve()
        if os.name != "nt":
            import pwd
            expected = Path(pwd.getpwuid(os.getuid()).pw_dir) / ".sc-agent"
        else:
            expected = None
        for override in (None, ""):
            environment = dict(os.environ)
            environment.pop("DECIDR_LOG_ROOT", None)
            if override is not None:
                environment["DECIDR_LOG_ROOT"] = override
            binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
            got = subprocess.run([str(binary), "--config-dir", str(root), "doctor"], cwd=root, env=environment,
                                 text=True, capture_output=True, timeout=20)
            self.assertEqual(got.returncode, 0, got.stdout + got.stderr)
            actual = Path(json.loads(got.stdout)["value"]["log_root"])
            if expected is None:
                self.assertTrue(actual.is_absolute())
                self.assertEqual(actual.name, ".sc-agent")
                expected = actual
            self.assertEqual(actual, expected)

    def test_sitr_returns_logger_admission_failure(self):
        root = Path(self.directory.name)
        file = root / "large.rs"
        file.write_text('fn f(){match "" { "' + 'x' * (1024 * 1024) + '" => () }}')
        binary = Path(__file__).resolve().parents[2] / "target/debug/sitr"
        got = subprocess.run([str(binary), "query", "rust", "match-arms", str(file)], cwd=root,
                             env=dict(os.environ), text=True, capture_output=True, timeout=20)
        self.assertNotEqual(got.returncode, 0)
        self.assertEqual(json.loads(got.stdout)["error"]["id"], "log.write")
        self.assertIn("1048576", json.loads(got.stdout)["error"]["message"])

    def test_send_and_doctor(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        with tempfile.TemporaryDirectory() as folder, server([(200, {}, ANSWER)]) as (url, received):
            port = int(url.split(":")[2].split("/")[0])
            config = Path(folder) / "decidr.json"
            config.write_text(json.dumps({"project": "contract", "workers": 2, "laya": {"host": "127.0.0.1", "port": port}}))
            env = {"PATH": os.defpath, "DECIDR_LOG_ROOT": folder,
                   "GIT_CEILING_DIRECTORIES": str(Path(folder).parent.resolve())}
            doctor = subprocess.run([str(binary), "--config-dir", folder, "doctor"], env=env, cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertEqual(doctor.returncode, 0, doctor.stderr)
            self.assertFalse((Path(folder) / "logs").exists())
            project = next(row for row in json.loads(doctor.stdout)["value"]["settings"] if row["path"] == "project")
            self.assertEqual(project["source"], str(config))
            self.assertFalse(any(row["path"] == "log_root" for row in json.loads(doctor.stdout)["value"]["settings"]))
            self.assertEqual(json.loads(doctor.stdout)["value"]["log_root"], folder)
            self.assertEqual(json.loads(doctor.stdout)["value"]["files"], [
                {"path": str(Path(folder) / "decidr.local.json"), "exists": False},
                {"path": str(config), "exists": True},
                {"path": str(Path(folder) / "sc-agent.json"), "exists": False},
            ])
            sent = subprocess.run([str(binary), "--config-dir", folder, "send", "--model", "laya",
                                   "--agent-role", "contract"], input=json.dumps(REQUEST), env=env,
                                  cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertEqual(sent.returncode, 0, sent.stderr + sent.stdout)
            self.assertTrue(json.loads(sent.stdout)["ok"])
            self.assertEqual(received[0][0], "/v1/systemone")
            self.assertTrue((Path(folder) / "logs/decidr.log.jsonl").is_file())
            self.assertEqual(len(list((Path(folder) / "sessions").rglob("sessions.jsonl"))), 1)

    def test_send_requires_agent_role(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        with tempfile.TemporaryDirectory() as folder:
            got = subprocess.run([str(binary), "--config-dir", folder, "send", "--model", "laya"],
                                 input=json.dumps(REQUEST), env={"PATH": os.defpath, "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()), "DECIDR_LOG_ROOT": folder},
                                 cwd=folder, text=True, capture_output=True, timeout=20)
            self.assertNotEqual(got.returncode, 0)
            error = json.loads(got.stdout)["error"]
            self.assertEqual(error["id"], "decidr.usage")
            self.assertIn("--agent-role", error["message"])

    @unittest.skipUnless(os.name == "posix", "POSIX signal delivery")
    def test_sigint_and_sigterm_cancel_and_flush_all_session_rows(self):
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        for interrupt in (signal.SIGINT, signal.SIGTERM):
            with self.subTest(signal=interrupt), tempfile.TemporaryDirectory() as folder:
                release = threading.Event()
                def respond(body):
                    process.send_signal(interrupt)
                    if not release.wait(20):
                        return 500, {}, {"error": "child did not exit while reply was withheld"}
                    return 200, {}, ANSWER
                with server([None], respond) as (url, received):
                    port = int(url.split(":")[2].split("/")[0])
                    Path(folder, "decidr.json").write_text(json.dumps({"project": "contract", "laya": {"host": "127.0.0.1", "port": port}}))
                    env = {"PATH": os.defpath, "DECIDR_LOG_ROOT": folder,
                   "GIT_CEILING_DIRECTORIES": str(Path(folder).parent.resolve())}
                    process = subprocess.Popen([str(binary), "--config-dir", folder, "send", "--model", "laya",
                                                 "--agent-role", "contract", "--batch", "--workers", "1"],
                                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=folder, env=env)
                    try:
                        stdout, stderr = process.communicate("\n".join([json.dumps(REQUEST)] * 3), timeout=20)
                    finally:
                        if process.poll() is None:
                            process.kill()
                            process.communicate()
                        release.set()
                self.assertNotEqual(process.returncode, 0, stdout + stderr)
                results = json.loads(stdout)
                self.assertTrue(results["ok"], results)
                self.assertEqual(len(results["value"]), 3)
                self.assertEqual([row["index"] for row in results["value"]], [0, 1, 2])
                self.assertTrue(all(row["result"]["error"]["id"] == "decision.cancelled"
                                    for row in results["value"]))
                self.assertEqual(len(received), 1)
                rows = [json.loads(line) for path in Path(folder).rglob("sessions.jsonl") for line in path.read_text().splitlines()]
                self.assertEqual(len(rows), 3)
                self.assertTrue(all(row["result"]["error"]["id"] == "decision.cancelled" for row in rows))

    def test_models_and_sitr_queries(self):
        root = Path(__file__).resolve().parents[2]
        for binary, args in (("decidr", ["models"]), ("sitr", ["languages"]), ("sitr", ["queries", "--language", "rust"])):
            with self.subTest(binary=binary, args=args), tempfile.TemporaryDirectory() as folder:
                env = {"PATH": os.defpath, "GIT_CEILING_DIRECTORIES": str(Path(self.directory.name).parent.resolve()), f"{binary.upper()}_LOG_ROOT": folder}
                got = subprocess.run([str(root / "target/debug" / binary), *args],
                                     env=env, cwd=folder, text=True, capture_output=True, timeout=20)
                self.assertEqual(got.returncode, 0, got.stderr)
                self.assertTrue(json.loads(got.stdout)["ok"])
