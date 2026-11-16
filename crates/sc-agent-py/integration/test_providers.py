"""Real providers only. Run with just integration, never as part of unit tests."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

import pytest
import sc_agent

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "crates/sc-agent-py/tests"))
from consumer_support import render_consumer

REQUEST = json.dumps({"model": "jev-latest", "state": "Two plus two equals four.",
                      "questions": {"q": {"type": "noul", "instructions": "Is the statement mathematically correct?"}}})

PROVIDER = os.environ.get("SC_AGENT_INTEGRATION_PROVIDER")
if PROVIDER not in {"typesafe", "anthropic", "openai"}:
    raise pytest.UsageError("run this real-provider suite with just integration")
MODEL = {"typesafe": "jev", "anthropic": "haiku", "openai": "luna"}[PROVIDER]


def options(tmp_path, record_property):
    config = tmp_path / "config"
    config.mkdir()
    (config / "default.json").write_text(json.dumps({"max_sends": 1, "log_root": str(tmp_path / "logs")}))
    record_property("provider", PROVIDER)
    record_property("model", MODEL)
    record_property("mode", "live")
    return dict(model=MODEL, project="integration", agent_role="pytest", config_dir=str(config))


@pytest.mark.parametrize("operation", ["call", "call_batch"], ids=["noul-send", "noul-batch"])
def test_real_noul_query(tmp_path, record_property, operation):
    client = render_consumer(tmp_path, models=[MODEL])
    selected = []

    def log(record):
        if record.get("fields", {}).get("provider"):
            selected.append(record["fields"]["provider"])

    result = getattr(client, operation)([REQUEST] * 2 if operation == "call_batch" else REQUEST,
                 **options(tmp_path, record_property), abort_on=frozenset(), category_codes={},
                 cancel=sc_agent.CancelToken(), general_log=log, timeout_secs=120)
    rows = result if operation == "call_batch" else [result]
    answers = []
    for text, code in rows:
        line = json.loads(text)
        error = line.get("error", {})
        assert "response" in line, (error.get("id"), error.get("category"), error.get("status"))
        assert code is None
        answer = line["response"]["answers"]["q"]
        assert answer["type"] == "noul"
        assert isinstance(answer["noul"], (int, float)) and 0 <= answer["noul"] <= 1
        answers.append(answer)
    assert selected and set(selected) == {PROVIDER}
    record_property("result", json.dumps(answers))


def test_real_invalid_key_auth_cancel_redaction(tmp_path, monkeypatch, record_property):
    monkeypatch.chdir(tmp_path)  # A repository .env must not replace the invalid key.
    assert subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, timeout=30).returncode != 0
    client = render_consumer(tmp_path, models=[MODEL])
    source = ROOT / "crates/sc-agent-secrets/src/vars.rs"
    variables = dict(re.findall(r'pub const ([A-Z_]+): &str = "([^"]+)";', source.read_text()))
    key = "invalid-integration-key-0000000001"
    monkeypatch.setenv(variables[f"{PROVIDER.upper()}_API_KEY"], key)
    cancel = sc_agent.CancelToken()
    kwargs = options(tmp_path, record_property)
    text, code = client.call(REQUEST, **kwargs, abort_on=frozenset({"auth"}),
                            category_codes={"auth": "AUTH"}, cancel=cancel, timeout_secs=120)
    error = json.loads(text)["error"]
    assert error["category"] == "auth" and error["status"] == 401
    assert code == "AUTH" and cancel.is_cancelled()
    followup, _ = client.call(REQUEST, **kwargs, abort_on=frozenset({"auth"}), category_codes={}, cancel=cancel, timeout_secs=120)
    assert json.loads(followup)["error"]["category"] == "cancelled"
    assert key not in text + followup
    assert all(key not in path.read_text() for path in tmp_path.rglob("*.jsonl"))
    record_property("result", "HTTP401/auth; token and follow-up cancelled; no key in results/logs")
