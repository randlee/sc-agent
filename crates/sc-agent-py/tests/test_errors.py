"""Python exception attributes and stored error categories."""
import json
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
