"""The wheel returns values and never raises; every call's record goes to the injected Logger."""
import json

import pytest

import sc_agent


@pytest.fixture
def log(tmp_path, monkeypatch):
    monkeypatch.setenv("T_LOG_ROOT", str(tmp_path))
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
    assert reply["error"]["attempts"] == 0
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


@pytest.mark.parametrize("method", [sc_agent.send, sc_agent.send_batch])
@pytest.mark.parametrize("value", [9.99, 3600.01, float("nan"), float("inf"), "bad", True, [], {}])
def test_timeout_invalid_is_an_error_value(method, value, log):
    logger, _ = log
    request = "not json" if method is sc_agent.send else ["not json"]
    result = method(request, "haiku", "p", "t", logger, timeout_secs=value)
    reply = json.loads(result if method is sc_agent.send else result[0])
    assert reply["error"]["id"] == "decidr.invalid_request_timeout"
    assert "10..=3600" in reply["error"]["message"]
    assert sc_agent.category(reply["error"]) == "validation"
    assert sc_agent.retryable(reply["error"]) is False


@pytest.mark.parametrize("value", [-1, 0, 65, 1.5, True, "bad", []])
def test_invalid_workers_do_not_raise(value, log):
    logger, _ = log
    replies = sc_agent.send_batch(["not json"], "haiku", "p", "t", logger, workers=value)
    assert json.loads(replies[0])["error"]["id"] == "decidr.invalid_workers"


@pytest.mark.parametrize("method", [sc_agent.send, sc_agent.send_batch])
@pytest.mark.parametrize("timeout", [10.125, 3600, None])
def test_valid_timeout_reaches_request_parsing(method, timeout, log):
    logger, _ = log
    request = "not json" if method is sc_agent.send else ["not json"]
    result = method(request, "haiku", "p", "t", logger, timeout_secs=timeout)
    reply = json.loads(result if method is sc_agent.send else result[0])
    assert reply["error"]["id"] == "config.invalid"
    assert "request 0" in reply["error"]["message"]


@pytest.mark.parametrize("method", [sc_agent.send, sc_agent.send_batch])
@pytest.mark.parametrize("options", [
    {"keyring_service": "custom"}, {"keyring_account": "alice"},
    {"keyring_service": "custom", "keyring_account": "alice"},
])
def test_keyring_options_are_accepted_without_reading_keys(method, options, log):
    logger, _ = log
    request = "not json" if method is sc_agent.send else ["not json"]
    result = method(request, "haiku", "p", "t", logger, **options)
    reply = json.loads(result if method is sc_agent.send else result[0])
    assert "request 0" in reply["error"]["message"]


@pytest.mark.parametrize("option", ["keyring_service", "keyring_account"])
@pytest.mark.parametrize("method", [sc_agent.send, sc_agent.send_batch])
def test_invalid_keyring_type_is_a_value(method, option, log):
    logger, _ = log
    request = "not json" if method is sc_agent.send else ["not json"]
    result = method(request, "haiku", "p", "t", logger, **{option: 42})
    reply = json.loads(result if method is sc_agent.send else result[0])
    assert reply["ok"] is False
    assert "keyring" in reply["error"]["message"]


@pytest.mark.parametrize("value,category,retryable", [
    ({"category": "busy"}, "busy", True), ({"category": "network"}, "network", True),
    ({"category": "quota"}, "quota", False), ({"category": "auth"}, "auth", False),
    ({"category": "validation"}, "validation", False), ({}, "other", False),
    ({"category": None}, "other", False), (None, "other", False),
    ("bad json", "other", False), (42, "other", False),
])
def test_error_helpers_read_values_without_raising(value, category, retryable):
    assert sc_agent.category(value) == category
    assert sc_agent.retryable(value) is retryable
    if isinstance(value, dict):
        assert sc_agent.category(json.dumps(value)) == category
        assert sc_agent.retryable(json.dumps(value)) is retryable


def test_precancelled_batch_delivers_each_result_and_catches_callback(log):
    logger, _ = log
    token = sc_agent.CancelToken()
    token.cancel()
    seen = []

    def persist(line):
        seen.append(line)
        if len(seen) == 1:
            raise ValueError("controlled callback failure")

    returned = sc_agent.send_batch(["{}", "{}"], "haiku", "p", "t", logger,
                                   cancel=token, on_result=persist)
    assert len(seen) == len(returned) == 2
    first, second = map(json.loads, returned)
    assert first["error"]["message"] == "on_result raised: ValueError: controlled callback failure"
    assert sc_agent.retryable(first["error"]) is False
    assert json.loads(seen[0])["error"]["id"] == "request.cancelled"
    assert json.loads(seen[1])["position"] == 1
    assert len(json.loads(seen[1])["session_id"]) == 26
    assert {k: v for k, v in json.loads(seen[1]).items() if k not in {"position", "session_id"}} == second
    assert second["error"]["id"] == "request.cancelled"
    assert first["error"]["attempts"] == second["error"]["attempts"] == 0


def test_callback_lines_map_back_to_inputs_when_consumed_out_of_order(log):
    logger, _ = log
    token = sc_agent.CancelToken()
    token.cancel()
    seen = []
    returned = sc_agent.send_batch(["{}", "{}"], "haiku", "p", "t", logger,
                                   cancel=token, on_result=lambda line: seen.append(json.loads(line)))
    received = {line["position"]: line for line in reversed(seen)}
    assert list(received) == [1, 0]
    for position, reply in enumerate(map(json.loads, returned)):
        assert received[position]["error"] == reply["error"]
        assert len(received[position]["session_id"]) == 26
        assert "position" not in reply and "session_id" not in reply
    assert received[0]["session_id"] != received[1]["session_id"]
