"""Layered binding and generated-client contracts over controlled files and loopback I/O."""
import json
import os
from pathlib import Path
import subprocess

import pytest
import sc_agent
import sc_observability as obs

from consumer_support import render_consumer

REQUEST = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})


def snapshot(root):
    return {p.relative_to(root): (p.read_bytes(), p.stat().st_mtime_ns) for p in root.rglob("*") if p.is_file()}


@pytest.mark.parametrize("source", ["file", "env"])
def test_resolved_config_reports_source_without_writes_or_requests(env, tmp_path, monkeypatch, source):
    runner = tmp_path / ".sc-agent/decidr.json"
    data = json.loads(runner.read_text())
    del data["log_root"]
    runner.write_text(json.dumps(data))
    expected = str(tmp_path / "configured")
    shared = tmp_path / ".sc-agent/sc-agent.json"
    shared.write_text(json.dumps({"log_root": expected}))
    origin = str(shared)
    if source == "env":
        monkeypatch.setenv("DECIDR_LOG_ROOT", expected)
        origin = "DECIDR_LOG_ROOT"
    before = snapshot(tmp_path)
    result = sc_agent.resolved_config()
    assert result["log_root"] == expected
    assert result["project"] is None
    assert result["sources"]["log_root"] == origin
    assert snapshot(tmp_path) == before
    assert env[0].requests == []


@pytest.mark.parametrize("batch", [False, True])
def test_missing_project_is_typed_before_logs_or_network(env, args, batch):
    operation = sc_agent.send_batch if batch else sc_agent.send
    options = {k:v for k,v in args.items() if k != "project"}
    with pytest.raises(sc_agent.SagError) as caught:
        operation([REQUEST] if batch else REQUEST, **options)
    error = caught.value
    assert error.id == "decidr.config" and error.category == "config"
    detail = json.loads(error.detail)
    assert detail["cause"]["id"] == "config.project_required"
    assert detail["cause"]["category"] == "config"
    assert all(place in detail["cause"]["message"] for place in ("--project", "SC_PROJECT", "sc-agent.json", "decidr.json"))
    assert env[0].requests == [] and not env[1].exists()


@pytest.mark.mode("ok")
def test_project_none_resolves_env_and_keeps_aged_sessions(env, args, monkeypatch):
    monkeypatch.setenv("SC_PROJECT", "phase-env")
    old = env[1] / "sessions/local/m5/work/role/old/sessions.jsonl"
    old.parent.mkdir(parents=True)
    old.write_bytes(b"old session retained forever\n")
    os.utime(old, ns=(0, 0))
    before = (old.read_bytes(), old.stat().st_mtime_ns)
    sc_agent.send(REQUEST, **{**args, "project": None})
    assert (old.read_bytes(), old.stat().st_mtime_ns) == before
    files = [p for p in env[1].rglob("sessions.jsonl") if p != old]
    assert len(files) == 1 and "phase-env" in files[0].parts
    row = json.loads(files[0].read_text())
    assert row["fields"]["project"] == "phase-env"
    assert (env[1] / "logs/decidr.log.jsonl").is_file()
    assert not list((env[1] / "logs").rglob("sessions.jsonl"))


@pytest.mark.parametrize("value,source", [("0", "DECIDR_WORKERS"), ("Phase Bad", "SC_PROJECT")])
def test_bad_environment_names_its_variable_without_io(env, monkeypatch, tmp_path, value, source):
    monkeypatch.setenv(source, value)
    before = snapshot(tmp_path)
    with pytest.raises(sc_agent.SagError) as caught:
        sc_agent.resolved_config()
    assert source in caught.value.detail
    assert snapshot(tmp_path) == before and env[0].requests == []


@pytest.mark.mode("ok")
@pytest.mark.parametrize("app_logger", [False, True])
def test_generated_client_uses_one_logger_and_fixed_role(env, args, tmp_path, monkeypatch, app_logger):
    output = tmp_path / "rendered"
    output.mkdir()
    client = render_consumer(output, monkeypatch=monkeypatch, client_name="plan-anchor", initialize_logger=False)
    logger = None
    root = env[1]
    service = "plan-anchor"
    if app_logger:
        root = tmp_path / "app"
        service = "app"
        logger = obs.create_logger(obs.LoggerConfig(service=service, log_root=str(root))).value
    result = client.call_result(REQUEST, model="dec", project="phase-client", abort_on=frozenset(), logger=logger)
    assert result["ok"]
    used = client.client_logger(logger=logger)
    if app_logger:
        assert used is logger
    else:
        assert used is client.client_logger()
    assert used.flush().kind == "ok"
    records = [json.loads(line) for line in (root / "logs" / f"{service}.log.jsonl").read_text().splitlines()]
    assert any(row["action"] == "decidr.send" for row in records)
    assert not (env[1] / "logs/decidr.log.jsonl").exists()
    assert not (env[1] / "logs/sc-agent-py.log.jsonl").exists()
    sessions = list(env[1].rglob("sessions.jsonl"))
    assert len(sessions) == 1
    assert json.loads(sessions[0].read_text())["fields"]["agent_role"] == "plan-anchor"
    for operation in (client.call_result, client.call_batch_results):
        with pytest.raises(TypeError, match="agent_role"):
            operation(REQUEST, model="dec", agent_role="other", abort_on=frozenset())
    assert used.log(obs.LogEvent(level="info", target="app", action="still.open", fields={})).kind == "ok"
    assert used.shutdown().kind == "ok"
