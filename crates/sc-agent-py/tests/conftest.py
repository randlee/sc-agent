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


def _write_config(root: pathlib.Path, port: int, max_sends: int | None = None) -> pathlib.Path:
    log_root = root / "logs-root"
    config = root / ".sc-agent"
    config.mkdir(parents=True, exist_ok=True)
    data = {
        "log_root": str(log_root),
        "local_runners": {"dec": {"kind": "mapika", "host": "127.0.0.1", "port": port, "model": MODEL}},
    }
    if max_sends is not None:
        data["max_sends"] = max_sends
    (config / "default.json").write_text(json.dumps(data))
    return log_root


@pytest.fixture
def env(tmp_path, monkeypatch, request):
    """`(loopback, log_root)`; the config is `tmp_path/.sc-agent` (see `args`), the log root is
    set in it. The mode comes from `@pytest.mark.mode`. No environment variable points at it."""
    marker = request.node.get_closest_marker("mode")
    callspec = getattr(request.node, "callspec", None)
    status = callspec.params.get("status") if callspec else None
    loopback = Loopback(
        marker.args[0] if marker else ("status" if status is not None else "error"),
        status,
    )
    for name in list(os.environ):
        if name.startswith("SC_AGENT__") or name.endswith("_API_KEY"):
            monkeypatch.delenv(name)
    # The extension and child CLI inherit this trap. Any real OS keyring
    # backend construction during Python tests fails before contacting it.
    monkeypatch.setenv("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING", "1")
    log_root = _write_config(tmp_path, loopback.port, max_sends=1 if status is not None else None)
    try:
        yield loopback, log_root
    finally:
        loopback.close()


def pytest_configure(config):
    config.addinivalue_line("markers", "mode(name): loopback runner mode, hang or error")
    # Applies during collection and to every test, including those that do
    # not request the per-test env fixture. Child CLI processes inherit it.
    os.environ["SC_AGENT_TEST_TRAP_PLATFORM_KEYRING"] = "1"


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
