"""Python exception attributes and stored error categories."""
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


@pytest.mark.live_outcome
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
