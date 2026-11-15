"""Default loopback fixtures; marked outcomes opt into owner-attended live calls on SC_AGENT_LIVE=1."""

from __future__ import annotations

import json
import os
import pathlib
import sys

import pytest

import sc_agent

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
    config.addinivalue_line("markers", "live_outcome: owner-run outcome may use real providers only on SC_AGENT_LIVE=1")
    # Armed during collection and for default/nonmarked cases. Only the
    # explicitly live outcome_call fixture temporarily clears this trap.
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


def _live_outcome(request):
    return os.environ.get("SC_AGENT_LIVE") == "1" and request.node.get_closest_marker("live_outcome") is not None


def _call_outcome(operation, *args, _live=False, **kwargs):
    try:
        return operation(*args, **kwargs)
    except sc_agent.SagError as error:
        if _live and len(error.args) == 3:
            try:
                detail = json.loads(error.args[2])
            except (TypeError, ValueError):
                detail = None
            while isinstance(detail, dict):
                if detail.get("id") in {"decidr.no_provider_key", "secrets.key_missing"}:
                    pytest.skip(f"{kwargs.get('model', 'jev')}: no applicable API key available")
                detail = detail.get("cause")
        raise


@pytest.fixture(params=("jev", "haiku", "luna") if os.environ.get("SC_AGENT_LIVE") == "1" else ("dec",))
def outcome_model(request):
    return request.param


@pytest.fixture
def outcome_args(request, outcome_model):
    if _live_outcome(request):
        # Omit provider/config overrides: the wheel selects the actual provider,
        # resolves credentials and uses the owner's real configuration.
        return dict(model=outcome_model, project="owner-check", agent_role="pytest")
    return request.getfixturevalue("args")


@pytest.fixture
def outcome_models_args(request):
    if _live_outcome(request):
        return {}  # models() is TypeSafe by default in the 0.1.5 API.
    args = request.getfixturevalue("args")
    return dict(runner="dec", config_dir=args["config_dir"])


@pytest.fixture
def outcome_call(request, monkeypatch):
    live = _live_outcome(request)
    if live:
        # Only explicitly opted-in, marked cases may reach the owner's keychain.
        monkeypatch.delenv("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING", raising=False)
    def call(operation, *args, **kwargs):
        return _call_outcome(operation, *args, _live=live, **kwargs)
    return call


@pytest.fixture
def explicit_live_switch(monkeypatch):
    """Controlled guard regression: a nonmarked test still uses loopback on opt-in."""
    monkeypatch.setenv("SC_AGENT_LIVE", "1")
