"""Build or select a wheel environment, then run the real suite once per provider."""
import argparse
import getpass
import os
from pathlib import Path
import re
import runpy
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


def run(argv, *, timeout=600, **kwargs):
    command = [str(value) for value in argv]
    try:
        subprocess.run(command, check=True, timeout=timeout, **kwargs)
    except subprocess.TimeoutExpired as exc:
        raise RuntimeError(f"integration command timed out after {timeout}s: {' '.join(command)}") from exc


def credential_variables():
    source = ROOT / "crates/sc-agent/src/lib.rs"
    return {name: name for name in re.findall(r'\bkey:\s*"([A-Z][A-Z0-9_]+)"', source.read_text())}


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
    binaries = parser.add_mutually_exclusive_group()
    binaries.add_argument("--wheel", type=Path)
    binaries.add_argument("--venv", type=Path)
    parser.add_argument("--python", default="python3.11")
    parser.add_argument("--env-file", type=Path, help="Consume and delete a temporary credential .env, including on failure")
    parser.add_argument("--export-env", type=Path, help="Write a new mode-0600 credential .env outside the checkout, without running tests")
    parser.add_argument("--platform", choices=["macos", "colima"], default="macos" if sys.platform == "darwin" else "colima")
    parser.add_argument("--results", type=Path)
    parser.add_argument("--pin-tag")
    parser.add_argument("--pin-sha")
    args = parser.parse_args()
    try:
        if bool(args.pin_tag) != bool(args.pin_sha) or (args.pin_tag and not (args.wheel or args.venv)):
            parser.error("a prerelease report requires --wheel/--venv, --pin-tag and --pin-sha together")
        return execute(args)
    finally:
        if args.env_file:
            args.env_file.unlink(missing_ok=True)


def execute(args):
    variables = credential_variables()
    providers = ("typesafe", "anthropic", "openai")
    if args.env_file:
        values = dict(line.split("=", 1) for line in args.env_file.read_text().splitlines()
                      if line.strip() and not line.lstrip().startswith("#"))
        keys = {provider: values.get(variables[f"{provider.upper()}_API_KEY"], "").strip() for provider in providers}
    else:
        keys = {provider: key_for(provider, variables[f"{provider.upper()}_API_KEY"]) for provider in providers}
    if not keys["typesafe"]:
        raise RuntimeError("TypeSafe key is required (environment or platform keychain); no tests run")
    if args.export_env:
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
    results = (args.results or Path(tempfile.mkdtemp(prefix="sc-agent-integration-results-"))).resolve()
    results.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sc-agent-integration-") as scratch:
        scratch = Path(scratch)
        if args.venv:
            python = args.venv.resolve() / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        else:
            environment = scratch / "venv"
            run([args.python, "-m", "venv", environment])
            python = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        version_check = "sys.version_info[:2] >= (3, 11)" if args.venv else "sys.version_info[:2] == (3, 11)"
        run([python, "-c", f"import sys; assert {version_check}, 'integration needs Python 3.11 or newer'"])
        run([python, "-m", "pip", "install", "--no-cache-dir", "pytest", *([] if args.wheel or args.venv else ["maturin>=1.7,<2"])])
        if not args.venv:
            wheel = args.wheel.resolve() if args.wheel else None
            if wheel is None:
                wheels = scratch / "wheels"
                run([python, "-m", "maturin", "build", "--manifest-path", ROOT / "crates/sc-agent-py/Cargo.toml",
                     "--interpreter", python, "--out", wheels], cwd=ROOT, timeout=1800)
                wheel, = wheels.glob("*.whl")
            run([python, "-m", "pip", "install", "--no-cache-dir", "--no-deps", wheel])
        report_run_id = None
        if args.pin_tag:
            renderer = runpy.run_path(str(ROOT / "templates/integration-report/render.py"))
            report_run_id = renderer["new_run_id"](ROOT / "site/reports")
        failed = False
        for provider in providers:
            junit = results / f"{args.platform}-{provider}.xml"
            if not keys[provider]:
                suite = ET.Element("testsuite", name=provider, tests="1", skipped="1")
                case = ET.SubElement(suite, "testcase", name=f"{provider}: not run (no key)")
                ET.SubElement(case, "skipped", message="no provider key available")
                ET.ElementTree(suite).write(junit, encoding="unicode")
                print(f"{provider}: NOT RUN (no key)", flush=True)
            else:
                env = {k: v for k, v in os.environ.items() if k not in variables.values() and not k.startswith("SC_AGENT__")}
                env.pop("SC_AGENT_LIVE", None)
                env.pop("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING", None)
                env[variables[f"{provider.upper()}_API_KEY"]] = keys[provider]
                env["SC_AGENT_INTEGRATION_PROVIDER"] = provider
                print(f"{provider}: real-provider integration", flush=True)
                try:
                    completed = subprocess.run([str(python), "-m", "pytest", str(ROOT / "crates/sc-agent-py/integration"),
                                               "-v", "-o", "junit_family=legacy", f"--junitxml={junit}"],
                                               cwd=scratch, env=env, timeout=600)
                except subprocess.TimeoutExpired as exc:
                    raise RuntimeError(f"{provider}: integration timed out after 600s") from exc
                failed |= completed.returncode != 0
                cases = list(ET.parse(junit).getroot().iter("testcase")) if junit.exists() else []
                if not cases or all(case.find("skipped") is not None for case in cases):
                    failed = True
            if args.pin_tag:
                run([sys.executable, ROOT / "templates/integration-report/render.py", junit,
                     "--run-id", report_run_id, "--platform", args.platform, "--mode", "live", "--source-revision",
                     args.pin_sha,
                     "--wheel-revision", f"{args.pin_tag}@{args.pin_sha}", "--output",
                     ROOT / f"site/reports/integration/{args.platform}-{provider}.xhtml"])
        print(f"Per-provider JUnit results: {results}", flush=True)
        return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
