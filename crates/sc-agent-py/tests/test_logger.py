"""Caller-owned sc-observability 1.5.0 Logger injection through the installed wheel."""
import json
from unittest.mock import patch

import pytest
import sc_agent
import sc_observability as obs

NOUL = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})


def ok(result):
    assert isinstance(result, obs.Ok), result
    return result.value


@pytest.mark.mode("ok")
@pytest.mark.parametrize("operation", ["send", "send_batch"])
@pytest.mark.parametrize("injection", ["general", "session", "both"])
def test_same_caller_owned_logger_receives_python_and_sc_agent_records(env, args, tmp_path, operation, injection):
    _, default_root = env
    caller_root = tmp_path / "caller"
    logger = ok(obs.create_logger(obs.LoggerConfig(service="consumer", log_root=str(caller_root))))
    ok(logger.log(obs.LogEvent(level="info", target="consumer", action="before", fields={"owner": "python"})))
    injected = {}
    if injection in ("general", "both"):
        injected["general_log"] = logger
    if injection in ("session", "both"):
        injected["session_log"] = logger
    seen = []
    original_log = logger.log
    def observe(event):
        seen.append(event)
        return original_log(event)
    try:
        with patch.object(logger, "log", side_effect=observe), patch.object(logger, "flush", wraps=logger.flush) as flush, patch.object(logger, "shutdown", wraps=logger.shutdown) as shutdown:
            result = sc_agent.send(NOUL, **args, **injected) if operation == "send" else sc_agent.send_batch([NOUL, NOUL], **args, **injected)
            lines = [result] if operation == "send" else result
            assert all("response" in json.loads(line) for line in lines)
            assert flush.call_count == 0, "caller retains flush ownership"
            assert shutdown.call_count == 0, "caller retains shutdown ownership"
        assert seen and all(isinstance(event, obs.LogEvent) for event in seen)
        ok(logger.log(obs.LogEvent(level="info", target="consumer", action="after", fields={"owner": "python"})))
        ok(logger.flush())
        assert ok(logger.health()).logging.writer_state == "running"
        events = ok(logger.query(obs.LogQuery(limit=1000, order="oldest_first"))).events
        actions = [event.action for event in events]
        assert {"before", "after"}.issubset(actions)
        assert all(event.service == "consumer" for event in events)
        if injection in ("general", "both"):
            assert "decidr.send" in actions
            assert "http.send" in actions
            assert not (default_root / "logs/sc-agent-py.log.jsonl").exists()
        else:
            assert (default_root / "logs/sc-agent-py.log.jsonl").exists()
            assert "decidr.send" not in actions
        sessions = [event for event in events if event.target == "sc_agent.session"]
        expected = 1 if operation == "send" else 2
        if injection in ("session", "both"):
            assert len(sessions) == expected
            assert sorted(event.fields["position"].value for event in sessions) == list(range(expected))
            assert {event.fields["session_id"].value for event in sessions} == {json.loads(line)["session_id"] for line in lines}
            assert not list(default_root.rglob("sessions.jsonl"))
        else:
            assert not sessions
            files = list(default_root.rglob("sessions.jsonl"))
            assert len(files) == 1
            assert len(files[0].read_text().splitlines()) == expected
    finally:
        ok(logger.shutdown())
    assert isinstance(logger.log(obs.LogEvent(level="info", target="consumer", action="closed")), obs.Err)


@pytest.mark.mode("ok")
def test_closed_caller_logger_reports_lost_rows_and_preserves_decision(env, args, tmp_path):
    logger = ok(obs.create_logger(obs.LoggerConfig(service="consumer", log_root=str(tmp_path / "caller"))))
    ok(logger.shutdown())
    with pytest.warns(RuntimeWarning) as warnings:
        result = sc_agent.send(NOUL, **args, general_log=logger, session_log=logger)
    assert "response" in json.loads(result)
    messages = "\n".join(str(warning.message) for warning in warnings)
    assert "log.rows_lost" in messages
    assert "SC_OBSERVABILITY_BINDING_CLOSED" in messages
