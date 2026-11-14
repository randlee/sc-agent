"""Actual rebuilt-wheel error paths with explicit dummy keys and a loopback runner.

No OS keychain or external provider is used; the suite's keychain trap stays armed.
"""
import json
import subprocess

import pytest
import sc_agent

REQUEST = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})
KEY = "sk-python-native-redaction-controlled-77"
KEY_ENV = "NATIVE_REDACTION_TEST_KEY"


def keyed_runner(config_dir):
    path = config_dir / "default.json"
    config = json.loads(path.read_text())
    config["local_runners"]["dec"]["key_env"] = KEY_ENV
    path.write_text(json.dumps(config))
    return path, config


@pytest.mark.mode("echo-key")
@pytest.mark.parametrize("status", [200, 409])  # fixture sets max_sends=1: no retry timing
@pytest.mark.parametrize("source", ["env", "dotenv"])
@pytest.mark.parametrize("operation", ["send", "send_batch"])
def test_echoed_key_is_redacted_in_actual_python_decision_results(env, args, config_dir, tmp_path, monkeypatch, source, operation, status):
    runner, log_root = env
    keyed_runner(config_dir)
    monkeypatch.delenv(KEY_ENV, raising=False)
    if source == "env":
        monkeypatch.setenv(KEY_ENV, KEY)
    else:
        subprocess.run(["git", "init", "--quiet", str(tmp_path)], check=True, capture_output=True, timeout=30)
        (tmp_path / ".env").write_text(f"{KEY_ENV}={KEY}\n")
        monkeypatch.chdir(tmp_path)
    text = sc_agent.send(REQUEST, **args) if operation == "send" else sc_agent.send_batch([REQUEST], **args)[0]
    error = json.loads(text)["error"]
    assert error["id"] == ("decision.response_parse" if status == 200 else "decision.api_status")
    assert "[redacted]" in error["message"]
    assert KEY not in text
    assert len(runner.requests) == 1
    assert runner.requests[0]["headers"]["authorization"] == f"Bearer {KEY}"
    logs = list(log_root.rglob("*.jsonl"))
    assert logs
    assert all(KEY not in path.read_text() for path in logs)


@pytest.mark.parametrize("failure", ["config", "credential"])
@pytest.mark.parametrize("operation", ["send", "send_batch"])
def test_python_pre_send_exception_detail_never_contains_credential_value(env, args, config_dir, monkeypatch, failure, operation):
    runner, _ = env
    path, config = keyed_runner(config_dir)
    monkeypatch.setenv(KEY_ENV, KEY + " trailing")
    if failure == "config":
        config["max_sends"] = 0
        path.write_text(json.dumps(config))
    with pytest.raises(sc_agent.SagError) as caught:
        if operation == "send":
            sc_agent.send(REQUEST, **args)
        else:
            sc_agent.send_batch([REQUEST], **args)
    identity, message, encoded = caught.value.args
    detail = json.loads(encoded)
    assert identity == ("decidr.config" if failure == "config" else "decidr.secrets")
    assert detail["id"] == identity
    assert detail["message"] == message
    assert detail["cause"]["id"]
    if failure == "credential":
        assert detail["cause"]["id"] == "secrets.key_malformed"
        assert "whitespace" in detail["cause"]["message"]
    assert KEY not in str(caught.value.args)
    assert runner.requests == []


@pytest.mark.mode("echo-model-key")
def test_post_credential_decision_error_stays_redacted_in_actual_sagerror(env, config_dir, monkeypatch):
    keyed_runner(config_dir)
    monkeypatch.setenv(KEY_ENV, KEY)
    with pytest.raises(sc_agent.SagError) as caught:
        sc_agent.models(runner="dec", config_dir=str(config_dir))
    identity, message, encoded = caught.value.args
    detail = json.loads(encoded)
    assert identity == "decidr.decision"
    assert detail["id"] == identity
    assert detail["message"] == message
    assert detail["cause"]["id"] == "decision.response_parse"
    assert "[redacted]" in detail["cause"]["message"]
    assert KEY not in str(caught.value.args)
