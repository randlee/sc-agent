"""The sc-agent Python API over the maturin build, against a loopback runner.

HOME is a tempdir whose default.json names the loopback runner `dec`; no test reaches an
external network. Each test names the acceptance line it proves.
"""

from __future__ import annotations

import _thread
import json
import pathlib
import re
import subprocess
import threading
import time

import pytest

import sc_agent

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
    text = subprocess.run([decide_bin, "--help"], check=True, capture_output=True, text=True).stdout
    commands = re.findall(r"^\s{2}(\w+)\s", text.split("Commands:")[1].split("Options:")[0], re.M)
    commands = [name for name in commands if name != "help"]
    assert sorted(commands) == ["doctor", "models", "send"]
    batch = subprocess.run([decide_bin, "send", "--help"], check=True, capture_output=True, text=True).stdout
    assert "--batch" in batch
    operations = {"send": "send", "models": "models", "doctor": "doctor", "send --batch": "send_batch"}
    for operation, function in operations.items():
        assert callable(getattr(sc_agent, function, None)), operation


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
        env={"HOME": str(tmp_path), "PATH": __import__("os").environ.get("PATH", "")},
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
    """AC: REQ-SAG-0011 Python."""
    loopback, log_root = env
    lines: list[str] = []
    sent_at: list[float] = []

    def interrupt() -> None:
        loopback.started.wait()
        sent_at.append(time.monotonic())
        _thread.interrupt_main()

    thread = threading.Thread(target=interrupt)
    thread.start()
    token = sc_agent.CancelToken()
    with pytest.raises(KeyboardInterrupt):
        sc_agent.send_batch([NOUL] * 3, cancel=token, on_result=lines.append, **ARGS)
    raised_at = time.monotonic()
    thread.join()
    assert raised_at - sent_at[0] < 2.0
    assert token.is_cancelled()
    assert len(lines) == 3
    assert all(json.loads(line)["error"]["id"] == "decision.cancelled" for line in lines)
    assert len(session_rows(log_root)) == 3


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
