"""Check the live suite's collection and helper contract without sending requests."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest
import sc_agent

from test_consumer_results import wire


LIVE_SUITE = Path(__file__).resolve().parents[1] / "integration/test_providers.py"


def test_live_suite_collects():
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "--collect-only", "-q", str(LIVE_SUITE)],
        stdin=subprocess.DEVNULL, env={**os.environ, "SC_AGENT_INTEGRATION_PROVIDER": "typesafe",
             "SC_AGENT_TEST_TRAP_PLATFORM_KEYRING": "1"},
        capture_output=True, text=True, timeout=60,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "test_real_noul_query[noul-send]" in result.stdout
    assert "test_real_noul_query[noul-batch]" in result.stdout
    assert "test_real_invalid_key_auth_cancel_redaction" in result.stdout


@pytest.mark.parametrize("provider", ["typesafe", "anthropic", "openai"])
def test_live_cases_use_rendered_consumer_contract(tmp_path, monkeypatch, provider):
    """Execute live assertions through the real client; only wheel sends are controlled.

    Python binds every helper/client call against its actual signature. Resolving
    client attributes on the rendered module catches removed API names as well.
    This is offline contract coverage, not evidence of provider behavior.
    """
    monkeypatch.setenv("SC_AGENT_INTEGRATION_PROVIDER", provider)
    monkeypatch.setattr(sys, "path", sys.path.copy())
    spec = importlib.util.spec_from_file_location("live_providers_contract", LIVE_SUITE)
    suite = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(suite)
    observed = []

    def send(request, *, cancel, general_log=None, **kwargs):
        assert request == suite.REQUEST
        assert kwargs["model"] == suite.MODEL
        observed.append("send")
        if general_log is not None:
            general_log({"fields": {"provider": provider}})
        if cancel.is_cancelled():
            return wire(error={"id": "decision.cancelled", "category": "cancelled", "message": "cancelled"})
        if os.environ.get(f"{provider.upper()}_API_KEY", "").startswith("invalid-integration-key-"):
            return wire(error={"id": "decision.auth", "category": "auth", "status": 401, "message": "invalid key"})
        return wire()

    def send_batch(requests, *, on_result, general_log=None, **kwargs):
        assert requests == [suite.REQUEST] * 2
        assert kwargs["model"] == suite.MODEL
        observed.append("send_batch")
        general_log({"fields": {"provider": provider}})
        lines = [wire(position) for position in range(len(requests))]
        for line in reversed(lines):
            on_result(line)
        return lines

    monkeypatch.delenv(f"{provider.upper()}_API_KEY", raising=False)
    monkeypatch.setattr(sc_agent, "send", send)
    monkeypatch.setattr(sc_agent, "send_batch", send_batch)
    properties = {}
    operations = next(mark.args[1] for mark in suite.test_real_noul_query.pytestmark
                      if mark.name == "parametrize" and mark.args[0] == "operation")
    for operation in operations:
        root = tmp_path / operation
        root.mkdir()
        suite.test_real_noul_query(root, monkeypatch, properties.__setitem__, operation)
        assert json.loads(properties["result"]) == [{"type": "noul", "noul": 0.5}] * (2 if operation == "call_batch_results" else 1)
    root = tmp_path / "auth"
    root.mkdir()
    suite.test_real_invalid_key_auth_cancel_redaction(root, monkeypatch, properties.__setitem__)
    assert observed == ["send", "send_batch", "send", "send"]
    assert properties["provider"] == provider
    assert properties["model"] == suite.MODEL
