"""Actual rebuilt-wheel error paths with explicit dummy keys and a loopback runner.

No OS keychain or external provider is used; the suite's keychain trap stays armed.
"""
import json
import os
import pathlib
import re
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


@pytest.mark.live_outcome
@pytest.mark.mode("echo-key")
@pytest.mark.parametrize("provider", ["typesafe", "anthropic"])
@pytest.mark.parametrize("status", [401])
def test_invalid_environment_key_auth_cancels_and_redacts(
    request, tmp_path, monkeypatch, consumer, provider, status, record_property,
):
    """Existing echo-key redaction path, also manually selectable against real providers."""
    live = os.environ.get("SC_AGENT_LIVE") == "1"
    key = "invalid-environment-key-b12-auth-0001"
    monkeypatch.chdir(tmp_path)  # No repository .env may override the explicit invalid key.
    assert subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, timeout=30).returncode != 0
    if live:
        for name in list(os.environ):
            if name.endswith("_API_KEY") or name.startswith("SC_AGENT__"):
                monkeypatch.delenv(name)
        # Read the credential spelling from its sole owner, not a second Python catalog.
        source = pathlib.Path(__file__).resolve().parents[3] / "crates/sc-agent-secrets/src/vars.rs"
        variables = dict(re.findall(r'pub const ([A-Z_]+): &str = "([^"]+)";', source.read_text()))
        monkeypatch.setenv(variables[f"{provider.upper()}_API_KEY"], key)
        config = tmp_path / "config"
        config.mkdir()
        (config / "default.json").write_text(json.dumps({"max_sends": 1, "log_root": str(tmp_path / "logs")}))
        options = dict(model="jev" if provider == "typesafe" else "haiku", project="owner-check", agent_role="pytest", config_dir=str(config))
    else:
        request.getfixturevalue("env")
        config = request.getfixturevalue("config_dir")
        keyed_runner(config)
        monkeypatch.setenv(KEY_ENV, key)
        options = request.getfixturevalue("args")
    record_property("provider", provider if live else "local")
    record_property("model", options["model"])
    record_property("mode", "live" if live else "loopback")
    cancel = sc_agent.CancelToken()
    text, code = consumer.call(REQUEST, **options, abort_on=frozenset({"auth"}),
                               category_codes={"auth": "AUTH"}, cancel=cancel)
    detail = json.loads(text)["error"]
    assert detail["status"] == 401 and detail["category"] == "auth"
    assert code == "AUTH" and cancel.is_cancelled()
    cancelled, _ = consumer.call(REQUEST, **options, abort_on=frozenset({"auth"}),
                                 category_codes={}, cancel=cancel)
    assert json.loads(cancelled)["error"]["category"] == "cancelled"
    assert key not in text + cancelled
    assert all(key not in path.read_text() for path in tmp_path.rglob("*.jsonl"))
    record_property("result", "HTTP 401; category auth; token and follow-up cancelled; key absent from results/logs")
