#!/usr/bin/env python3
"""Run a poured integration formula against verified installed prerelease artifacts.

A run directory is recovery state: it retains selected source/artifacts and sanitized results
so an interrupted provider run resumes without losing its evidence. No raw provider output
or credentials are persisted. Reports require the owner's selected XHTML template.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import tempfile
import tomllib

from preflight import ROOT, Refusal, command, digest, git, prerelease_helper


def runtime_python(stage):
    return stage / "venv" / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")


def formula_cases():
    formula = tomllib.loads((ROOT / "formulas/integration.formula.toml").read_text())
    return [step["id"] for step in formula["steps"] if step["id"] not in {"verify-assets", "report"}]


def prepare(args):
    if not re.fullmatch(r"[A-Za-z0-9_.-]{1,64}", args.run_id):
        raise Refusal("run-id must be a safe 1..64 character identifier")
    if not re.fullmatch(r"prerelease/v\d+\.\d+\.\d+", args.tag):
        raise Refusal("tag must be prerelease/vMAJOR.MINOR.PATCH")
    run_dir = args.run_dir.resolve()
    run_dir.mkdir(parents=True, exist_ok=True)
    state_path = run_dir / "run.json"
    if state_path.exists():
        state = json.loads(state_path.read_text())
        if (state["prerelease_tag"], state["platform"], state["run_id"]) != (args.tag, args.platform, args.run_id):
            raise Refusal("run directory belongs to a different tag/platform/run-id")
        return state
    helper = prerelease_helper()
    manifest = tomllib.loads((ROOT / "release/publish-artifacts.toml").read_text())
    target = helper.host_target(manifest)
    expected = {"aarch64-apple-darwin": "macos-aarch64", "x86_64-apple-darwin": "macos-x86_64", "aarch64-unknown-linux-gnu": "linux-aarch64", "x86_64-unknown-linux-gnu": "linux-x86_64", "x86_64-pc-windows-msvc": "windows-x86_64"}[target["target"]]
    if args.platform != expected:
        raise Refusal("platform must match the machine executing the installed artifacts")
    version = args.tag.removeprefix(manifest["prerelease"]["tag_prefix"])
    release = helper.release_for_tag(args.tag)
    helper.require_release_assets(release, manifest, version)
    names = helper.release_asset_names(release)
    git("fetch", "origin", f"refs/tags/{args.tag}:refs/tags/{args.tag}")
    source = git("rev-list", "-n", "1", args.tag)
    if git("rev-parse", "HEAD") != source:
        raise Refusal("check out the exact prerelease source before executing its integration scripts")
    if not git("branch", "--show-current"):
        raise Refusal("use an attached branch for report commits and pushes")
    if git("status", "--porcelain", "--untracked-files=no"):
        raise Refusal("integration requires a clean tested source checkout")
    receipt_path = run_dir / "formula-receipt.json"
    if receipt_path.exists():
        receipt = json.loads(receipt_path.read_text())
    else:
        receipt = json.loads(command(["sc-compose", "bead", "pour", "formulas/integration.formula.toml", "--var", f"run_id={args.run_id}", "--json"]))
        receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    payload = receipt["payload"]
    if payload.get("outcome") != "succeeded" or not payload.get("graph", {}).get("ids"):
        raise Refusal("formula pour did not produce a successful graph receipt")
    stage = run_dir / "installed"
    with tempfile.TemporaryDirectory(prefix="integration-assets-") as download_dir:
        directory = Path(download_dir)
        checksums = helper.checksums(helper.download_asset(args.tag, "checksums.txt", directory).read_text())
        helper.stage_release(args.tag, version, manifest, names, stage, directory)
    wheels = helper.staged_wheels(stage, manifest)
    subprocess.run([sys.executable, str(ROOT / "scripts/integration/install_wheels.py"), "--stage", str(stage), *map(str, wheels)], check=True)
    subprocess.run([sys.executable, str(ROOT / "scripts/integration/install_wheels.py"), "--verify", "--stage", str(stage), "--version", version], check=True)
    installed = json.loads(command([str(runtime_python(stage)), str(ROOT / "scripts/integration/wheel_case.py"), "contract", "--config-dir", str(run_dir / "config")]))
    if set(installed) != set(formula_cases()):
        raise Refusal("installed Rust case contract differs from the formula")
    archive = helper.archive_name(manifest, version, target)
    artifacts = [{"name": name, "sha256": checksums[name]} for name in [archive, *[p.name for p in wheels]]]
    state = {"run_id": args.run_id, "source_commit": source, "platform": args.platform, "prerelease_tag": args.tag, "artifacts": artifacts, "results": [], "beads": payload["graph"]["ids"], "formula_root": payload["graph"]["parent"]}
    state_path.write_text(json.dumps(state, indent=2) + "\n")
    subprocess.run(["bd", "close", state["beads"]["verify-assets"], "--reason", "Downloaded and checksum-verified installed target binary archive and both wheels"], check=True)
    return state


def result_outcome(case, output, returncode):
    """Judge the actual installed process output, without retaining raw provider text."""
    try:
        value = json.loads(output)
    except ValueError:
        return "FAIL", "Installed process did not return valid JSON", None
    if "invalid-key" in case:
        expected = ["decision.api_status"] if case.startswith("typesafe") else ["decision.backend", "decision_llm.llm", "llm.api_status"]
        error = value.get("error") if isinstance(value, dict) else None
        chain = []
        messages = []
        while isinstance(error, dict):
            chain.append(error.get("id"))
            messages.append(str(error.get("message", "")))
            error = error.get("cause")
        for index in range(len(chain) - len(expected) + 1):
            if chain[index:index + len(expected)] == expected and re.search(r"\b401\b", messages[index + len(expected) - 1]):
                return "PASS", "Invalid environment credential returned the expected sanitized HTTP 401 cause chain", value.get("session_id")
        return "FAIL", "Invalid environment credential did not produce the required HTTP 401 cause chain", None
    if "secrets.key_missing" in output:
        return "SKIP", "Required provider credential was unavailable", None
    if returncode or (isinstance(value, dict) and "error" in value):
        return "FAIL", "Installed provider call returned an error", None
    if "models" in case:
        if isinstance(value, dict) and value.get("models"):
            return "PASS", "Installed client discovered real TypeSafe models", None
        return "FAIL", "TypeSafe model discovery returned no models", None
    try:
        answer = value["response"]["answers"]["q"]
        if answer["type"] != "noul" or not isinstance(answer["noul"], (int, float)) or not 0 <= answer["noul"] <= 1:
            raise ValueError("invalid noul")
        return "PASS", "Installed client returned a real schema-valid noul decision", value["session_id"]
    except (KeyError, TypeError, ValueError):
        return "FAIL", "Provider response did not match the required noul schema", None


def execute_case(state, run_dir, case):
    if case not in formula_cases():
        raise Refusal("unknown integration case")
    if any(result["case_id"] == case for result in state["results"]):
        raise Refusal("case already has a result; use a new run-id for a rerun")
    stage = run_dir / "installed"
    provider = "typesafe" if case.startswith("typesafe") else "anthropic"
    model = "jev" if provider == "typesafe" else "haiku"
    config = run_dir / "config"
    config.mkdir(exist_ok=True)
    (config / "default.json").write_text(json.dumps({"log_root": str(run_dir / "logs")}))
    request = run_dir / "request.json"
    request.write_text(json.dumps({"model": "jev-latest", "state": "A door is open.", "questions": {"q": {"type": "noul"}}}))
    models = "models" in case
    if case.endswith("-cli"):
        binary = stage / "bin" / ("decidr.exe" if sys.platform == "win32" else "decidr")
        cmd = [str(binary), "--config-dir", str(config), "models"] if models else [str(binary), "--config-dir", str(config), "send", "--model", model, "--provider", provider, "--project", "integration", "--agent-role", "prerelease", str(request)]
    else:
        cmd = [str(runtime_python(stage)), str(ROOT / "scripts/integration/wheel_case.py"), "models" if models else "send", "--config-dir", str(config), "--request", str(request), "--provider", provider, "--model", model]
    env = dict(os.environ)
    invalid = "invalid-key" in case
    marker = "invalid-integration-" + secrets.token_hex(16) if invalid else None
    if invalid:
        # The isolated working directory has no repository-root .env; the invalid ENV key is authoritative.
        env["TYPESAFE_API_KEY" if provider == "typesafe" else "ANTHROPIC_API_KEY"] = marker
    try:
        with tempfile.TemporaryDirectory(prefix="integration-no-dotenv-") as cwd:
            result = subprocess.run(cmd, cwd=cwd if invalid else ROOT, env=env, text=True, capture_output=True, check=False, timeout=240)
        output = result.stdout + "\n" + result.stderr
    except subprocess.TimeoutExpired:
        result = None
        output = ""

    leaked = marker and (marker in output or any(marker.encode() in path.read_bytes() for path in (run_dir / "logs").rglob("*.jsonl")))
    if result is None:
        status, reason, session = "FAIL", "Installed provider operation exceeded its bounded execution guard", None
    elif leaked:
        status, reason, session = "FAIL", "Credential disclosure detected; raw output was discarded", None
    else:
        status, reason, session = result_outcome(case, result.stdout.strip() or result.stderr.strip(), result.returncode)
    if case.endswith("-wheel"):
        artifact = next(a for a in state["artifacts"] if a["name"].startswith("sc_decidr-") and a["name"].endswith(".whl"))
    else:
        artifact = next(a for a in state["artifacts"] if a["name"].endswith((".tar.gz", ".zip")))
    record = {"case_id": case, "platform": state["platform"], "status": status, "reason": reason, "prerelease_tag": state["prerelease_tag"], "source_commit": state["source_commit"], "artifact_sha256": artifact["sha256"]}
    if session is not None:
        record["session_id"] = session
    state["results"].append(record)
    (run_dir / "run.json").write_text(json.dumps(state, indent=2) + "\n")
    if status == "PASS":
        subprocess.run(["bd", "close", state["beads"][case], "--reason", reason], check=True)
    else:
        subprocess.run(["bd", "update", state["beads"][case], "--append-notes", f"{status}: {reason}"], check=True)
    print(json.dumps(record))


def render_report(state, run_dir, template, report):
    vars_path = run_dir / "report-vars.json"
    vars_path.write_text(json.dumps(state, indent=2) + "\n")
    subprocess.run(["sc-compose", "render", "--file", str(template), "--var-file", str(vars_path), "--strict", "--output", str(report)], cwd=ROOT, check=True)
    import xml.etree.ElementTree as ET
    if ET.fromstring(report.read_bytes()).tag != "{http://www.w3.org/1999/xhtml}html":
        raise Refusal("rendered report must be well-formed XHTML")


def publish_report(state, run_dir, template):
    if not template.is_file():
        raise Refusal("owner-selected XHTML template is unavailable; report acceptance remains open")
    if {result["case_id"] for result in state["results"]} != set(formula_cases()):
        raise Refusal("run every required case before publishing its platform report")
    directory = ROOT / "site/reports/integration"
    directory.mkdir(parents=True, exist_ok=True)
    report = directory / f"{state['run_id']}.html"
    render_report(state, run_dir, template, report)
    state["report"] = str(report.relative_to(ROOT))
    state["report_sha256"] = digest(report)
    record = directory / f"{state['run_id']}.json"
    record.write_text(json.dumps({key: value for key, value in state.items() if key not in {"beads", "formula_root"}}, indent=2) + "\n")
    git("add", "--", str(report.relative_to(ROOT)), str(record.relative_to(ROOT)))
    git("commit", "-m", f"integration: record {state['platform']} run {state['run_id']}")
    git("push", "origin", "HEAD")
    passed = all(result["status"] == "PASS" for result in state["results"])
    subprocess.run(["bd", "close", state["beads"]["report"], "--reason", "Platform XHTML report committed and pushed"], check=True)
    if passed:
        subprocess.run(["bd", "close", state["formula_root"], "--reason", "All required live provider cases passed; platform report committed and pushed"], check=True)
    return passed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--run-dir", required=True, type=Path)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--case", choices=formula_cases())
    parser.add_argument("--template", type=Path, default=ROOT / "templates/integration-report/platform.xhtml.j2")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--report-only", action="store_true")
    args = parser.parse_args()
    try:
        state = prepare(args)
        if args.prepare_only:
            return 0
        if not args.report_only:
            completed = {result["case_id"] for result in state["results"]}
            for case in [args.case] if args.case else [case for case in formula_cases() if case not in completed]:
                execute_case(state, args.run_dir.resolve(), case)
        if args.case:
            return 0 if state["results"][-1]["status"] == "PASS" else 1
        return 0 if publish_report(state, args.run_dir.resolve(), args.template) else 1
    except (Exception, SystemExit) as error:
        reason = str(error) if isinstance(error, Refusal) else f"{type(error).__name__}: integration could not complete; inspect local execution before retrying"
        print(json.dumps({"status": "FAIL", "error": "integration.cannot_complete", "reason": reason}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
