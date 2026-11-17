"""Controlled tests of consent and shared batch records; no provider or keychain access."""
import hashlib
import importlib
import io
import json
import sys
from types import SimpleNamespace

import pytest

from consumer_support import install_test_settings_module, render_consumer


def _client(tmp_path, monkeypatch):
    return render_consumer(tmp_path, monkeypatch=monkeypatch, batch_records=True, source_template=True)


def _job(client, anchor_id=None):
    request_id = client.batch_request_id("run", "item", anchor_id, "candidate", 1)
    return client.BatchJob("item", anchor_id, "candidate", 1, request_id,
                           {"model": "jev", "state": "item", "questions": {"q": {"type": "noul"}}},
                           {"q": {"type": "noul"}}, {"r": "q"}, "directive")


def _settings(client):
    return client.BatchSettings("plan", "jev", 1, "run", 0.6, 30.0, 1, 24000, False)


@pytest.mark.parametrize("anchor_id,expected", [("anchor", "run/item/anchor/candidate/r1"),
                                                  (None, "run/item/candidate/r1")])
def test_shared_record_format_and_run_id_are_stable(tmp_path, monkeypatch, anchor_id, expected):
    client = _client(tmp_path, monkeypatch)
    job = _job(client, anchor_id)
    assert job.request_id == expected
    assert client.batch_run_id(job.request_id) == hashlib.sha256(expected.encode()).hexdigest()[:16]
    assert client.questions_digest(job.questions) == hashlib.sha256(
        json.dumps(job.questions, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

    raw = {"model": "fixture", "answers": {"q": {"noul": 0.5}}}
    line = client._line(job, _settings(client), raw, [{"attempt": 1, "ok": True}], None, False,
                        result_fields=lambda job, settings, raw: {"verdict": "satisfied"})
    assert line["schema"] == "jev-batch-result/1"
    assert line["request_id"] == expected and line["run_id"] == client.batch_run_id(expected)
    assert line["anchor_id"] == anchor_id and line["verdict"] == "satisfied"
    assert line["questions_sha256"] == client.questions_digest(job.questions)
    assert line["request_sha256"] == hashlib.sha256(json.dumps(job.body, allow_nan=False).encode()).hexdigest()
    clipped = client._line(job, _settings(client), None, [], client.ClientError("INPUT", "long message"), False,
                           error_message_limit=4)
    assert clipped["error"]["message"] == "long"


def test_fixture_lookup_and_shared_input_callbacks(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    job = _job(client, "anchor")
    fixture = {"item/candidate/r1": [{"q": {"noul": 0.25}}]}
    fixture_result = client._fixture_attempt(job, fixture, 1)
    assert fixture_result["model"] == "fixture"
    assert fixture_result["answers"]["q"].noul == 0.25
    with pytest.raises(client.ClientError, match="answers fixture has no entry"):
        client._fixture_attempt(job, {}, 1)

    question_sets = object()
    seen = []
    make_jobs = lambda line, number, settings, questions: (
        seen.append((line, number, settings, questions)) or SimpleNamespace(ok=True, value=[job]))
    assert client.validate_request_lines([{"item": "one"}], _settings(client), question_sets, make_jobs) == 1
    assert seen == [({"item": "one"}, 1, _settings(client), question_sets)]
    assert client.check_fixture_coverage([{"item": "one"}], _settings(client), question_sets, make_jobs, fixture) is None

    loaded = client.load_answers_fixture("answers.json", "item/candidate", lambda path, label:
                                         SimpleNamespace(ok=True, value=fixture))
    assert loaded is fixture

    with pytest.raises(client.ClientError) as transient:
        client._fixture_attempt(job, {"item/candidate/r1": {client.BATCH_FIXTURE_ERROR: "DECIDR.NETWORK"}},
                                1, transient_codes=frozenset({"DECIDR.NETWORK"}))
    assert transient.value.code == "DECIDR.NETWORK" and transient.value.retryable is True


def test_batch_stream_logs_success_and_failure_as_each_line_arrives(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    ok = {"ok": True, "attempts": [{"attempt": 1}]}
    failed = {"ok": False, "attempts": [{"attempt": 1}], "error": {"code": "AUTH", "message": "denied"}}
    logged = []

    async def lines():
        yield ok
        yield failed

    result = client.pump_batch(lines(), log_result=logged.append)
    assert result == [ok, failed]
    assert logged == [ok, failed]
    assert client._add_attempts(result) == client._total_attempts(result) == 2
    summary = client.streamed_result(result)
    assert summary.failed == [failed] and summary.attempts == 2
    assert client.encode_result_line(failed).endswith("\n")


def test_stream_batch_converts_worker_exception_to_logged_failure(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    job = _job(client)
    logged = []

    def make_jobs(line, number, settings, question_sets):
        return SimpleNamespace(ok=True, value=[job])

    def run_job(job, settings):
        raise ValueError("controlled worker failure")

    async def source():
        yield {"item": "one"}

    result = client.pump_batch(
        client.stream_batch(source(), _settings(client), make_jobs=make_jobs, run_job=run_job, question_sets={}),
        log_result=logged.append,
    )
    assert result == logged and len(logged) == 1
    assert logged[0]["ok"] is False
    assert logged[0]["request_id"] == job.request_id
    assert logged[0]["error"] == {"code": "DECIDR.BATCH_JOB",
                                   "message": "batch job failed: ValueError: controlled worker failure",
                                   "recoverable": False}


def test_duplicate_request_ids_are_rejected_before_batch_execution(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    job = _job(client)
    make_jobs = lambda line, number, settings, questions: SimpleNamespace(ok=True, value=[job])
    with pytest.raises(client.ClientError, match="appears twice"):
        client.validate_request_lines([{}, {}], _settings(client), {}, make_jobs)


def test_plain_dict_result_and_public_error_conversion(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    wheel_result = {"position": 0, "session_id": "s", "sends": 1, "digests": {},
                    "error": None, "answers": {}, "usage": {}}
    monkeypatch.setattr(client, "_load_wheel", lambda: SimpleNamespace(parse_result=lambda _: wheel_result))
    result = client.decode_result(json.dumps({"response": {"model": "fixture"}}))
    assert type(result) is dict
    assert result["ok"] is True and result["value"] == {} and result["model_answered"] == "fixture"

    tool_error = SimpleNamespace(code="SETTINGS.BAD", message="invalid setting", recoverable=True,
                                 action="repair settings", status=422, retry_after_ms=10,
                                 attempts=2, data={"field": "allow_remote_judge"})
    converted = client.from_tool_error(tool_error)
    assert (converted.code, converted.message, converted.retryable, converted.action) == (
        "SETTINGS.BAD", "invalid setting", True, "repair settings")
    assert (converted.status, converted.retry_after_ms, converted.attempts, converted.data) == (
        422, 10, 2, {"field": "allow_remote_judge"})
    assert "repository_root" in __import__("inspect").signature(client.call_result).parameters
    assert "repository_root" in __import__("inspect").signature(client.call_batch_results).parameters


def test_non_boolean_truthy_consent_is_denied_and_settings_failures_are_typed(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    settings = install_test_settings_module(monkeypatch)
    settings.remote_judge_consent = lambda path: SimpleNamespace(ok=True, value=1)
    with pytest.raises(client.ClientError) as denied:
        client.remote_judge_consent()
    assert denied.value.code == "POLICY.REMOTE_JUDGE_DISABLED"

    settings.find_repo_root = lambda path: SimpleNamespace(
        ok=False, error=SimpleNamespace(code="SETTINGS.ROOT", message="no repository", data={"path": str(path)}))
    with pytest.raises(client.ClientError) as root_error:
        client.remote_judge_consent()
    assert root_error.value.code == "SETTINGS.ROOT" and root_error.value.data["path"]


def test_settings_import_error_is_typed_and_preserves_cause(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    import_error = ImportError("controlled missing settings module")
    import_module = importlib.import_module

    def fail_settings_import(name, package=None):
        if name == "sc_agent_test_settings":
            raise import_error
        return import_module(name, package)

    monkeypatch.setattr(importlib, "import_module", fail_settings_import)
    with pytest.raises(client.ClientError) as caught:
        client.remote_judge_consent()
    assert caught.value.code == "DEPENDENCY.SETTINGS"
    assert "Install the consumer's settings module" in caught.value.action
    assert caught.value.__cause__ is import_error

    loaded = []
    monkeypatch.setattr(client, "_load_wheel", lambda: loaded.append(True))
    result = client.call_result("{}", model="jev", project="p", agent_role="r", abort_on=frozenset())
    assert result["ok"] is False and result["error"].code == "DEPENDENCY.SETTINGS"
    assert result["error"].__cause__ is import_error
    assert loaded == []


def test_settings_module_installation_is_scoped():
    name = "sc_agent_test_settings"
    before = sys.modules.get(name)
    with pytest.MonkeyPatch.context() as scoped_patch:
        module = install_test_settings_module(scoped_patch)
        assert sys.modules[name] is module
    assert sys.modules.get(name) is before


def test_load_answers_fixture_error_branches(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    assert client.load_answers_fixture("", "item/candidate", lambda *_: None) is None

    failed = SimpleNamespace(ok=False, error=SimpleNamespace(code="INPUT.READ", message="unreadable"))
    with pytest.raises(client.BatchInputError) as read_error:
        client.load_answers_fixture("answers.json", "item/candidate", lambda *_: failed)
    assert read_error.value.error.code == "INPUT.READ"

    with pytest.raises(client.ClientError, match="must be an object"):
        client.load_answers_fixture("answers.json", "item/candidate",
                                    lambda *_: SimpleNamespace(ok=True, value=[]))

    invalid = client.ClientError("INPUT.SCHEMA", "invalid answer envelope")
    with pytest.raises(client.BatchInputError) as envelope_error:
        client.load_answers_fixture("answers.json", "item/candidate",
                                    lambda *_: SimpleNamespace(ok=True, value={}),
                                    validate_envelope=lambda *_: invalid)
    assert envelope_error.value.error.code == invalid.code


def test_write_stdout_line_uses_binary_and_text_streams(tmp_path, monkeypatch):
    class BinaryStdout:
        def __init__(self):
            self.buffer = io.BytesIO()
            self.flushed = 0

        def flush(self):
            self.flushed += 1

    binary = BinaryStdout()
    monkeypatch.setattr(sys, "stdout", binary)
    client = _client(tmp_path, monkeypatch)
    client.write_stdout_line("é\n")
    assert binary.buffer.getvalue() == "é\n".encode("utf-8")
    assert binary.flushed == 1

    text = io.StringIO()
    monkeypatch.setattr(sys, "stdout", text)
    client.write_stdout_line("plain\n")
    assert text.getvalue() == "plain\n"


def test_batch_exports_are_absent_when_batch_records_are_disabled(tmp_path, monkeypatch):
    client = render_consumer(tmp_path, monkeypatch=monkeypatch, source_template=True)
    for name in ("BatchJob", "batch_request_id", "_fixture_attempt", "stream_batch", "pump_batch"):
        assert not hasattr(client, name)


def test_shared_batch_settings_delegates_consumer_question_rules(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    args = SimpleNamespace(repeat=2, max_workers=2, model="jev", timeout=3.0, dry_run=True,
                           gate="dev", threshold=0.75, max_request_bytes=900)
    settings = client.batch_settings(args, "run", check_threshold=lambda value: SimpleNamespace(ok=True, value=value),
                                     request_cap=lambda gate, requested: SimpleNamespace(ok=True, value=requested))
    assert settings == client.BatchSettings("dev", "jev", 2, "run", 0.75, 3.0, 2, 900, True)
    assert client._safe_id("candidate-1", "candidate") == "candidate-1"
    with pytest.raises(client.ClientError, match="safe id"):
        client._safe_id("unsafe id", "candidate")


@pytest.mark.parametrize("allowed,missing,code", [(False, False, "POLICY.REMOTE_JUDGE_DISABLED"),
                                                   (False, True, "POLICY.REMOTE_JUDGE_DISABLED")])
def test_both_send_paths_refuse_without_affirmative_consent(tmp_path, monkeypatch, allowed, missing, code):
    client = _client(tmp_path, monkeypatch)
    install_test_settings_module(monkeypatch, allowed=allowed, missing=missing)
    loaded = []
    monkeypatch.setattr(client, "_load_wheel", lambda: loaded.append(True))

    single = client.call_result("{}", model="jev", project="p", agent_role="r", abort_on=frozenset())
    batch = client.call_batch_results(["{}"], model="jev", project="p", agent_role="r", abort_on=frozenset())
    direct = client._invoke("send", "{}", frozenset(), {}, None)
    assert not single["ok"] and single["error"].code == code
    assert not batch["ok"] and batch["error"].code == code
    assert not direct["ok"] and direct["error"].code == code
    assert loaded == []
    assert "allow_remote_judge" not in __import__("inspect").signature(client.call_result).parameters


def test_affirmative_consent_allows_shared_send_path_and_cancel_envelope(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    install_test_settings_module(monkeypatch, allowed=True)
    sent = []
    class CancelToken:
        def cancel(self):
            pass

    class SagError(Exception):
        pass

    wheel = SimpleNamespace(CancelToken=CancelToken, SagError=SagError,
                            send=lambda request, **kwargs: sent.append(("send", request)) or "wire",
                            send_batch=lambda requests, **kwargs: (sent.append(("send_batch", requests)),
                                                                    kwargs["on_result"]("wire"), ["wire"])[2])
    monkeypatch.setattr(client, "_load_wheel", lambda: wheel)
    monkeypatch.setattr(client, "decode_result", lambda line, **kwargs:
                        {"ok": True, "value": {}, "position": 0, "error": None, "attempts": 1})
    result = client.call_result("{}", model="jev", project="p", agent_role="r", abort_on=frozenset())
    batch = client.call_batch_results(["{}"], model="jev", project="p", agent_role="r", abort_on=frozenset())
    assert result["ok"] and batch["ok"]
    assert [entry[0] for entry in sent] == ["send", "send_batch"]

    canceled = client.canceled_envelope()
    assert client.CANCELED_EXIT == 130
    assert canceled["canceled"] and canceled["aborted_by"] == "user"
    assert canceled["error"]["code"] == "CANCELED"
    assert canceled["error"]["recoverable"] is True


@pytest.mark.parametrize("answer,expected", [
    ({"type": "noul", "noul": 0.5}, {"type": "noul", "noul": 0.5}),
    ({"type": "choice", "choice": "yes", "confidence": 0.8, "probabilities": {"yes": 0.8}},
     {"type": "choice", "choice": "yes", "confidence": 0.8, "probabilities": {"yes": 0.8}}),
    ({"type": "score", "score": 2, "confidence": 0.8, "legend": {"2": "high"}, "probabilities": {"2": 0.8}},
     {"type": "score", "score": 2, "confidence": 0.8, "legend": {"2": "high"}, "probabilities": {"2": 0.8}}),
    ({"type": "box", "box": {"top": 0.1, "bottom": 0.9, "left": 0.2, "right": 0.8}, "confidence": 0.7},
     {"type": "box", "box": {"top": 0.1, "bottom": 0.9, "left": 0.2, "right": 0.8}, "confidence": 0.7}),
])
def test_real_wheel_and_offline_fixture_share_typed_batch_records(tmp_path, monkeypatch, answer, expected):
    client = _client(tmp_path, monkeypatch)
    job = _job(client)
    wire = json.dumps({"session_id": "session", "position": 0, "sends": 3, "digests": {},
                       "response": {"model": "jev-real", "answers": {"q": answer},
                                    "usage": {"input_tokens": 1, "output_tokens": 1}}})
    live = client.decode_result(wire)
    assert live["ok"] is True
    fixture = client._fixture_attempt(job, {"*": {"q": answer}}, 1)
    assert type(live["value"]["q"]) is type(fixture["answers"]["q"])
    assert live["value"]["q"] == fixture["answers"]["q"]
    for raw, model, sends in ((live, "jev-real", 3), (fixture, "fixture", None)):
        record = client._line(job, _settings(client), raw, [], None, False)
        decoded = json.loads(client.encode_result_line(record))
        assert decoded["answers"]["q"] == expected
        assert decoded["raw_response"]["answers"]["q"] == expected
        assert decoded["model_answered"] == model and decoded["sends"] == sends


def test_fixture_conversion_does_not_import_wheel(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    monkeypatch.setattr(client, "_load_wheel", lambda: pytest.fail("fixture loaded wheel"))
    result = client._fixture_attempt(_job(client), {"*": {"q": {"noul": 0.5}}}, 1)
    assert result["answers"]["q"].noul == 0.5
    assert json.loads(client.encode_result_line(result))["answers"]["q"] == {"type": "noul", "noul": 0.5}


def test_batch_error_retains_recovery_action(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    error = client.ClientError("DECIDR.NETWORK", "unavailable", action="Retry later")
    line = client._line(_job(client), _settings(client), None, [], error, False)
    assert json.loads(client.encode_result_line(line))["error"]["suggested_action"] == "Retry later"


def test_batch_line_encodes_each_native_wheel_answer(tmp_path, monkeypatch):
    import sc_agent

    client = _client(tmp_path, monkeypatch)
    answers = {
        "n": sc_agent.NoulAnswer(0.5),
        "c": sc_agent.ChoiceAnswer("yes", 0.8, {"yes": 0.8}),
        "s": sc_agent.ScoreAnswer(2, 0.8, {"2": "high"}, {"2": 0.8}),
        "b": sc_agent.BoxAnswer(0.1, 0.9, 0.2, 0.8, 0.7),
    }
    for key, answer in answers.items():
        line = client._line(_job(client), _settings(client), {"model": "jev", "answers": {key: answer}},
                            [], None, False)
        decoded = json.loads(client.encode_result_line(line))
        assert decoded["answers"][key]["type"] == {"n": "noul", "c": "choice", "s": "score", "b": "box"}[key]
        assert decoded["raw_response"]["answers"][key] == decoded["answers"][key]


@pytest.mark.parametrize("bad", [{"choice": "yes"}, {"noul": "bad"}, {"box": {"top": 1}},
                                  {"type": "unknown"}, {"noul": True}])
def test_malformed_fixture_answer_is_rejected_without_wheel(tmp_path, monkeypatch, bad):
    client = _client(tmp_path, monkeypatch)
    monkeypatch.setattr(client, "_load_wheel", lambda: pytest.fail("fixture loaded wheel"))
    with pytest.raises(client.ClientError) as error:
        client._fixture_attempt(_job(client), {"*": {"q": bad}}, 1)
    assert error.value.code == "DECIDR.RESPONSE_INVALID"


def test_redaction_happens_at_serialization_and_preserves_typed_records(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    job = _job(client)
    raw = client._fixture_attempt(job, {"*": {"q": {"noul": 0.5}}}, 1)
    raw["secret"] = "sk-canary"
    line = client._line(job, _settings(client), raw, [], None, False)
    typed = line["answers"]["q"]

    def redact(value):
        if isinstance(value, dict):
            return {key: redact(item) for key, item in value.items()}
        if isinstance(value, (list, tuple)):
            return tuple(redact(item) for item in value)
        return value.replace("sk-canary", "[redacted]") if isinstance(value, str) else value

    logged, echoed = [], []
    monkeypatch.setattr(client, "write_stdout_line", echoed.append)

    async def source():
        yield line

    results = client.pump_batch(source(), log_result=logged.append, echo=True, redact=redact)
    assert results[0]["answers"]["q"] is typed and typed.noul == 0.5
    assert logged[0]["answers"]["q"] == {"type": "noul", "noul": 0.5}
    assert logged[0]["raw_response"]["secret"] == "[redacted]"
    assert "sk-canary" not in echoed[0] and "sk-canary" not in client.encode_result_line(line, redact=redact)


@pytest.mark.parametrize("sends", [0, 1, 4])
def test_native_send_count_controls_batch_retry_count(tmp_path, monkeypatch, sends):
    client = _client(tmp_path, monkeypatch)
    error = client.ClientError("DECIDR.NETWORK", "down", attempts=sends)
    for raw, failure in ((None, error), ({"ok": False, "error": error, "attempts": sends}, None),
                         ({"ok": True, "value": {}, "model_answered": "jev", "attempts": sends}, None)):
        line = client._line(_job(client), _settings(client), raw, [], failure, False)
        assert line["sends"] == sends and line["retry_count"] == max(sends - 1, 0)
        assert line["ok"] is (failure is None and raw.get("ok") is True)


def test_fixture_retry_count_uses_local_attempts_when_native_sends_unknown(tmp_path, monkeypatch):
    client = _client(tmp_path, monkeypatch)
    raw = client._fixture_attempt(_job(client), {"*": {"q": {"noul": 0.5}}}, 1)
    line = client._line(_job(client), _settings(client), raw, [{}, {}], None, False)
    assert line["sends"] is None and line["retry_count"] == 1


@pytest.mark.parametrize("batch", [False, True])
@pytest.mark.parametrize("selected,cwd,change,should_send", [
    (True, None, False, True),
    (False, True, False, False),
    (True, True, True, False),
])
def test_send_rechecks_selected_repository_consent(tmp_path, monkeypatch, batch, selected, cwd, change, should_send):
    client = _client(tmp_path, monkeypatch)
    selected_root = tmp_path / "selected"
    cwd_root = tmp_path / "cwd"
    selected_root.mkdir()
    cwd_root.mkdir()
    monkeypatch.chdir(cwd_root)
    values = {selected_root: selected, cwd_root: cwd}
    settings = install_test_settings_module(monkeypatch)
    settings.find_repo_root = lambda path: SimpleNamespace(ok=True, value=path)
    def consent(path):
        value = values.get(path)
        if value is None:
            return SimpleNamespace(ok=False, error=SimpleNamespace(code="SETTINGS.REMOTE_JUDGE_UNSET"))
        return SimpleNamespace(ok=True, value=value)
    settings.remote_judge_consent = consent
    sent = []
    class CancelToken:
        def cancel(self):
            pass
    wheel = SimpleNamespace(CancelToken=CancelToken, SagError=type("SagError", (Exception,), {}),
                            send=lambda request, **kwargs: sent.append(request) or "wire",
                            send_batch=lambda requests, **kwargs: sent.append(requests) or ["wire"])
    monkeypatch.setattr(client, "_load_wheel", lambda: wheel)
    monkeypatch.setattr(client, "decode_result", lambda line, **kwargs: {"ok": True, "value": {}, "position": 0})
    if change:
        client.remote_preflight(repository_root=str(selected_root))
        values[selected_root] = False
    call = client.call_batch_results if batch else client.call_result
    result = call(["{}"] if batch else "{}", model="jev", project="p", agent_role="r",
                  abort_on=frozenset(), repository_root=str(selected_root))
    assert result["ok"] is should_send
    assert bool(sent) is should_send
    if not should_send:
        assert result["error"].code == "POLICY.REMOTE_JUDGE_DISABLED"
    assert ".sc/test/settings.yaml" in client.REPO_ROOT_HELP
