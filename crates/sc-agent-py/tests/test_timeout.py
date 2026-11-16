"""Per-call timeout boundary: immediate loopback replies, no elapsed-time assertions."""
import json

import pytest
import sc_agent

NOUL = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})


@pytest.mark.parametrize("operation", [sc_agent.send, sc_agent.send_batch])
@pytest.mark.parametrize("seconds", [0, -1, 9.99, 3600.01, float("nan"), float("inf"), -float("inf")])
def test_invalid_timeout_is_typed_before_config_logs_or_network(env, args, operation, seconds, tmp_path):
    loopback, log_root = env
    request = [NOUL] if operation is sc_agent.send_batch else NOUL
    # This would fail config loading if option validation did not run first.
    config = tmp_path / "invalid-config"
    config.mkdir()
    (config / "default.json").write_text("not JSON")
    options = {**args, "config_dir": str(config)}
    with pytest.raises(sc_agent.SagError) as info:
        operation(request, **options, timeout_secs=seconds)
    ident, message, detail = info.value.args
    assert ident == "decidr.invalid_request_timeout"
    assert "timeout_secs" in message and "10..=3600" in message
    assert json.loads(detail)["id"] == ident
    assert loopback.requests == []
    assert not log_root.exists()


@pytest.mark.mode("ok")
@pytest.mark.parametrize("operation", [sc_agent.send, sc_agent.send_batch])
@pytest.mark.parametrize("seconds", [None, 10, 30.5, 3600.0])
def test_checked_optional_timeout_is_additive(env, args, operation, seconds):
    loopback, _ = env
    request = [NOUL] if operation is sc_agent.send_batch else NOUL
    result = operation(request, **args, timeout_secs=seconds)
    lines = result if isinstance(result, list) else [result]
    assert all("response" in json.loads(line) for line in lines)
    assert len(loopback.requests) == 1
