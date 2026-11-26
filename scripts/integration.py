"""Export provider credentials for the consumers' owner-attended integration runs."""
import argparse
import getpass
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def credential_variables():
    source = ROOT / "crates/sc-agent/src/providers.rs"
    return {name: name for name in re.findall(r'\bSome\("([A-Z][A-Z0-9_]+)"\)', source.read_text())}


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
    providers = ("typesafe", "anthropic", "openai")
    keys = {provider: key_for(provider, variables[f"{provider.upper()}_API_KEY"]) for provider in providers}
    if not keys["typesafe"]:
        raise RuntimeError("TypeSafe key is required (environment or platform keychain)")
    destination = args.export_env.resolve()
    if ROOT in destination.parents:
        raise RuntimeError("temporary credential export must be outside the checkout")
    if any("\n" in key or "\r" in key for key in keys.values() if key):
        raise RuntimeError("credential contains a newline")
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as output:
        for provider, key in keys.items():
            if key:
                output.write(f"{variables[f'{provider.upper()}_API_KEY']}={key}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
