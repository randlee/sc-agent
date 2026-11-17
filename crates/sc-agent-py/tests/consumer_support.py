"""Render the installed wheel's consumer template for unit and real integration tests."""
import importlib.metadata
import importlib.resources
import importlib.util
import json
import pathlib
import subprocess
import types
import sys

SC_DECIDR_VERSION = importlib.metadata.version("sc-decidr")
try:
    SITR_VERSION = importlib.metadata.version("sitr")
except importlib.metadata.PackageNotFoundError:
    SITR_VERSION = ""

def render_consumer(tmp_path, *, monkeypatch, general_log=True, models=None, sitr=False, batch_records=False,
                    source_template=False):
    import sc_agent
    template = (pathlib.Path(__file__).resolve().parents[1] / "python/sc_agent/templates/consumer.py.j2"
                if source_template else pathlib.Path(str(importlib.resources.files(sc_agent) / "templates/consumer.py.j2")))
    values = {"general_log": general_log, "models": models or ["dec"], "sitr": sitr,
              "batch_records": batch_records, "settings_module": "sc_agent_test_settings",
              "settings_file": ".sc/test/settings.yaml", "settings_set_command": "test-settings set allow_remote_judge true|false",
              "sc_decidr_version": SC_DECIDR_VERSION, "sitr_version": SITR_VERSION}
    variables = tmp_path / "consumer-vars.json"
    variables.write_text(json.dumps(values))
    output = tmp_path / "consumer.py"
    subprocess.run(["sc-compose", "render", "--root", str(template.parent), "--file", str(template),
                    "--var-file", str(variables), "--strict", "--output", str(output)],
                   check=True, capture_output=True, text=True, timeout=60)
    spec = importlib.util.spec_from_file_location("rendered_consumer", output)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    install_test_settings_module(monkeypatch)
    return module


def install_test_settings_module(monkeypatch, *, allowed=True, missing=False):
    """Controlled settings result used by consumer tests; never reads a host repo or sends data."""
    module = types.ModuleType("sc_agent_test_settings")
    module.find_repo_root = lambda path: types.SimpleNamespace(ok=True, value=path)
    if allowed:
        module.remote_judge_consent = lambda path: types.SimpleNamespace(ok=True, value=True)
    elif missing:
        module.remote_judge_consent = lambda path: types.SimpleNamespace(
            ok=False, error=types.SimpleNamespace(code="SETTINGS.REMOTE_JUDGE_UNSET", message="unset", action="set it"))
    else:
        module.remote_judge_consent = lambda path: types.SimpleNamespace(ok=True, value=False)
    monkeypatch.setitem(sys.modules, module.__name__, module)
    return module

