"""Fixtures: a tempdir config dir naming a loopback runner `dec`, passed as `config_dir=`."""

from __future__ import annotations

import json
import os
import pathlib
import sys

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).parent))

from loopback import MODEL, Loopback  # noqa: E402

REPO_ROOT = pathlib.Path(__file__).resolve().parents[3]


#: Bound for every wait on a forced ordering event; a guard only, never asserted on.
HANG_GUARD = 60.0


def _write_config(root: pathlib.Path, port: int) -> pathlib.Path:
    log_root = root / "logs-root"
    config = root / ".sc-agent"
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
    """`(loopback, log_root)`; the config is `tmp_path/.sc-agent` (see `args`), the log root is
    set in it. The mode comes from `@pytest.mark.mode`. No environment variable points at it."""
    marker = request.node.get_closest_marker("mode")
    loopback = Loopback(marker.args[0] if marker else "error")
    for name in list(os.environ):
        if name.startswith("SC_AGENT__") or name.endswith("_API_KEY"):
            monkeypatch.delenv(name)
    log_root = _write_config(tmp_path, loopback.port)
    try:
        yield loopback, log_root
    finally:
        loopback.close()


def pytest_configure(config):
    config.addinivalue_line("markers", "mode(name): loopback runner mode, hang or error")


@pytest.fixture
def decidr_bin() -> pathlib.Path:
    """The decidr binary built by `cargo build -p decidr`."""
    name = "decidr.exe" if os.name == "nt" else "decidr"
    path = pathlib.Path(os.environ.get("DECIDR_BIN", REPO_ROOT / "target" / "debug" / name))
    assert path.exists(), f"build decidr first (cargo build -p decidr): {path}"
    return path


@pytest.fixture
def config_dir(tmp_path) -> pathlib.Path:
    """The test's config directory (written by `env`)."""
    return tmp_path / ".sc-agent"


@pytest.fixture
def args(env, config_dir) -> dict:
    """Keyword arguments of one send to runner `dec` with the test's `config_dir`."""
    return dict(model="dec", project="proj", agent_role="role", config_dir=str(config_dir))
