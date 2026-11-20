"""Live cases for just integration; offline contract tests control the wheel send boundary."""
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


def options(tmp_path, record_property, monkeypatch):
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("USERPROFILE", str(tmp_path))
    monkeypatch.chdir(tmp_path)
    config = tmp_path / ".sc-agent"
    config.mkdir()
    (config / "decidr.json").write_text(json.dumps({"max_sends": 1}))
    record_property("provider", PROVIDER)
    record_property("model", MODEL)
    record_property("mode", "live")
    return dict(model=MODEL, project="integration")


@pytest.mark.parametrize("operation", ["call_result", "call_batch_results"], ids=["noul-send", "noul-batch"])
def test_real_noul_query(tmp_path, monkeypatch, record_property, operation):
    client = render_consumer(tmp_path, monkeypatch=monkeypatch, models=[MODEL])
    result = getattr(client, operation)([REQUEST] * 2 if operation == "call_batch_results" else REQUEST,
                 **options(tmp_path, record_property, monkeypatch), abort_on=frozenset(), category_codes={},
                 cancel=sc_agent.CancelToken(), timeout_secs=120)
    assert result["ok"], result.get("error")
    rows = result["value"] if operation == "call_batch_results" else [result]
    assert len(rows) == (2 if operation == "call_batch_results" else 1)
    answers = []
    for row in rows:
        assert row["ok"], row.get("error")
        answer = row["value"]["q"]
        assert isinstance(answer, client.NoulAnswer)
        assert isinstance(answer.noul, (int, float)) and 0 <= answer.noul <= 1
        answers.append({"type": "noul", "noul": answer.noul})
    assert client.client_logger().flush().kind == "ok"
    records = [json.loads(line) for line in (tmp_path / "client-logs/logs/plan-anchor.log.jsonl").read_text().splitlines()]
    selected = [row["fields"]["provider"] for row in records if row.get("fields", {}).get("provider")]
    assert selected and set(selected) == {PROVIDER}
    record_property("result", json.dumps(answers))


def test_real_invalid_key_auth_cancel_redaction(tmp_path, monkeypatch, record_property):
    monkeypatch.chdir(tmp_path)  # A repository .env must not replace the invalid key.
    assert subprocess.run(["git", "rev-parse", "--show-toplevel"], stdin=subprocess.DEVNULL, capture_output=True, timeout=30).returncode != 0
    client = render_consumer(tmp_path, monkeypatch=monkeypatch, models=[MODEL])
    source = ROOT / "crates/sc-agent-secrets/src/vars.rs"
    variables = dict(re.findall(r'pub const ([A-Z_]+): &str = "([^"]+)";', source.read_text()))
    key = "invalid-integration-key-0000000001"
    monkeypatch.setenv(variables[f"{PROVIDER.upper()}_API_KEY"], key)
    cancel = sc_agent.CancelToken()
    kwargs = options(tmp_path, record_property, monkeypatch)
    result = client.call_result(REQUEST, **kwargs, abort_on=frozenset({"auth"}),
                                category_codes={"auth": "AUTH"}, cancel=cancel, timeout_secs=120)
    assert not result["ok"]
    error = result["error"]
    assert isinstance(error, client.ClientError)
    wire_error = json.loads(result["line"])["error"]
    assert wire_error["category"] == "auth" and wire_error["status"] == 401
    assert error.data == wire_error and error.status == 401
    assert error.code == "AUTH" and cancel.is_cancelled()
    followup = client.call_result(REQUEST, **kwargs, abort_on=frozenset({"auth"}),
                                 category_codes={}, cancel=cancel, timeout_secs=120)
    assert not followup["ok"] and followup["error"].data["category"] == "cancelled"
    assert followup["error"].code == "CANCELED"
    output = json.dumps([error.to_json(), followup["error"].to_json(), result["line"], followup["line"]])
    assert key not in output
    assert all(key not in path.read_text() for path in tmp_path.rglob("*.jsonl"))
    record_property("result", "HTTP401/auth; token and follow-up cancelled; no key in results/logs")
