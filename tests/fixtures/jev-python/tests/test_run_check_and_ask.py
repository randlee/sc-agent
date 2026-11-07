"""run_check, session ask, JevError codes, and session JSONL."""

from __future__ import annotations

import inspect
import json
import sys
import threading
import time
from pathlib import Path

import pytest

import sc_jev_py

# Scripted judges live only in the test-util build (ADR-SAG-0022).
run_check_scripted = sc_jev_py._testing.run_check_scripted

FIXTURES = Path(__file__).resolve().parents[2]
SECRET = "sk-test-sentinel-not-a-real-key"


def _questions(instructions: str) -> str:
    return json.dumps(
        {
            "schema": "plan-anchoring/questions/1",
            "version": 1,
            "questions": {
                "relevant": {
                    "type": "choice",
                    "instructions": instructions,
                    "criteria": {"no": "No.", "yes": "Yes."},
                }
            },
        }
    )


def _repo(tmp: Path, instructions: str) -> Path:
    repo = tmp / "repo"
    repo.mkdir()
    (repo / "questions.json").write_text(_questions(instructions), encoding="utf-8")
    (repo / "check.toml").write_text(
        '[questions]\npath = "questions.json"\n',
        encoding="utf-8",
    )
    return repo


def _lines(log_dir: Path) -> list[str]:
    path = sc_jev_py.log_file(log_dir)
    if not path.exists():
        return []
    return [line for line in path.read_text(encoding="utf-8").splitlines() if line]


def test_module_documents_log_root(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    doc = sc_jev_py.__doc__ or ""
    assert "SC_AGENT_LOG_DIR" in doc
    assert "SC_LOG_ROOT" in doc
    assert ".sc-agent" in doc
    assert "sc-jev-py.log.jsonl" in doc
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("USERPROFILE", str(tmp_path))
    assert sc_jev_py.default_log_root() == tmp_path / ".sc-agent" / "log"


def test_run_check_has_one_signature_without_scripted_answers() -> None:
    params = inspect.signature(sc_jev_py.run_check).parameters
    assert "scripted_answers" not in params
    assert "scripted_answers" in inspect.signature(run_check_scripted).parameters


def test_log_root_resolution(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.delenv("SC_AGENT_LOG_DIR", raising=False)
    monkeypatch.delenv("SC_LOG_ROOT", raising=False)
    monkeypatch.delenv("SC_AGENT_SESSION_DIR", raising=False)
    explicit = tmp_path / "explicit"
    assert sc_jev_py.resolve_log_root(explicit) == explicit
    assert sc_jev_py.resolve_log_root(str(explicit)) == explicit
    from_agent = tmp_path / "from-agent"
    monkeypatch.setenv("SC_AGENT_LOG_DIR", str(from_agent))
    monkeypatch.setenv("SC_LOG_ROOT", str(tmp_path / "from-observability"))
    assert sc_jev_py.resolve_log_root(None) == from_agent
    monkeypatch.delenv("SC_AGENT_LOG_DIR")
    assert sc_jev_py.resolve_log_root(None) == tmp_path / "from-observability"
    session = tmp_path / "sessions"
    monkeypatch.setenv("SC_AGENT_SESSION_DIR", str(session))
    assert sc_jev_py.resolve_session_dir(explicit) == session
    assert sc_jev_py.resolve_session_dir(explicit, session_dir=tmp_path / "override") == (
        tmp_path / "override"
    )


def test_dry_run_git_repo_returns_digest(tmp_path: Path) -> None:
    repo = FIXTURES / "jev" / "git-repo"
    result = sc_jev_py.run_check(
        "check.toml",
        backends=["jev"],
        dry_run=True,
        repo_root=repo,
        log_dir=tmp_path / "log",
        session_dir=tmp_path / "sessions",
    )
    assert result.session_id is None
    assert result.session is None
    assert result.outcomes == []
    assert result.equal_input is None
    assert len(result.prompt_sha256) == 64
    assert "alpha note" in result.payload
    assert _lines(tmp_path / "log") == []
    assert not (tmp_path / "log").exists()
    assert not (tmp_path / "sessions").exists()
    with pytest.raises(sc_jev_py.JevError) as caught:
        result.ask("why?")
    assert caught.value.code == "CONFIG_INVALID"
    assert SECRET not in str(caught.value)


def test_run_check_and_ask_append_jsonl_without_secrets(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("TYPESAFE_API_KEY", SECRET)
    monkeypatch.setenv("ANTHROPIC_API_KEY", SECRET)
    sentinel = "sentinel-question-text-not-for-logs"
    repo = _repo(tmp_path, sentinel)
    log_dir = tmp_path / "log"
    result = run_check_scripted(
        "check.toml",
        scripted_answers={"relevant": "yes"},
        backends=["anthropic_haiku"],
        dry_run=False,
        repo_root=repo,
        log_dir=log_dir,
        session_dir=tmp_path / "sessions",
    )
    assert result.session_id
    assert result.session.session_id == result.session_id
    assert sentinel in result.payload
    assert SECRET not in result.payload
    assert result.outcomes[0]["backend"] == "anthropic_haiku"
    assert result.outcomes[0]["answers"]["relevant"]["choice"] == "yes"
    assert result.equal_input["equal"] is True
    assert result.equal_input["prompt_sha256"] == result.prompt_sha256
    lines = _lines(log_dir)
    assert len(lines) == 1
    assert "judge" in lines[0]
    assert result.prompt_sha256 in lines[0]
    assert sentinel not in lines[0]
    assert SECRET not in lines[0]
    asked = result.ask("why this choice?")
    assert asked["backend"] == "anthropic_haiku"
    assert SECRET not in json.dumps(asked)
    lines = _lines(log_dir)
    assert len(lines) == 2
    assert "ask" in lines[1]
    assert "why this choice?" not in lines[1]
    assert SECRET not in lines[1]
    session_asked = result.session.ask("one more")
    assert session_asked["backend"] == "anthropic_haiku"
    assert len(_lines(log_dir)) == 3


def test_jev_ask_and_missing_check_map_error_codes(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("TYPESAFE_API_KEY", SECRET)
    repo = _repo(tmp_path, "Does it apply?")
    result = run_check_scripted(
        "check.toml",
        scripted_answers={"relevant": "yes"},
        backends=["jev"],
        repo_root=str(repo),
        log_dir=str(tmp_path / "log"),
        session_dir=str(tmp_path / "sessions"),
    )
    with pytest.raises(sc_jev_py.JevError) as caught:
        result.ask("why?")
    assert caught.value.code == "INPUT_INVALID"
    assert "rationale" in caught.value.message
    assert SECRET not in caught.value.message
    assert SECRET not in str(caught.value)
    with pytest.raises(sc_jev_py.JevError) as missing:
        sc_jev_py.run_check(
            "missing.toml",
            backends=["jev"],
            repo_root=str(repo),
            log_dir=str(tmp_path / "log"),
            session_dir=str(tmp_path / "sessions"),
        )
    assert missing.value.code == "INPUT_MISSING"
    assert SECRET not in missing.value.message
    # Backend ids parse like sc-ai and the engine: unknown ids and kebab-case
    # aliases are INPUT_INVALID.
    for name in ["not-a-backend", "anthropic-haiku"]:
        with pytest.raises(sc_jev_py.JevError) as unknown:
            sc_jev_py.run_check(
                "check.toml",
                backends=[name],
                repo_root=str(repo),
                log_dir=str(tmp_path / "log"),
            )
        assert unknown.value.code == "INPUT_INVALID"
        assert name in unknown.value.message


def test_build_request_releases_the_gil(tmp_path: Path) -> None:
    repo = _repo(tmp_path, "Slow:\n\n{slow}")
    (repo / "check.toml").write_text(
        "\n".join(
            [
                "[questions]",
                'path = "questions.json"',
                "",
                "[inputs.slow]",
                'kind = "command"',
                f"command = [{json.dumps(sys.executable)}, \"-c\", \"import time; time.sleep(5)\"]",
                "timeout_seconds = 1",
                "",
            ]
        ),
        encoding="utf-8",
    )
    ticks = [0]
    stop = threading.Event()

    def spin() -> None:
        while not stop.is_set():
            ticks[0] += 1
            time.sleep(0.01)

    worker = threading.Thread(target=spin)
    worker.start()
    try:
        with pytest.raises(sc_jev_py.JevError) as caught:
            sc_jev_py.build_request("check.toml", repo_root=repo)
    finally:
        stop.set()
        worker.join()
    assert caught.value.code == "EXTRACTION_TIMEOUT"
    assert ticks[0] >= 20, ticks[0]
