"""Controlled tests of consent and shared batch records; no provider or keychain access."""
import hashlib
import json
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
