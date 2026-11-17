"""Controlled tests of consent and shared batch records; no provider or keychain access."""
import hashlib
import io
import json
import sys
from types import SimpleNamespace

import pytest

from consumer_support import install_test_settings_module, render_consumer


def _client(tmp_path):
    return render_consumer(tmp_path, batch_records=True, source_template=True)


def _job(client, anchor_id=None):
    request_id = client.batch_request_id("run", "item", anchor_id, "candidate", 1)
    return client.BatchJob("item", anchor_id, "candidate", 1, request_id,
                           {"model": "jev", "state": "item", "questions": {"q": {"type": "noul"}}},
                           {"q": {"type": "noul"}}, {"r": "q"}, "directive")


def _settings(client):
    return client.BatchSettings("plan", "jev", 1, "run", 0.6, 30.0, 1, 24000, False)


@pytest.mark.parametrize("anchor_id,expected", [("anchor", "run/item/anchor/candidate/r1"),
                                                  (None, "run/item/candidate/r1")])
def test_shared_record_format_and_run_id_are_stable(tmp_path, anchor_id, expected):
    client = _client(tmp_path)
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


def test_fixture_lookup_and_shared_input_callbacks(tmp_path):
    client = _client(tmp_path)
    job = _job(client, "anchor")
    fixture = {"item/candidate/r1": [{"q": {"noul": 0.25}}]}
    assert client._fixture_attempt(job, fixture, 1) == {"model": "fixture", "answers": {"q": {"noul": 0.25}}}
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


def test_batch_stream_logs_success_and_failure_as_each_line_arrives(tmp_path):
    client = _client(tmp_path)
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


def test_stream_batch_converts_worker_exception_to_logged_failure(tmp_path):
    client = _client(tmp_path)
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


def test_duplicate_request_ids_are_rejected_before_batch_execution(tmp_path):
    client = _client(tmp_path)
    job = _job(client)
    make_jobs = lambda line, number, settings, questions: SimpleNamespace(ok=True, value=[job])
    with pytest.raises(client.ClientError, match="appears twice"):
        client.validate_request_lines([{}, {}], _settings(client), {}, make_jobs)


def test_plain_dict_result_and_public_error_conversion(tmp_path, monkeypatch):
    client = _client(tmp_path)
    wheel_result = {"position": 0, "session_id": "s", "sends": 1, "digests": {},
                    "error": None, "answers": {"model": "fixture", "answers": {}}, "usage": {}}
    monkeypatch.setattr(client, "_load_wheel", lambda: SimpleNamespace(parse_result=lambda _: wheel_result))
    result = client.decode_result("controlled wire")
    assert type(result) is dict
    assert result["ok"] is True and result["value"] == {"model": "fixture", "answers": {}}

    tool_error = SimpleNamespace(code="SETTINGS.BAD", message="invalid setting", recoverable=True,
                                 action="repair settings", status=422, retry_after_ms=10,
                                 attempts=2, data={"field": "allow_remote_judge"})
    converted = client.from_tool_error(tool_error)
    assert (converted.code, converted.message, converted.retryable, converted.action) == (
        "SETTINGS.BAD", "invalid setting", True, "repair settings")
    assert (converted.status, converted.retry_after_ms, converted.attempts, converted.data) == (
        422, 10, 2, {"field": "allow_remote_judge"})
    assert "repository_root" not in __import__("inspect").signature(client.call_result).parameters
    assert "repository_root" not in __import__("inspect").signature(client.call_batch_results).parameters


def test_non_boolean_truthy_consent_is_denied_and_settings_failures_are_typed(tmp_path, monkeypatch):
    client = _client(tmp_path)
    settings = install_test_settings_module()
    settings.remote_judge_consent = lambda path: SimpleNamespace(ok=True, value=1)
    with pytest.raises(client.ClientError) as denied:
        client.remote_judge_consent()
    assert denied.value.code == "POLICY.REMOTE_JUDGE_DISABLED"

    settings.find_repo_root = lambda path: SimpleNamespace(
        ok=False, error=SimpleNamespace(code="SETTINGS.ROOT", message="no repository", data={"path": str(path)}))
    with pytest.raises(client.ClientError) as root_error:
        client.remote_judge_consent()
    assert root_error.value.code == "SETTINGS.ROOT" and root_error.value.data["path"]


def test_load_answers_fixture_error_branches(tmp_path):
    client = _client(tmp_path)
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
    client = _client(tmp_path)
    client.write_stdout_line("é\n")
    assert binary.buffer.getvalue() == "é\n".encode("utf-8")
    assert binary.flushed == 1

    text = io.StringIO()
    monkeypatch.setattr(sys, "stdout", text)
    client.write_stdout_line("plain\n")
    assert text.getvalue() == "plain\n"


def test_batch_exports_are_absent_when_batch_records_are_disabled(tmp_path):
    client = render_consumer(tmp_path, source_template=True)
    for name in ("BatchJob", "batch_request_id", "_fixture_attempt", "stream_batch", "pump_batch"):
        assert not hasattr(client, name)


def test_shared_batch_settings_delegates_consumer_question_rules(tmp_path):
    client = _client(tmp_path)
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
    client = _client(tmp_path)
    install_test_settings_module(allowed=allowed, missing=missing)
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
    client = _client(tmp_path)
    install_test_settings_module(allowed=True)
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
