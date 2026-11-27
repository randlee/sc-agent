"""Export provider credentials for consumer integration runs; requires decidr on PATH."""
import argparse
import getpass
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def credential_variables() -> dict[str, str]:
    found = subprocess.run(["decidr", "models"], capture_output=True, text=True, timeout=20)
    if found.returncode:
        raise RuntimeError(found.stdout + found.stderr)
    return {row["name"]: row["key"] for row in json.loads(found.stdout)["value"] if row["key"]}


def key_for(provider, variable):
    if sys.platform == "darwin":
        found = subprocess.run(["security", "find-generic-password", "-s", variable,
                                "-a", getpass.getuser(), "-w"], capture_output=True, text=True, timeout=20)
        if found.returncode == 0:
            return found.stdout.rstrip("\n")
        if found.returncode != 44:  # Missing item; every other keystore error fails.
            raise RuntimeError(f"{provider}: keychain lookup failed (exit {found.returncode})")
    return os.environ.get(variable)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--export-env", type=Path, required=True,
                        help="Write a new mode-0600 credential .env outside the checkout")
    args = parser.parse_args()
    variables = credential_variables()
    keys = {variable: key_for(provider, variable) for provider, variable in variables.items()}
    if not any(keys.values()):
        parser.error("no provider credentials found (environment or platform keychain)")
    destination = args.export_env.resolve()
    if ROOT in destination.parents:
        raise RuntimeError("temporary credential export must be outside the checkout")
    if any("\n" in key or "\r" in key for key in keys.values() if key):
        raise RuntimeError("credential contains a newline")
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as output:
        for variable, key in keys.items():
            if key:
                output.write(f"{variable}={key}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
