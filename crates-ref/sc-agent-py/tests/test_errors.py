"""Python exception attributes and stored error categories."""
import copy
import json
import pathlib
import pickle

import pytest
import sc_agent


def test_error_fields_and_pickle(env, args):
    with pytest.raises(sc_agent.SagError) as caught:
        sc_agent.send("{}", **args, timeout_secs=1)
    error = caught.value
    assert error.id == "decidr.invalid_request_timeout"
    assert error.message == error.args[1]
    assert error.detail == error.args[2]
    assert error.status is None
    assert error.attempts is None
    assert error.retry_after_ms is None
    assert error.category == "validation"
    assert not error.retryable
    restored = pickle.loads(pickle.dumps(error))
    assert restored.args == error.args
    assert restored.category == error.category
    with pytest.raises(AttributeError):
        error.category = "busy"


@pytest.mark.parametrize("rebuild", [copy.copy, copy.deepcopy, lambda error: pickle.loads(pickle.dumps(error))])
def test_sag_error_string_and_reconstruction_keep_message_and_details(rebuild):
    detail = json.dumps({"id": "decidr.http", "message": "provider failed (http.network)",
                         "cause": {"id": "http.network", "message": "connection closed"},
                         "category": "network", "attempts": 2})
    error = sc_agent.SagError("decidr.http", "provider failed (http.network)", detail)
    expected = "decidr.http: provider failed (http.network)"
    assert str(error) == expected
    restored = rebuild(error)
    assert restored is not error
    assert type(restored) is sc_agent.SagError
    assert str(restored) == expected
    assert restored.args == (error.id, error.message, detail)
    assert restored.detail == detail
    assert (restored.category, restored.attempts, restored.retryable) == ("network", 2, True)


def test_stored_category_from_real_loopback_error(env, args):
    line = json.loads(sc_agent.send('{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}', **args))
    detail = json.dumps(line["error"])
    assert sc_agent.category(detail) == "other"
    assert not sc_agent.retryable(detail)


@pytest.mark.parametrize(
    ("status", "expected_category", "expected_retryable"),
    [(401, "auth", False), (429, "busy", True)],
)
def test_stored_category_and_retryable_from_status_loopback(
    env, args, status, expected_category, expected_retryable
):
    """Status-mode loopback errors preserve their stored category and retryable outcome."""
    request = '{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}'
    line = json.loads(sc_agent.send(request, **args))
    detail = json.dumps(line["error"])
    assert sc_agent.category(detail) == expected_category
    assert sc_agent.retryable(detail) is expected_retryable

    with pytest.raises(sc_agent.SagError) as caught:
        sc_agent.models(runner="dec", config_dir=args["config_dir"])
    error = caught.value
    assert error.status == status
    restored = pickle.loads(pickle.dumps(error))
    assert restored.status == status
    assert restored.category == expected_category
    assert restored.retryable is expected_retryable


def test_category_fallback_for_empty_and_legacy_details():
    empty = sc_agent.SagError("x", "y", "{}")
    assert empty.status is None
    assert empty.category == "other"
    assert empty.retryable is False

    legacy_detail = (
        pathlib.Path(__file__).resolve().parents[3]
        / "crates/sc-agent-types/tests/fixtures/error-detail-v0.1.0.json"
    ).read_text()
    assert sc_agent.category(legacy_detail) == "other"
    assert sc_agent.retryable(legacy_detail) is False


@pytest.mark.parametrize("category,retryable", [("busy", True), ("network", True), (None, False)])
def test_sag_error_constructor_caches_fields_and_rebuilds(monkeypatch, category, retryable):
    """Each construction parses once; properties and reconstruction keep the contract."""
    fields = {"id": "sample.error", "message": "sample", "status": 429,
              "attempts": 3, "retry_after_ms": 2001}
    if category is not None:
        fields["category"] = category
    detail = json.dumps(fields)
    globals_ = sc_agent.SagError.__init__.__globals__
    parse = globals_["_sag_error_fields"]
    parsed = []

    def record(detail_json):
        parsed.append(detail_json)
        return parse(detail_json)

    monkeypatch.setitem(globals_, "_sag_error_fields", record)
    error = sc_agent.SagError("sample.error", "sample", detail)
    expected = {"id": "sample.error", "message": "sample", "detail": detail,
                "status": 429, "attempts": 3, "retry_after_ms": 2001,
                "category": category or "other", "retryable": retryable}
    for _ in range(2):
        assert error.args == ("sample.error", "sample", detail)
        assert {name: getattr(error, name) for name in expected} == expected
    assert parsed == [detail]
    for name in expected:
        with pytest.raises(AttributeError):
            setattr(error, name, None)
    for rebuild in [copy.copy, lambda error: pickle.loads(pickle.dumps(error))]:
        restored = rebuild(error)
        assert restored is not error
        assert type(restored) is sc_agent.SagError
        assert restored.args == error.args
        assert {name: getattr(restored, name) for name in expected} == expected
    assert parsed == [detail, detail, detail]


@pytest.mark.mode("error")
def test_outcome_category_matches_returned_error(outcome_args, outcome_call):
    request = '{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}'
    outcome = json.loads(outcome_call(sc_agent.send, request, **outcome_args))
    if "error" in outcome:
        detail = outcome["error"]
        category = detail.get("category", "other")
        assert sc_agent.category(json.dumps(detail)) == category
        assert sc_agent.retryable(json.dumps(detail)) == (category in {"busy", "network"})
    else:
        assert "response" in outcome
