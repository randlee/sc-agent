#!/usr/bin/env python3
"""Render both consumer configurations from the installed wheel and check them in strict pyright mode."""
import importlib.metadata
import importlib.resources
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def render_consumers(destination):
    template_root = Path(str(importlib.resources.files("sc_agent") / "templates"))
    rendered = []
    for client, sitr in (("plan-anchor", False), ("judge-jev", True)):
        variables = destination / f"{client}.json"
        variables.write_text(json.dumps({"client_name": client, "sitr": sitr,
                                         "sc_decidr_version": importlib.metadata.version("sc-decidr")}))
        output = destination / (client.replace("-", "_") + "_client.py")
        subprocess.run(["sc-compose", "render", "--root", str(template_root), "--file", "consumer.py.j2",
                        "--var-file", str(variables), "--strict", "--output", str(output)], check=True,
                       env={**os.environ, "SC_COMPOSE_LOG_ROOT": str(destination)})
        rendered.append(output)
    return rendered


def main():
    with tempfile.TemporaryDirectory(prefix="sc-agent-template-") as temporary:
        root = Path(temporary)
        files = render_consumers(root)
        config = root / "pyrightconfig.json"
        config.write_text(json.dumps({"include": [path.name for path in files],
                                      "typeCheckingMode": "strict", "pythonVersion": "3.11"}))
        checked = subprocess.run([sys.executable, "-m", "pyright", "--project", str(config),
                                  "--pythonpath", sys.executable, "--outputjson"], text=True, stdout=subprocess.PIPE)
        report = json.loads(checked.stdout)
        summary = report["summary"]
        if checked.returncode:
            print(checked.stdout)
        checked.check_returncode()
        if summary["filesAnalyzed"] < len(files):
            raise RuntimeError("pyright did not analyze both generated clients")
        print(f"Pyright strict: {summary['filesAnalyzed']} files, {summary['errorCount']} errors")


if __name__ == "__main__":
    main()
