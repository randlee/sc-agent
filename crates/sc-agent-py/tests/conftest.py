"""Fixtures: a tempdir HOME whose global config names a loopback runner `dec`."""

from __future__ import annotations

import json
import os
import pathlib
import sys

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).parent))

from loopback import MODEL, Loopback  # noqa: E402

REPO_ROOT = pathlib.Path(__file__).resolve().parents[3]


def _write_config(home: pathlib.Path, port: int) -> pathlib.Path:
    log_root = home / "logs-root"
    config = home / ".sc-agent"
    config.mkdir(parents=True, exist_ok=True)
    (config / "default.json").write_text(
        json.dumps(
            {
                "log_root": str(log_root),
                "local_runners": {"dec": {"kind": "mapika", "host": "127.0.0.1", "port": port, "model": MODEL}},
            }
        )
    )
    return log_root


@pytest.fixture
def env(tmp_path, monkeypatch, request):
    """`(loopback, log_root)` with HOME = tmp_path; the mode comes from `@pytest.mark.mode`."""
    marker = request.node.get_closest_marker("mode")
    loopback = Loopback(marker.args[0] if marker else "error")
    for name in list(os.environ):
        if name.startswith("SC_AGENT__") or name.endswith("_API_KEY"):
            monkeypatch.delenv(name)
    monkeypatch.setenv("HOME", str(tmp_path))
    log_root = _write_config(tmp_path, loopback.port)
    try:
        yield loopback, log_root
    finally:
        loopback.close()


def pytest_configure(config):
    config.addinivalue_line("markers", "mode(name): loopback runner mode, hang or error")


@pytest.fixture
def decide_bin() -> pathlib.Path:
    """The decide binary built by `cargo build -p decide`."""
    name = "decide.exe" if os.name == "nt" else "decide"
    path = pathlib.Path(os.environ.get("DECIDE_BIN", REPO_ROOT / "target" / "debug" / name))
    assert path.exists(), f"build decide first (cargo build -p decide): {path}"
    return path
