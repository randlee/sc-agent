"""Contract tests for both rendered clients and their /2 batch output."""
import importlib.metadata
import importlib.resources
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import sc_agent
from sc_agent.testing import FakeRun, fake_open_run

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from check_consumer_template import render_consumers


class RenderedClient(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.clients = []
        for path in render_consumers(Path(cls.temp.name)):
            spec = importlib.util.spec_from_file_location(path.stem, path)
            module = importlib.util.module_from_spec(spec)
            sys.modules[path.stem] = module
            spec.loader.exec_module(module)
            cls.clients.append(module)

    @classmethod
    def tearDownClass(cls):
        for module in cls.clients:
            sys.modules.pop(module.__name__, None)
        cls.temp.cleanup()

    def test_render_and_typed_values(self):
        package = importlib.resources.files("sc_agent")
        self.assertTrue((package / "_native.pyi").is_file())
        self.assertTrue((package / "py.typed").is_file())
        for c in self.clients:
            with self.subTest(client=c.CLIENT_NAME):
                self.assertEqual(c.SC_DECIDR_VERSION, importlib.metadata.version("sc-decidr"))
                self.assertEqual(hasattr(c, "extract"), c.CLIENT_NAME == "judge-jev")
                self.assertTrue(c.Ok(1).ok)
                self.assertFalse(c.Err(c.ToolError("x", "bad")).ok)
                self.assertEqual(c.typed_answer({"noul": 0.3}), c.NoulAnswer(0.3))
                for value in (None, 1, [], {}, "not JSON", {"ok": "yes"}, {"ok": True, "value": {"answers": None}}, {"ok": False, "error": []}):
                    failure = c.decode_result(value)
                    self.assertFalse(failure.ok, value)
                    self.assertEqual(failure.error.code, "DECIDR.INVALID_RESULT")
                for value in (object(), None, "x"):
                    self.assertEqual(c.from_tool_error(value).code, "DECIDR.ERROR")

    def test_schema_error_and_verdict(self):
        c = self.clients[0]
        schema = json.loads((importlib.resources.files("sc_agent") / "templates/jev-batch-result.schema.json").read_text())
        self.assertEqual(schema["required"], list(c.ResultLine._fields))
        job = c.BatchJob("item", None, "candidate", 1, "request", {}, {}, {}, "directive")
        settings = c.BatchSettings("plan", 1, "prefix", .6, False)
        error = c.wheel_error({"id": "http.auth", "category": "auth", "message": "bad", "attempts": 2})
        line = c.result_line(job, settings, "run", "project", c.Err(error), verdict=lambda *_: ("fail", .7, None))
        encoded = json.loads(c.encode_result_line(line))
        self.assertEqual(list(encoded), list(c.ResultLine._fields))
        self.assertEqual((encoded["error"]["id"], encoded["error"]["attempts"]), ("http.auth", 2))
        self.assertEqual((line.verdict, line.min_confidence, line.leaning), ("fail", .7, None))

    def test_run_batch_cache_answers_and_errors(self):
        c = self.clients[0]
        jobs = [c.BatchJob("item", None, str(i), 1, str(i), {"n": i}, {}, {}, "directive") for i in range(3)]
        envelopes = [{"ok": True, "value": {"answers": {"q": {"noul": .5}}, "session_id": f"id{i}", "cached": False}} for i in range(3)]
        rows = {f"id{i}": {"result": {"value": {"answers": {"q": i}}}} for i in range(3)}
        with fake_open_run(c, {"provider": "other", "run_id": "run", "envelopes": envelopes, "rows": rows}) as runs:
            run = c.open_run(model="example").value
            out = []
            done = c.run_batch(run, jobs, c.BatchSettings("plan", 2, "p", .6, False), project="p", on_line=lambda line, answers: out.append(line))
            self.assertTrue(done.ok)
            self.assertEqual([x.session_id for x in out], ["id0", "id1", "id2"])
            self.assertEqual(runs[0].sent[0][1], False)
            self.assertEqual(c.answers_for([x._asdict() for x in out]).value, {str(i): {"q": i} for i in range(3)})
        refused = FakeRun({"sends_to_error": {"ok": False, "error": {"id": "http.auth", "category": "auth", "message": "no key"}}})
        self.assertEqual(c.call_result(refused, {}).error.code, "AUTH.CREDENTIALS")
        self.assertEqual(refused.sent, [])

    def test_fixture_dry_run_and_redaction(self):
        c = self.clients[0]
        job = c.BatchJob("item", None, "candidate", 1, "request", {}, {"q": {"type": "noul"}}, {}, "directive")
        lines = []
        settings = c.BatchSettings("plan", 1, "p", .6, False)
        run = FakeRun({"run_id": "r"})
        self.assertFalse(c.run_batch(run, [job], settings, fixture={}, on_line=lambda *v: lines.append(v)).ok)
        self.assertEqual(lines, [])
        result = c.run_batch(run, [job], settings, fixture={"*": {"q": {"noul": .2}}}, on_line=lambda *v: lines.append(v))
        self.assertTrue(result.ok)
        self.assertEqual(lines[0][0].run_id, "r")
        self.assertEqual(run.sent, [])
        self.assertTrue(c.run_batch(None, [job], settings._replace(dry_run=True), on_line=lambda *v: self.fail("dry run wrote a line")).ok)
        with patch.dict(sys.modules, {"sc_agent": None}):
            self.assertIn("[redacted]", c.redact_secrets("Bearer abcdefghijklmnopqrstuvwxyz"))
            self.assertFalse(c.open_run().ok)
        deep = "Bearer abcdefghijklmnopqrstuvwxyz"
        for _ in range(10000):
            deep = [deep]
        value = c.redact_secrets(deep)
        for _ in range(10000):
            value = value[0]
        self.assertIn("[redacted]", value)

    def test_abort_on_auth_cancels_remaining_jobs(self):
        c = self.clients[0]
        jobs = [c.BatchJob("item", None, str(i), 1, str(i), {}, {}, {}, "directive") for i in range(3)]
        run = FakeRun({"provider": "other", "envelopes": [{"ok": False, "error": {"id": "http.auth", "category": "auth", "message": "denied"}}]})
        lines = []
        result = c.run_batch(run, jobs, c.BatchSettings("plan", 1, "p", .6, False), abort_on=frozenset({"auth"}), on_line=lambda line, _: lines.append(line))
        self.assertTrue(result.ok)
        self.assertEqual([line.error.code for line in lines], ["AUTH.CREDENTIALS", "CANCELED", "CANCELED"])
        self.assertGreaterEqual(run.cancels, 1)

    def test_open_run_passes_identity_and_logger_only_in_seams(self):
        c = self.clients[0]
        with patch.object(sc_agent, "Run") as constructor:
            self.assertTrue(c.open_run(model="laya").ok)
            self.assertEqual(constructor.call_args.kwargs["tool"], c.CLIENT_NAME)
            self.assertEqual(constructor.call_args.kwargs["agent_role"], c.CLIENT_NAME)
            self.assertEqual(constructor.call_args.kwargs["tool_version"], c.TOOL_VERSION)
            self.assertNotIn("general_log", constructor.call_args.kwargs)
            c.open_run(seams={"general_log": "test"})
            self.assertEqual(constructor.call_args.kwargs["general_log"], "test")

    def test_log_event_redacts_provider_keys_and_propagates_error(self):
        c = self.clients[0]
        run = FakeRun({})
        with patch.dict("os.environ", {"TYPESAFE_API_KEY": "a-secret-value"}):
            result = c.log_event(run, "target", "action", "correlation", {"secret": "a-secret-value"})
        self.assertTrue(result.ok)
        self.assertEqual(run.logged[0]["fields"]["secret"], "[redacted]")
        self.assertEqual(run.logged[0]["correlation_id"], "correlation")
        run.log = lambda _: {"ok": False, "error": {"id": "log.write", "category": "log", "message": "refused"}}
        self.assertEqual(c.log_event(run, "target", "action", None, {}).error.code, "DECIDR.LOG")

    def test_cache_passthrough_and_invalid_abort(self):
        c = self.clients[0]
        success = {"ok": True, "value": {"answers": {}, "session_id": "id", "cached": False}}
        run = FakeRun({"provider": "local", "envelopes": [success]})
        result = c.call_result(run, {}, cache=True)
        self.assertTrue(result.ok)
        self.assertTrue(run.sent[0][1])
        for abort in (None, 42, [{}], {"unknown"}):
            self.assertEqual(c.call_result(run, {}, abort_on=abort).error.code, "DECIDR.INVALID_OPTION")

    def test_schema_matches_encoded_values(self):
        c = self.clients[0]
        schema = json.loads((importlib.resources.files("sc_agent") / "templates/jev-batch-result.schema.json").read_text())
        job = c.BatchJob("item", None, "candidate", 1, "request", {}, {}, {}, "directive")
        settings = c.BatchSettings("plan", 1, "prefix", .6, False)
        for result in (c.Ok(c.Reply({}, "session", False)), c.Err(c.wheel_error({"id": "http.auth", "category": "auth", "message": "bad"}))):
            line = c.result_line(job, settings, "run", "project", result)
            encoded = json.loads(c.encode_result_line(line))
            self.assertEqual(set(encoded), set(schema["required"]))
            type_map = {"string": str, "number": (int, float), "integer": int, "boolean": bool, "null": type(None)}
            for key, rule in schema["properties"].items():
                if key == "error":
                    if encoded[key] is not None:
                        self.assertEqual(set(encoded[key]), set(rule["anyOf"][1]["required"]))
                    continue
                if "const" in rule:
                    self.assertEqual(encoded[key], rule["const"])
                    continue
                allowed = rule["type"] if isinstance(rule["type"], list) else [rule["type"]]
                self.assertIsInstance(encoded[key], tuple(type_map[name] for name in allowed), key)

    def test_redaction_includes_mapping_keys_and_missing_rows(self):
        c = self.clients[0]
        with patch.dict("os.environ", {"TYPESAFE_API_KEY": "a-secret-value"}):
            redacted = c.redact_secrets({"a-secret-value": "Bearer abcdefghijklmnopqrstuvwxyz"})
        self.assertEqual(list(redacted), ["[redacted]"])
        self.assertIn("[redacted]", redacted["[redacted]"])
        with fake_open_run(c, {"rows": {}}):
            result = c.answers_for([{"ok": True, "session_id": "missing", "project": "p", "request_id": "r"}])
        self.assertFalse(result.ok)
        self.assertEqual(result.error.data["id"], "session.not_found")

    def test_invalid_requests_and_callback_errors_return_values(self):
        c = self.clients[0]
        run = FakeRun({"provider": "local"})
        self.assertFalse(c.call_result(run, {"not_json": object()}).ok)
        self.assertFalse(c.call_batch_results(run, [{"not_json": object()}]).ok)
        job = c.BatchJob("item", None, "candidate", 1, "request", {}, {}, {}, "directive")
        settings = c.BatchSettings("plan", 1, "p", .6, False)
        def fail(*_):
            raise ValueError("callback refused")
        result = c.run_batch(None, [job], settings, fixture={"*": {}}, on_line=fail)
        self.assertFalse(result.ok)
        self.assertIn("callback refused", result.error.message)

    def test_moved_tool_error_and_stdio_keep_original_behavior(self):
        c = self.clients[0]
        self.assertEqual(c.ToolError("ERROR", "bad", "fix").to_json(),
                         {"code": "ERROR", "message": "bad", "recoverable": False, "suggested_action": "fix"})
        class ClosedStream:
            def reconfigure(self, **kwargs):
                raise ValueError("closed")
        with patch.object(sys, "stdin", ClosedStream()):
            c.utf8_stdio()
