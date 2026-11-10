"""The sc-agent Python API over the maturin build, against a loopback runner.

SC_AGENT_CONFIG_DIR (never HOME) is a tempdir whose default.json names the loopback runner
`dec`; no test reaches an external network. Each test names the acceptance line it proves.
Orderings are forced with events; every wait is bounded by HANG_GUARD and no assertion
depends on elapsed time.
"""

from __future__ import annotations

import _thread
import json
import pathlib
import re
import subprocess
import threading
import os
import warnings

import pytest

import sc_agent
from conftest import HANG_GUARD

NOUL = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})
PART = json.dumps(
    {
        "model": "jev-latest",
        "state": {"door": "a"},
        "assets": {"a": {"type": "image", "part": "img1"}},
        "questions": {"q": {"type": "noul"}},
    }
)
PNG = b"\x89PNG\r\n\x1a\n" + b"\x00" * 16
ARGS = dict(model="dec", project="proj", agent_role="role")


def session_files(log_root: pathlib.Path) -> list[pathlib.Path]:
    return sorted(log_root.rglob("sessions.jsonl")) if log_root.exists() else []


def session_rows(log_root: pathlib.Path) -> list[dict]:
    return [json.loads(line) for path in session_files(log_root) for line in path.read_text().splitlines()]


def error_of(info: pytest.ExceptionInfo) -> tuple[str, dict]:
    ident, _message, detail = info.value.args
    return ident, json.loads(detail)


def test_every_decide_operation_has_its_python_function(decide_bin):
    """AC: ANC-006."""
    text = subprocess.run(
        [decide_bin, "--help"], check=True, capture_output=True, text=True, timeout=HANG_GUARD
    ).stdout
    commands = re.findall(r"^\s{2}(\w+)\s", text.split("Commands:")[1].split("Options:")[0], re.M)
    commands = [name for name in commands if name != "help"]
    assert sorted(commands) == ["doctor", "models", "send"]
    batch = subprocess.run(
        [decide_bin, "send", "--help"], check=True, capture_output=True, text=True, timeout=HANG_GUARD
    ).stdout
    assert "--batch" in batch
    operations = {"send": "send", "models": "models", "doctor": "doctor", "send --batch": "send_batch"}
    for operation, function in operations.items():
        assert callable(getattr(sc_agent, function, None)), operation


@pytest.mark.mode("ok")
def test_send_and_send_batch_return_the_decided_response(env):
    """REQ-SAG-0501 (successful path), REQ-SAG-0007: a runner that answers gives send() one
    result line with the response, its session ULID and position 0, the probability
    unrounded; send_batch() gives one such line per request, positions 0..n-1."""
    loopback, log_root = env
    text = sc_agent.send(NOUL, **ARGS)
    line = json.loads(text)
    assert "error" not in line
    assert line["position"] == 0
    assert len(line["session_id"]) == 26
    assert line["response"]["answers"]["q"] == {"type": "noul", "noul": 0.123456789}
    assert '"noul":0.123456789' in text
    assert set(line["digests"]) == {"q"}
    lines = [json.loads(item) for item in sc_agent.send_batch([NOUL] * 3, **ARGS)]
    assert sorted(item["position"] for item in lines) == [0, 1, 2]
    assert all(item["response"]["answers"]["q"]["noul"] == 0.123456789 for item in lines)
    assert len(loopback.requests) == 4
    rows = session_rows(log_root)
    assert len(rows) == 4
    assert all(row["fields"]["outcome"] == "ok" for row in rows), rows


def test_a_cancelled_token_stops_send_and_send_batch(env):
    """AC: REQ-SAG-0104 AC2."""
    loopback, _log_root = env
    token = sc_agent.CancelToken()
    token.cancel()
    assert token.is_cancelled()
    line = json.loads(sc_agent.send(NOUL, cancel=token, **ARGS))
    assert line["error"]["id"] == "decision.cancelled"
    lines = sc_agent.send_batch([NOUL, NOUL], cancel=token, **ARGS)
    assert [json.loads(item)["error"]["id"] for item in lines] == ["decision.cancelled"] * 2
    assert loopback.requests == []


def test_an_invalid_project_is_refused(env):
    """AC: REQ-SAG-0004."""
    loopback, log_root = env
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send(NOUL, model="dec", project="Phase B", agent_role="role")
    ident, detail = error_of(info)
    assert ident == "decide.log"
    assert detail["cause"]["id"] == "log.invalid_slug"
    assert loopback.requests == []
    assert session_files(log_root) == []


def test_cli_and_python_rows_carry_the_same_identity(env, decide_bin, tmp_path):
    """AC: REQ-SAG-0004 AC3."""
    loopback, log_root = env
    request = tmp_path / "request.json"
    request.write_text(NOUL)
    child = subprocess.run(
        [decide_bin, "send", "--model", "dec", "--project", "proj", "--agent-role", "role", str(request)],
        capture_output=True,
        text=True,
        env=dict(os.environ),  # carries SC_AGENT_CONFIG_DIR from the fixture
        timeout=HANG_GUARD,
    )
    assert child.returncode == 1, child.stderr  # the loopback answers a fixed error
    assert json.loads(child.stdout)["error"]["id"] == "decision.api_status"
    line = json.loads(sc_agent.send(NOUL, **ARGS))
    assert line["error"]["id"] == "decision.api_status"
    rows = session_rows(log_root)
    assert len(rows) == 2
    identities = {(row["fields"]["project"], row["fields"]["agent_role"]) for row in rows}
    assert identities == {("proj", "role")}
    assert len(loopback.requests) == 2


def test_injected_loggers_receive_the_records_and_write_no_file(env):
    """AC: REQ-SAG-0008 Python."""
    _loopback, log_root = env
    records: list[dict] = []
    rows: list[dict] = []
    sc_agent.send(NOUL, general_log=records.append, session_log=rows.append, **ARGS)
    assert "decide.send" in [record["action"] for record in records]
    assert "http.send" in [record["action"] for record in records]
    assert len(rows) == 1 and rows[0]["action"] == "session.row"
    assert list(rows[0]["fields"])[:4] == ["session_id", "repo", "project", "agent_role"]
    assert not (log_root / "logs" / "sc-agent-py.log.jsonl").exists()
    assert session_files(log_root) == []
    models: list[dict] = []
    listed = json.loads(sc_agent.models(runner="dec", general_log=models.append))
    assert listed["models"][0]["name"] == "jev-latest"
    assert "decide.models" in [record["action"] for record in models]


def test_only_a_session_log_callable_keeps_the_default_general_log(env):
    """AC: REQ-SAG-0008 AC4."""
    _loopback, log_root = env
    rows: list[dict] = []
    sc_agent.send(NOUL, session_log=rows.append, **ARGS)
    assert len(rows) == 1
    general = log_root / "logs" / "sc-agent-py.log.jsonl"
    assert general.exists()
    assert "decide.send" in general.read_text()
    assert session_files(log_root) == []


def test_only_a_general_log_callable_keeps_the_default_session_log(env):
    """AC: REQ-SAG-0008 AC5."""
    _loopback, log_root = env
    records: list[dict] = []
    sc_agent.send(NOUL, general_log=records.append, **ARGS)
    assert "decide.send" in [record["action"] for record in records]
    files = session_files(log_root)
    assert len(files) == 1
    assert files[0].parts[-3:] == ("role", "proj", "sessions.jsonl")
    assert not (log_root / "logs" / "sc-agent-py.log.jsonl").exists()


@pytest.mark.mode("hang")
def test_keyboard_interrupt_cancels_drains_and_reraises(env):
    """AC: REQ-SAG-0011 Python.

    The interrupt is raised once the runner holds a request it never answers, so the drained
    results can only come from the cancel, not from the transport. A guard cancels the token
    if KeyboardInterrupt is never seen, so a broken build fails instead of hanging.
    """
    loopback, log_root = env
    lines: list[str] = []
    token = sc_agent.CancelToken()
    returned = threading.Event()

    def interrupt() -> None:
        if loopback.started.wait(HANG_GUARD):
            _thread.interrupt_main()
        if not returned.wait(HANG_GUARD):
            token.cancel()

    thread = threading.Thread(target=interrupt)
    thread.start()
    try:
        with pytest.raises(KeyboardInterrupt):
            sc_agent.send_batch([NOUL] * 3, cancel=token, on_result=lines.append, **ARGS)
    finally:
        returned.set()
        thread.join(HANG_GUARD)
    assert not loopback.release.is_set(), "the runner never answered"
    assert token.is_cancelled()
    assert len(lines) == 3
    assert all(json.loads(line)["error"]["id"] == "decision.cancelled" for line in lines)
    assert len(session_rows(log_root)) == 3


def test_a_raising_log_callable_never_changes_the_result(env):
    """AC: REQ-SAG-0008 (QA-003). A general_log or session_log callable that raises makes its
    writes refused writes: the send returns its normal result, and the lost rows are reported
    (as a RuntimeWarning naming log.rows_lost) instead of being dropped silently."""

    def refuse(_record: dict) -> None:
        raise ValueError("this callable refuses every record")

    with pytest.warns(RuntimeWarning, match="log.rows_lost"):
        line = json.loads(sc_agent.send(NOUL, general_log=refuse, **ARGS))
    assert line["error"]["id"] == "decision.api_status"  # the loopback's fixed error, as always

    with pytest.warns(RuntimeWarning, match="log.rows_lost"):
        line = json.loads(sc_agent.send(NOUL, session_log=refuse, **ARGS))
    assert line["error"]["id"] == "decision.api_status"

    with warnings.catch_warnings():
        warnings.simplefilter("error")  # a callable that accepts every record warns nothing
        sc_agent.send(NOUL, general_log=lambda _record: None, session_log=lambda _record: None, **ARGS)


def test_an_on_result_exception_cancels_drains_and_reraises(env):
    """AC: REQ-SAG-0011 Python (on_result)."""
    seen: list[str] = []

    def explode(line: str) -> None:
        seen.append(line)
        raise ValueError("stop here")

    token = sc_agent.CancelToken()
    with pytest.raises(ValueError, match="stop here"):
        sc_agent.send_batch([NOUL] * 5, cancel=token, on_result=explode, **ARGS)
    assert token.is_cancelled()
    assert len(seen) == 1


def test_images_and_long_context_reach_the_runner(env):
    """AC: REQ-SAG-0501 images."""
    loopback, _log_root = env
    sc_agent.send(PART, images={"img1": PNG}, long_context=True, **ARGS)
    (request,) = loopback.requests
    assert request["headers"]["content-type"].startswith("multipart/form-data; boundary=")
    assert request["headers"]["typesafe-danger-zone-long-context"] == "true"
    assert b'name="img1"' in request["body"]
    assert PNG in request["body"]
    sc_agent.send_batch([(PART, {"img1": PNG}, True)], **ARGS)
    assert b'name="img1"' in loopback.requests[1]["body"]


def test_input_limits_raise_before_any_send(env):
    """AC: REQ-SAG-0014."""
    loopback, _log_root = env
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send_batch([NOUL] * 10_001, **ARGS)
    ident, detail = error_of(info)
    assert ident == "decide.limit_exceeded" and "batch_lines" in detail["message"]
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send(" " * (9 * 1024 * 1024), **ARGS)
    ident, detail = error_of(info)
    assert ident == "decide.limit_exceeded" and "request_bytes" in detail["message"]
    assert loopback.requests == []
