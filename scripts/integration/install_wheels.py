#!/usr/bin/env python3
"""Install verified staged wheels into an isolated Python 3.11+ environment or verify it."""
import argparse
from pathlib import Path
import subprocess
import sys
import venv


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
        for binary in ["decidr", "sitr"]:
            path = args.stage / "bin" / (binary + (".exe" if sys.platform == "win32" else ""))
            result = subprocess.run([str(path), "--version"], check=True, text=True, capture_output=True)
            if args.version not in result.stdout:
                raise SystemExit(f"{binary} version differs from requested prerelease")
        subprocess.run([str(python), "-c", "import importlib.metadata as m,sc_agent,sc_treesitter,sys; expected=sys.argv[1]; actual=[m.version(name) for name in ('sc-decidr','sitr')]; sys.exit('installed wheel version differs from prerelease' if any(version != expected for version in actual) else 0)", args.version], check=True)
        print(args.version)
    else:
        if not args.wheels:
            parser.error("verified wheels are required")
        venv.create(args.stage / "venv", with_pip=True)
        subprocess.run([str(python), "-m", "pip", "install", "--no-index", "--no-deps", *map(str, args.wheels)], check=True)


if __name__ == "__main__":
    main()
