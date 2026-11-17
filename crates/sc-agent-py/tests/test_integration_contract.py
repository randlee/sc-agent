"""Check the live suite's collection and helper contract without sending requests."""
import ast
import inspect
import os
from pathlib import Path
import subprocess
import sys

from consumer_support import render_consumer


LIVE_SUITE = Path(__file__).resolve().parents[1] / "integration/test_providers.py"


def test_live_suite_collects():
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "--collect-only", "-q", str(LIVE_SUITE)],
        env={**os.environ, "SC_AGENT_INTEGRATION_PROVIDER": "typesafe",
             "SC_AGENT_TEST_TRAP_PLATFORM_KEYRING": "1"},
        capture_output=True, text=True, timeout=60,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "test_real_noul_query[noul-send]" in result.stdout
    assert "test_real_noul_query[noul-batch]" in result.stdout
    assert "test_real_invalid_key_auth_cancel_redaction" in result.stdout


def test_live_suite_render_calls_match_helper_signature():
    tree = ast.parse(LIVE_SUITE.read_text())
    signature = inspect.signature(render_consumer)
    calls = [node for node in ast.walk(tree)
             if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
             and node.func.id == "render_consumer"]
    assert calls, "live suite must exercise the rendered consumer"
    for call in calls:
        # Bind argument names, without evaluating expressions or running live tests.
        signature.bind(*[object() for arg in call.args],
                       **{keyword.arg: object() for keyword in call.keywords})
