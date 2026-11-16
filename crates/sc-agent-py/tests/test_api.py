"""The sc-agent Python API over the maturin build.

Default runs use a controlled loopback runner and pass its tempdir config explicitly.
Real-provider cases run separately through just integration.
No test changes HOME or USERPROFILE. Orderings are driven by events, with bounded
waits used only as failure backstops and no elapsed-time assertions.
"""

from __future__ import annotations

import _thread
import json
import os
import pathlib
import re
import subprocess
import threading
import signal
import warnings

import pytest

import sc_agent
from conftest import HANG_GUARD
from loopback import FIXED_ERROR

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


def test_python_module_exposes_its_module_documentation():
    assert sc_agent.__doc__ == (
        "The `sc_agent` module: `send`, `send_batch`, `models`, `doctor`, `CancelToken`, `SagError`."
    )


def session_files(log_root: pathlib.Path) -> list[pathlib.Path]:
    root = log_root / "logs"
    return sorted(root.rglob("sessions.jsonl")) if root.exists() else []


def session_rows(log_root: pathlib.Path) -> list[dict]:
    return [json.loads(line) for path in session_files(log_root) for line in path.read_text().splitlines()]


def error_of(info: pytest.ExceptionInfo) -> tuple[str, dict]:
    ident, _message, detail = info.value.args
    return ident, json.loads(detail)


def find_status(detail: dict) -> int | None:
    if "status" in detail:
        return detail["status"]
    cause = detail.get("cause")
    return find_status(cause) if cause else None


@pytest.mark.parametrize("status", [429, 503, 529, 401, 422])
def test_structured_http_status_on_batch_and_exception(env, args, status):
    """Controlled loopback statuses remain numeric in batch errors and SagError details."""
    _loopback, _log_root = env
    result = json.loads(sc_agent.send_batch([NOUL], **args)[0])
    assert result["sends"] == 1
    assert find_status(result["error"]) == status

    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.models(runner="dec", config_dir=args["config_dir"])
    _ident, detail = error_of(info)
    assert find_status(detail) == status


@pytest.mark.parametrize("status,body,expected_id,category", [
    (400, "private MAX_TOKENS exceeded", "decision.context_exceeded", "validation"),
    (400, "ordinary refusal", "decision.api_bad_request", "validation"),
    (401, "max_tokens exceeded", "decision.api_status", "auth"),
])
def test_context_overflow_through_python_wheel(env, args, status, body, expected_id, category):
    loopback, _ = env
    loopback.response_body = {"detail": body}
    result = json.loads(sc_agent.send(NOUL, **args))
    error = result["error"]
    assert error["id"] == expected_id
    assert error["category"] == category
    assert error["status"] == status
    if expected_id == "decision.context_exceeded":
        assert body not in json.dumps(result)


def test_every_decidr_operation_has_its_python_function(decidr_bin):
    """AC: ANC-006."""
    text = subprocess.run(
        [decidr_bin, "--help"], check=True, capture_output=True, text=True, timeout=HANG_GUARD
    ).stdout
    commands = re.findall(r"^\s{2}(\w+)\s", text.split("Commands:")[1].split("Options:")[0], re.M)
    commands = [name for name in commands if name != "help"]
    assert sorted(commands) == ["doctor", "models", "send"]
    batch = subprocess.run(
        [decidr_bin, "send", "--help"], check=True, capture_output=True, text=True, timeout=HANG_GUARD
    ).stdout
    assert "--batch" in batch
    operations = {"send": "send", "models": "models", "doctor": "doctor", "send --batch": "send_batch"}
    for operation, function in operations.items():
        assert callable(getattr(sc_agent, function, None)), operation


@pytest.mark.mode("ok")
def test_send_and_send_batch_return_the_decided_response(env, args):
    """REQ-SAG-0501 (successful path), REQ-SAG-0007: a runner that answers gives send() one
    result line with the response, its session ULID and position 0, the probability
    unrounded; send_batch() gives one such line per request, positions 0..n-1."""
    loopback, log_root = env
    text = sc_agent.send(NOUL, **args)
    line = json.loads(text)
    assert "error" not in line
    assert line["position"] == 0
    assert line["sends"] == 1
    assert len(line["session_id"]) == 26
    assert line["response"]["answers"]["q"] == {"type": "noul", "noul": 0.123456789}
    assert '"noul":0.123456789' in text
    assert set(line["digests"]) == {"q"}
    lines = [json.loads(item) for item in sc_agent.send_batch([NOUL] * 3, **args)]
    assert sorted(item["position"] for item in lines) == [0, 1, 2]
    assert all(item["response"]["answers"]["q"]["noul"] == 0.123456789 for item in lines)
    assert len(loopback.requests) == 4
    rows = session_rows(log_root)
    assert len(rows) == 4
    assert all(row["fields"]["outcome"] == "ok" for row in rows), rows


def test_a_cancelled_token_stops_send_and_send_batch(env, args):
    """AC: REQ-SAG-0104 AC2."""
    loopback, _log_root = env
    token = sc_agent.CancelToken()
    token.cancel()
    assert token.is_cancelled()
    line = json.loads(sc_agent.send(NOUL, cancel=token, **args))
    assert line["error"]["id"] == "decision.cancelled"
    assert line["sends"] == 0
    lines = sc_agent.send_batch([NOUL, NOUL], cancel=token, **args)
    assert [json.loads(item)["error"]["id"] for item in lines] == ["decision.cancelled"] * 2
    assert loopback.requests == []


def test_an_invalid_project_is_refused(env, args):
    """AC: REQ-SAG-0004."""
    loopback, log_root = env
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send(NOUL, **{**args, "project": "Phase B"})
    ident, detail = error_of(info)
    assert ident == "decidr.log"
    assert detail["cause"]["id"] == "log.invalid_slug"
    assert loopback.requests == []
    assert session_files(log_root) == []


def assert_lowest_409(error: dict) -> None:
    """The loopback's 409 as the lowest-layer error: its id and its message with the body."""
    assert error["id"] == "decision.api_status"
    assert "cause" not in error, error
    assert error["message"] == "the API answered HTTP 409: " + json.dumps(FIXED_ERROR), error


def test_cli_and_python_rows_carry_the_same_identity(env, decidr_bin, tmp_path, args, config_dir):
    """AC: REQ-SAG-0004 AC3."""
    loopback, log_root = env
    request = tmp_path / "request.json"
    request.write_text(NOUL)
    child = subprocess.run(
        [
            decidr_bin,
            "--config-dir",
            str(config_dir),
            "send",
            "--model",
            "dec",
            "--project",
            "proj",
            "--agent-role",
            "role",
            str(request),
        ],
        capture_output=True,
        text=True,
        timeout=HANG_GUARD,
    )
    assert child.returncode == 1, child.stderr  # the loopback answers a fixed error
    # REQ-SAG-0002 AC1 at both entry points: the 409 is the lowest-layer failure, so it is the
    # error itself (no cause below it), and its message carries the provider's body whole.
    for error in (json.loads(child.stdout)["error"], json.loads(sc_agent.send(NOUL, **args))["error"]):
        assert_lowest_409(error)
    rows = session_rows(log_root)
    assert len(rows) == 2
    identities = {(row["fields"]["project"], row["fields"]["agent_role"]) for row in rows}
    assert identities == {("proj", "role")}
    assert len(loopback.requests) == 2


def test_injected_loggers_receive_the_records_and_write_no_file(env, args):
    """AC: REQ-SAG-0008 Python."""
    _loopback, log_root = env
    records: list[dict] = []
    rows: list[dict] = []
    sc_agent.send(NOUL, general_log=records.append, session_log=rows.append, **args)
    assert "decidr.send" in [record["action"] for record in records]
    assert "http.send" in [record["action"] for record in records]
    assert len(rows) == 1 and rows[0]["action"] == "session.row"
    assert list(rows[0]["fields"])[:4] == ["session_id", "repo", "project", "agent_role"]
    assert not (log_root / "logs" / "sc-agent-py.log.jsonl").exists()
    assert session_files(log_root) == []
    models: list[dict] = []
    listed = json.loads(
        sc_agent.models(runner="dec", general_log=models.append, config_dir=args["config_dir"])
    )
    assert listed["models"][0]["name"] == "jev-latest"
    assert "decidr.models" in [record["action"] for record in models]


def test_only_a_session_log_callable_keeps_the_default_general_log(env, args):
    """AC: REQ-SAG-0008 AC4."""
    _loopback, log_root = env
    rows: list[dict] = []
    sc_agent.send(NOUL, session_log=rows.append, **args)
    assert len(rows) == 1
    general = log_root / "logs" / "sc-agent-py.log.jsonl"
    assert general.exists()
    assert "decidr.send" in general.read_text()
    assert session_files(log_root) == []


def test_only_a_general_log_callable_keeps_the_default_session_log(env, args):
    """AC: REQ-SAG-0008 AC5."""
    _loopback, log_root = env
    records: list[dict] = []
    sc_agent.send(NOUL, general_log=records.append, **args)
    assert "decidr.send" in [record["action"] for record in records]
    files = session_files(log_root)
    assert len(files) == 1
    assert files[0].parts[-3:] == ("role", "proj", "sessions.jsonl")
    assert (log_root / "logs") in files[0].parents
    assert not (log_root / "logs" / "sc-agent-py.log.jsonl").exists()


@pytest.mark.mode("hang")
def test_keyboard_interrupt_cancels_drains_and_reraises(env, args):
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

    # interrupt_main only reaches a SIGINT handler Python owns; a launcher (a background
    # shell job, some CI runners) may start the process with SIGINT ignored, so the test
    # installs Python's default handler for its own duration.
    previous = signal.signal(signal.SIGINT, signal.default_int_handler)
    thread = threading.Thread(target=interrupt)
    thread.start()
    try:
        with pytest.raises(KeyboardInterrupt):
            sc_agent.send_batch([NOUL] * 3, cancel=token, on_result=lines.append, **args)
    finally:
        returned.set()
        thread.join(HANG_GUARD)
        signal.signal(signal.SIGINT, previous)
    assert not loopback.release.is_set(), "the runner never answered"
    assert token.is_cancelled()
    assert len(lines) == 3
    assert all(json.loads(line)["error"]["id"] == "decision.cancelled" for line in lines)
    assert len(session_rows(log_root)) == 3


def test_a_raising_log_callable_never_changes_the_result(env, args):
    """AC: REQ-SAG-0008 (QA-003). A general_log or session_log callable that raises makes its
    writes refused writes: the send returns its normal result, and the lost rows are reported
    (as a RuntimeWarning naming log.rows_lost) instead of being dropped silently."""

    def refuse(_record: dict) -> None:
        raise ValueError("this callable refuses every record")

    with pytest.warns(RuntimeWarning, match="log.rows_lost"):
        line = json.loads(sc_agent.send(NOUL, general_log=refuse, **args))
    assert line["error"]["id"] == "decision.api_status"  # the loopback's fixed error, as always

    with pytest.warns(RuntimeWarning, match="log.rows_lost"):
        line = json.loads(sc_agent.send(NOUL, session_log=refuse, **args))
    assert line["error"]["id"] == "decision.api_status"

    with warnings.catch_warnings():
        warnings.simplefilter("error")  # a callable that accepts every record warns nothing
        sc_agent.send(NOUL, general_log=lambda _record: None, session_log=lambda _record: None, **args)


def test_an_on_result_exception_cancels_drains_and_reraises(env, args):
    """AC: REQ-SAG-0011 Python (on_result)."""
    seen: list[str] = []

    def explode(line: str) -> None:
        seen.append(line)
        raise ValueError("stop here")

    token = sc_agent.CancelToken()
    with pytest.raises(ValueError, match="stop here"):
        sc_agent.send_batch([NOUL] * 5, cancel=token, on_result=explode, **args)
    assert token.is_cancelled()
    assert len(seen) == 1


def test_images_and_long_context_reach_the_runner(env, args):
    """AC: REQ-SAG-0501 images."""
    loopback, _log_root = env
    sc_agent.send(PART, images={"img1": PNG}, long_context=True, **args)
    (request,) = loopback.requests
    assert request["headers"]["content-type"].startswith("multipart/form-data; boundary=")
    assert request["headers"]["typesafe-danger-zone-long-context"] == "true"
    assert b'name="img1"' in request["body"]
    assert PNG in request["body"]
    sc_agent.send_batch([(PART, {"img1": PNG}, True)], **args)
    assert b'name="img1"' in loopback.requests[1]["body"]


def test_input_limits_raise_before_any_send(env, args):
    """AC: REQ-SAG-0014."""
    loopback, _log_root = env
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send_batch([NOUL] * 10_001, **args)
    ident, detail = error_of(info)
    assert ident == "decidr.limit_exceeded" and "batch_lines" in detail["message"]
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send(" " * (9 * 1024 * 1024), **args)
    ident, detail = error_of(info)
    assert ident == "decidr.limit_exceeded" and "request_bytes" in detail["message"]
    assert loopback.requests == []


@pytest.mark.parametrize("operation", [sc_agent.send, sc_agent.send_batch])
@pytest.mark.parametrize("option", ["keyring_service", "keyring_account"])
@pytest.mark.parametrize("value", ["", "x" * 256, "é" * 128])
def test_keyring_location_invalid_before_any_send(env, args, operation, option, value):
    loopback, log_root = env
    request = [NOUL] if operation is sc_agent.send_batch else NOUL
    with pytest.raises(sc_agent.SagError) as info:
        operation(request, **args, **{option: value})
    ident, detail = error_of(info)
    assert ident == "sc_agent_py.invalid_option"
    assert detail["id"] == ident
    assert option in detail["message"]
    assert "255" in detail["message"]
    assert detail["recovery"] == "Correct the named option and retry."
    assert loopback.requests == []
    assert not session_files(log_root)


@pytest.mark.parametrize("operation", [sc_agent.send, sc_agent.send_batch])
def test_keyring_override_does_not_require_provider_for_local_runner(env, args, operation):
    loopback, _ = env
    request = [NOUL] if operation is sc_agent.send_batch else NOUL
    result = operation(request, **args, keyring_service="consumer")
    lines = result if isinstance(result, list) else [result]
    assert all("error" in json.loads(line) for line in lines)
    assert len(loopback.requests) == 1


def test_real_platform_keyring_is_trapped_in_python_tests(env, args, monkeypatch, tmp_path):
    """A missing remote credential reaches the real-backend constructor, which must refuse."""
    loopback, _log_root = env
    assert os.environ.get("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING") == "1"
    # Avoid repository .env discovery so the test always reaches the keyring path.
    monkeypatch.chdir(tmp_path)
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.send(
            NOUL,
            model="jev-latest",
            project="proj",
            agent_role="role",
            provider="typesafe",
            config_dir=args["config_dir"],
            keyring_service="test-keyring-trap",
        )
    ident, detail = error_of(info)
    assert ident == "decidr.secrets"
    assert detail["cause"]["id"] == "secrets.keyring_construction_refused"
    assert "refused by the active test guard" in detail["cause"]["message"]
    assert detail["cause"]["recovery"] == (
        "use an injected keyring source in tests; do not access the platform keyring from a test"
    )
    assert loopback.requests == []


@pytest.mark.mode("ok")
def test_models_list_outcome(outcome_models_args, outcome_call, record_property):
    """The unit outcome uses the local models endpoint."""
    models = json.loads(outcome_call(sc_agent.models, **outcome_models_args))["models"]
    assert isinstance(models, list) and models
    assert all(isinstance(model["name"], str) and model["name"] for model in models)
    record_property("provider", "typesafe" if "runner" not in outcome_models_args else "local")
    record_property("result", f"{len(models)} models listed")


@pytest.mark.mode("ok")
def test_noul_send_outcome(outcome_args, outcome_call, consumer, record_property):
    """The rendered consumer returns a valid noul response over loopback."""
    text, code = outcome_call(consumer.call, NOUL, **outcome_args, abort_on=frozenset(), category_codes={}, cancel=sc_agent.CancelToken())
    assert code is None
    line = json.loads(text)
    assert "error" not in line
    assert line["position"] == 0
    assert len(line["session_id"]) == 26
    answer = line["response"]["answers"]["q"]
    assert answer["type"] == "noul"
    assert isinstance(answer["noul"], (int, float)) and 0 <= answer["noul"] <= 1
    record_property("result", json.dumps(answer))


@pytest.mark.mode("ok")
def test_noul_send_batch_outcome(outcome_args, outcome_call, consumer, record_property):
    """The rendered consumer returns a valid noul response over loopback."""
    results = outcome_call(consumer.call_batch, [NOUL] * 3, **outcome_args, abort_on=frozenset(), category_codes={}, cancel=sc_agent.CancelToken())
    assert all(code is None for _, code in results)
    lines = [json.loads(line) for line, _ in results]
    assert sorted(line["position"] for line in lines) == [0, 1, 2]
    for line in lines:
        assert "error" not in line
        assert len(line["session_id"]) == 26
        answer = line["response"]["answers"]["q"]
        assert answer["type"] == "noul"
        assert isinstance(answer["noul"], (int, float)) and 0 <= answer["noul"] <= 1
    record_property("result", json.dumps([line["response"]["answers"]["q"] for line in lines]))


def test_keyring_override_refuses_ambiguous_ranking_before_send(env, args):
    loopback, _ = env
    with pytest.raises(sc_agent.SagError) as caught:
        sc_agent.send(NOUL, **{**args, "model": "haiku"}, keyring_service="consumer")
    assert caught.value.id == "decidr.ambiguous_keyring_override"
    assert caught.value.category == "config"
    assert "anthropic" in caught.value.message
    assert all(provider in caught.value.message for provider in ("anthropic", "openrouter", "together"))
    assert loopback.requests == []
