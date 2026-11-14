#!/usr/bin/env python3
"""Fail-closed live integration gate over committed reports and verified prerelease artifacts."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

from configuration import Refusal, case_specs, distribution_wheel, integration_case_records, module_distribution, platform_target, release_manifest

ROOT = Path(__file__).resolve().parents[2]
REPORT_ROOT = PurePosixPath("site/reports/integration")


def command(args, *, cwd=ROOT):
    env = {**os.environ, "GIT_TERMINAL_PROMPT": "0"}
    result = subprocess.run(
        args,
        cwd=cwd,
        check=True,
        text=True,
        capture_output=True,
        stdin=subprocess.DEVNULL,
        timeout=60,
        env=env,
    )
    return result.stdout.strip()


def git(*args, root=ROOT):
    return command(["git", *args], cwd=root)


def pushed_ref_contains(commit, *, root=ROOT):
    return bool(git("for-each-ref", f"--contains={commit}", "--format=%(refname)", "refs/remotes/origin/", root=root))


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def prerelease_helper():
    path = ROOT / ".claude/skills/prerelease/scripts/prerelease.py"
    spec = importlib.util.spec_from_file_location("integration_prerelease", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def report_path(value):
    if not isinstance(value, str):
        raise Refusal("report path must be repository-relative")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or not path.is_relative_to(REPORT_ROOT):
        raise Refusal("report must be inside site/reports/integration")
    return path


def check_source_history(source, candidate, changed_paths):
    # The caller proves ancestry separately; only report descendants may be accepted.
    if source != candidate and any(not PurePosixPath(path).is_relative_to(REPORT_ROOT) for path in changed_paths):
        raise Refusal("candidate changed production or release source after the tested revision; rerun integration")


def verify_artifacts(run, *, helper=None):
    helper = helper or prerelease_helper()
    tag = run["prerelease_tag"]
    release = helper.release_for_tag(tag)
    manifest = release_manifest(ROOT)
    version = tag.removeprefix(manifest["prerelease"]["tag_prefix"])
    helper.require_release_assets(release, manifest, version)
    # Resolve the remote tag, not a local tag that could have been replaced.
    remote = git("ls-remote", "--tags", "origin", f"refs/tags/{tag}", f"refs/tags/{tag}^{{}}")
    refs = dict(line.split()[::-1] for line in remote.splitlines())
    remote_source = refs.get(f"refs/tags/{tag}^{{}}", refs.get(f"refs/tags/{tag}"))
    if remote_source != run["source_commit"]:
        raise Refusal("remote prerelease tag does not point to the tested source")
    target = platform_target(manifest, run["platform"])
    target_name = target["target"]
    release_names = helper.release_asset_names(release)
    expected_names = {helper.archive_name(manifest, version, target)}
    for distribution in manifest["python_distributions"]:
        declaration = next((wheel for wheel in distribution["wheels"] if wheel["target"] == target_name), None)
        if declaration is None:
            raise Refusal("reported platform has no declared wheel for each distribution")
        wheels, _ = helper.distribution_assets(release_names, distribution["name"], version)
        matching = [name for name in wheels if declaration["platform"] in helper.parse_wheel(name)["platform"]]
        if len(matching) != 1:
            raise Refusal("platform wheel is missing or ambiguous")
        expected_names.add(matching[0])
    artifacts = run["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != 1 + len(manifest["python_distributions"]):
        raise Refusal("verified target archive and every declared Python distribution are required")
    names = [artifact["name"] for artifact in artifacts]
    if set(names) != expected_names:
        raise Refusal("reported artifact set does not match the tested platform's archive and wheels")
    if len(names) != len(set(names)) or any(Path(name).name != name for name in names):
        raise Refusal("artifact names must be unique basenames")
    with tempfile.TemporaryDirectory(prefix="sc-agent-integration-preflight-") as directory:
        checksums = helper.checksums(helper.download_asset(tag, "checksums.txt", Path(directory)).read_text())
        for artifact in artifacts:
            name = artifact["name"]
            if checksums.get(name) != artifact["sha256"]:
                raise Refusal("result artifact digest differs from published checksums")
            helper.verify_checksum(helper.download_asset(tag, name, Path(directory)), artifact["sha256"])


@contextmanager
def installed_validator(run):
    """Use a checksum-verified host wheel from the tested source, even on another platform."""
    import venv
    helper = prerelease_helper()
    tag = run["prerelease_tag"]
    manifest = release_manifest(ROOT)
    version = tag.removeprefix(manifest["prerelease"]["tag_prefix"])
    release = helper.release_for_tag(tag)
    names = helper.host_wheel_names(helper.release_asset_names(release), manifest, version)
    wheel_name = distribution_wheel(helper, names, module_distribution(manifest), version)
    with tempfile.TemporaryDirectory(prefix="integration-validator-") as directory:
        root = Path(directory)
        checksums = helper.checksums(helper.download_asset(tag, "checksums.txt", root).read_text())
        wheel = helper.download_asset(tag, wheel_name, root)
        helper.verify_checksum(wheel, checksums[wheel_name])
        venv.create(root / "venv", with_pip=True)
        python = root / "venv" / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        subprocess.run([str(python), "-m", "pip", "install", "--no-index", "--no-deps", str(wheel)], check=True)
        def validate(results, source):
            code = "import json,sys,sc_agent; value=json.load(sys.stdin); print(sc_agent.validate_integration_results(value['results'], value['source']))"
            result = subprocess.run([str(python), "-I", "-c", code], input=json.dumps({"results": results, "source": source}), text=True, capture_output=True, check=True)
            return result.stdout
        yield validate


def validate_run(run, candidate, *, root=ROOT, validator=None, artifact_verifier=verify_artifacts):
    if validator is None:
        with installed_validator(run) as candidate_validator:
            return validate_run(run, candidate, root=root, validator=candidate_validator, artifact_verifier=artifact_verifier)
    results = json.loads(validator(json.dumps(run["results"]), run["source_commit"]))
    # The Rust boundary has checked record values; the enclosing run must agree with them.
    for result in results:
        if any(result[field] != run[field] for field in ("source_commit", "platform", "prerelease_tag")):
            raise Refusal("run identity differs from its case results")
    artifacts = run["artifacts"]
    verified = {artifact["sha256"] for artifact in artifacts}
    if any(result["artifact_sha256"] not in verified for result in results):
        raise Refusal("case names an artifact not verified by this run")
    for result in results:
        case = case_specs(ROOT).get(result["case_id"])
        if case is None:
            raise Refusal("unknown integration case")
        if case["interface"] == "wheel":
            helper = prerelease_helper()
            manifest = release_manifest(root)
            version = run["prerelease_tag"].removeprefix(manifest["prerelease"]["tag_prefix"])
            name = distribution_wheel(helper, [a["name"] for a in artifacts], module_distribution(manifest), version)
            selected = [a for a in artifacts if a["name"] == name]
        else:
            selected = [a for a in artifacts if a["name"].endswith((".tar.gz", ".zip"))]
        if len(selected) != 1 or selected[0]["sha256"] != result["artifact_sha256"]:
            raise Refusal("case is bound to the wrong binary/wheel artifact")
    git("merge-base", "--is-ancestor", run["source_commit"], candidate, root=root)
    changes = git("diff", "--name-only", run["source_commit"], candidate, root=root).splitlines()
    check_source_history(run["source_commit"], candidate, changes)
    report = report_path(run["report"])
    if report.suffix != ".html":
        raise Refusal("report must be XHTML in an .html file")
    path = root / str(report)
    if digest(path) != run["report_sha256"]:
        raise Refusal("XHTML checksum differs from recorded report")
    element = ET.fromstring(path.read_bytes())
    if element.tag != "{http://www.w3.org/1999/xhtml}html":
        raise Refusal("report must be well-formed XHTML with the XHTML namespace")
    git("ls-files", "--error-unmatch", "--", str(report), root=root)
    git("diff", "--exit-code", "HEAD", "--", str(report), root=root)
    commit = git("log", "-1", "--format=%H", "--", str(report), root=root)
    if not commit or not pushed_ref_contains(commit, root=root):
        raise Refusal("platform report has not been committed and pushed to origin")
    artifact_verifier(run)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", default=None)
    args = parser.parse_args()
    import os
    candidate = args.candidate or os.environ.get("CANDIDATE_SHA") or git("rev-parse", "HEAD")
    # Refresh only remote branch evidence; never infer publication from an unpushed local commit.
    git("fetch", "--prune", "origin")
    reasons = []
    for path in sorted((ROOT / str(REPORT_ROOT)).glob("*.json")):
        try:
            git("ls-files", "--error-unmatch", "--", str(path.relative_to(ROOT)))
            git("diff", "--exit-code", "HEAD", "--", str(path.relative_to(ROOT)))
            run = json.loads(path.read_text())
            record_commit = git("log", "-1", "--format=%H", "--", str(path.relative_to(ROOT)))
            report_commit = git("log", "-1", "--format=%H", "--", str(report_path(run["report"])))
            if record_commit != report_commit or not pushed_ref_contains(record_commit):
                raise Refusal("case results and XHTML must be committed together and pushed to origin")
            validate_run(run, candidate)
            print(json.dumps({"status": "PASS", "report": run["report"], "source_commit": run["source_commit"]}))
            return 0
        except (Exception, SystemExit) as error:
            # No provider text or credential bytes are included in gate errors.
            reasons.append(f"{path.name}: {str(error) if isinstance(error, Refusal) else type(error).__name__}")
    print(json.dumps({"status": "FAIL", "error": "integration.no_passing_platform", "reason": "No complete passing platform matches candidate source and verified artifacts", "reports": reasons}))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
