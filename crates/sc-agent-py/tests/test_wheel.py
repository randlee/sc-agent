"""The wheel returns values and never raises; every call's record goes to the injected Logger."""
import json

import pytest

import sc_agent


@pytest.fixture
def log(tmp_path, monkeypatch):
    monkeypatch.setenv("SC_DECIDR_LOG_ROOT", str(tmp_path))
    logger, error = sc_agent.logger("t")
    assert error is None
    return logger, tmp_path


def records(root):
    path = root / "logs" / "t.log.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def test_bad_request_is_a_value_and_is_logged(log):
    logger, root = log
    reply = json.loads(sc_agent.send("not json", "haiku", "p", "t", logger))
    assert reply["ok"] is False and reply["error"]["id"]
    assert [r["fields"]["ok"] for r in records(root)] == [False]


def test_zero_workers_is_a_value_per_request(log):
    logger, root = log
    replies = [json.loads(r) for r in sc_agent.send_batch(["{}", "{}"], "haiku", "p", "t", logger, workers=0)]
    assert len(replies) == 2 and all(r["ok"] is False for r in replies)
    assert len(records(root)) == 2


def test_bad_slug_is_a_value(log):
    logger, _ = log
    request = json.dumps({"model": "x", "state": {}, "questions": {}})
    reply = json.loads(sc_agent.send(request, "haiku", "Phase B", "t", logger))
    assert reply["ok"] is False


def test_cancel_token():
    token = sc_agent.CancelToken()
    assert not token.cancelled()
    token.cancel()
    assert token.cancelled()


def test_empty_batch_is_no_replies(log):
    logger, root = log
    assert sc_agent.send_batch([], "haiku", "p", "t", logger) == []
    assert not (root / "logs" / "t.log.jsonl").exists() or records(root) == []
