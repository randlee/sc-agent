"""Render the installed wheel's consumer template for unit and real integration tests."""
import importlib.metadata
import importlib.resources
import importlib.util
import json
import pathlib
import subprocess

SC_DECIDR_VERSION = importlib.metadata.version("sc-decidr")
try:
    SITR_VERSION = importlib.metadata.version("sitr")
except importlib.metadata.PackageNotFoundError:
    SITR_VERSION = ""

def render_consumer(tmp_path, *, general_log=True, models=None, sitr=False):
    import sc_agent
    template = pathlib.Path(str(importlib.resources.files(sc_agent) / "templates/consumer.py.j2"))
    values = {"general_log": general_log, "models": models or ["dec"], "sitr": sitr,
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
    return module

