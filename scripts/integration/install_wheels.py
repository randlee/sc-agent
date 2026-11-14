#!/usr/bin/env python3
"""Install verified staged wheels into an isolated Python 3.11+ environment or verify it."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import venv

from configuration import release_manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", required=True, type=Path)
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--version")
    parser.add_argument("wheels", nargs="*", type=Path)
    args = parser.parse_args()
    if sys.version_info < (3, 11):
        parser.error("Python 3.11+ is required")
    python = args.stage / "venv" / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
    if args.verify:
        manifest = release_manifest()
        for binary in manifest["prerelease"]["binaries"]:
            path = args.stage / "bin" / (binary + (".exe" if sys.platform == "win32" else ""))
            result = subprocess.run([str(path), "--version"], check=True, text=True, capture_output=True)
            if args.version not in result.stdout:
                raise SystemExit(f"{binary} version differs from requested prerelease")
        packages = manifest["python_packages"]
        code = "import importlib,importlib.metadata as m,json,sys; expected=sys.argv[1]; packages=json.loads(sys.argv[2]); [importlib.import_module(p['module']) for p in packages]; actual=[m.version(p['package']) for p in packages]; sys.exit('installed wheel version differs from prerelease' if any(version != expected for version in actual) else 0)"
        subprocess.run([str(python), "-c", code, args.version, json.dumps(packages)], check=True)
        print(args.version)
    else:
        if not args.wheels:
            parser.error("verified wheels are required")
        venv.create(args.stage / "venv", with_pip=True)
        subprocess.run([str(python), "-m", "pip", "install", "--no-index", "--no-deps", *map(str, args.wheels)], check=True)


if __name__ == "__main__":
    main()
