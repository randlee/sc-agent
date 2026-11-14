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

from integration_errors import failure_detail, redact
from configuration import Refusal, case_specs, distribution_wheel, integration_case_records, module_distribution, platform_target, release_manifest

ROOT = Path(__file__).resolve().parents[2]
REPORT_ROOT = PurePosixPath("site/reports/integration")
RUN_SCHEMA_VERSION = 1


def validate_run_schema(run):
    if not isinstance(run, dict) or type(run.get("schema_version")) is not int or run["schema_version"] != RUN_SCHEMA_VERSION:
        raise Refusal(
            'run record has no supported schema_version',
            code="integration.invalid_run_schema",
            remediation=f'Rerun integration with the current scripts to produce schema_version {RUN_SCHEMA_VERSION} evidence.',
        )


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



def checked_process(args, *, cwd=ROOT, capture=False):
    """Capture failure stderr; keep successful helper/installer console output available."""
    result = subprocess.run(args, cwd=cwd, check=True, text=True, capture_output=True,
                            stdin=subprocess.DEVNULL, timeout=60,
                            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    if not capture:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(redact(result.stderr), end="", file=sys.stderr)
    return result


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
    # The pinned helper stays unchanged; its subprocess seam supplies captured diagnostics.
    module.command = checked_process
    return module


def report_path(value):
    if not isinstance(value, str):
        raise Refusal('report path must be repository-relative', code="integration.invalid_report_path", remediation='Use a repository-relative .html path under site/reports/integration.')
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or not path.is_relative_to(REPORT_ROOT):
        raise Refusal('report must be inside site/reports/integration', code="integration.invalid_report_path", remediation='Use a repository-relative .html path under site/reports/integration without traversal.')
    return path


def check_source_history(source, candidate, changed_paths):
    # The caller proves ancestry separately; only report descendants may be accepted.
    if source != candidate and any(not PurePosixPath(path).is_relative_to(REPORT_ROOT) for path in changed_paths):
        raise Refusal('candidate changed production or release source after the tested revision; rerun integration', code="integration.stale_source", remediation='Publish and test the candidate prerelease, then commit and push its live report without further source changes.')


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
        raise Refusal('remote prerelease tag does not point to the tested source', code="integration.tag_source_mismatch", remediation='Select a run whose tested source equals the remote immutable prerelease tag.')
    target = platform_target(manifest, run["platform"])
    target_name = target["target"]
    release_names = helper.release_asset_names(release)
    expected_names = {helper.archive_name(manifest, version, target)}
    for distribution in manifest["python_distributions"]:
        declaration = next((wheel for wheel in distribution["wheels"] if wheel["target"] == target_name), None)
        if declaration is None:
            raise Refusal('reported platform has no declared wheel for each distribution', code="integration.missing_platform_wheel", remediation='Declare and publish a compatible platform wheel for every distribution.')
        wheels, _ = helper.distribution_assets(release_names, distribution["name"], version)
        matching = [name for name in wheels if declaration["platform"] in helper.parse_wheel(name)["platform"]]
        if len(matching) != 1:
            raise Refusal('platform wheel is missing or ambiguous', code="integration.ambiguous_platform_wheel", remediation='Publish exactly one compatible wheel for each platform distribution.')
        expected_names.add(matching[0])
    artifacts = run["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != 1 + len(manifest["python_distributions"]):
        raise Refusal('verified target archive and every declared Python distribution are required', code="integration.incomplete_artifacts", remediation='Record the verified target binary archive and every declared Python distribution.')
    names = [artifact["name"] for artifact in artifacts]
    if set(names) != expected_names:
        raise Refusal("reported artifact set does not match the tested platform's archive and wheels", code="integration.artifact_set_mismatch", remediation='Select the declared binary archive and wheels for the tested platform.')
    if len(names) != len(set(names)) or any(Path(name).name != name for name in names):
        raise Refusal('artifact names must be unique basenames', code="integration.invalid_artifact_names", remediation='Use unique release-asset basenames with no directory components.')
    with tempfile.TemporaryDirectory(prefix="sc-agent-integration-preflight-") as directory:
        checksums = helper.checksums(helper.download_asset(tag, "checksums.txt", Path(directory)).read_text())
        for artifact in artifacts:
            name = artifact["name"]
            if checksums.get(name) != artifact["sha256"]:
                raise Refusal('result artifact digest differs from published checksums', code="integration.artifact_checksum_mismatch", remediation='Download the published checksums and verified assets again; rerun cases against those assets.')
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
        subprocess.run([str(python), "-m", "pip", "install", "--no-index", "--no-deps", str(wheel)], check=True, text=True, capture_output=True)
        def validate(results, source):
            code = "import json,sys,sc_agent; value=json.load(sys.stdin); print(sc_agent.validate_integration_results(value['results'], value['source']))"
            result = subprocess.run([str(python), "-I", "-c", code], input=json.dumps({"results": results, "source": source}), text=True, capture_output=True, check=True)
            return result.stdout
        yield validate


def validate_run(run, candidate, *, root=ROOT, validator=None, artifact_verifier=verify_artifacts):
    validate_run_schema(run)
    if validator is None:
        with installed_validator(run) as candidate_validator:
            return validate_run(run, candidate, root=root, validator=candidate_validator, artifact_verifier=artifact_verifier)
    results = json.loads(validator(json.dumps(run["results"]), run["source_commit"]))
    # The Rust boundary has checked record values; the enclosing run must agree with them.
    for result in results:
        if any(result[field] != run[field] for field in ("source_commit", "platform", "prerelease_tag")):
            raise Refusal('run identity differs from its case results', code="integration.run_identity_mismatch", remediation='Use case results from the same source, platform and prerelease tag as the run.')
    artifacts = run["artifacts"]
    verified = {artifact["sha256"] for artifact in artifacts}
    if any(result["artifact_sha256"] not in verified for result in results):
        raise Refusal('case names an artifact not verified by this run', code="integration.unverified_case_artifact", remediation='Verify and record the artifact used by this case before publishing its results.')
    for result in results:
        case = case_specs(ROOT).get(result["case_id"])
        if case is None:
            raise Refusal('unknown integration case', code="integration.unknown_case", remediation='Select a case declared by the integration case manifest.')
        if case["interface"] == "wheel":
            helper = prerelease_helper()
            manifest = release_manifest(root)
            version = run["prerelease_tag"].removeprefix(manifest["prerelease"]["tag_prefix"])
            name = distribution_wheel(helper, [a["name"] for a in artifacts], module_distribution(manifest), version)
            selected = [a for a in artifacts if a["name"] == name]
        else:
            selected = [a for a in artifacts if a["name"].endswith((".tar.gz", ".zip"))]
        if len(selected) != 1 or selected[0]["sha256"] != result["artifact_sha256"]:
            raise Refusal('case is bound to the wrong binary/wheel artifact', code="integration.case_artifact_mismatch", remediation='Bind the case to the archive or wheel actually used and rerun it if necessary.')
    git("merge-base", "--is-ancestor", run["source_commit"], candidate, root=root)
    changes = git("diff", "--name-only", run["source_commit"], candidate, root=root).splitlines()
    check_source_history(run["source_commit"], candidate, changes)
    report = report_path(run["report"])
    if report.suffix != ".html":
        raise Refusal('report must be XHTML in an .html file', code="integration.invalid_report_path", remediation='Render the selected XHTML template into a repository-relative .html report.')
    path = root / str(report)
    if digest(path) != run["report_sha256"]:
        raise Refusal('XHTML checksum differs from recorded report', code="integration.report_checksum_mismatch", remediation='Render and hash the final XHTML, then commit it together with its case results.')
    element = ET.fromstring(path.read_bytes())
    if element.tag != "{http://www.w3.org/1999/xhtml}html":
        raise Refusal('report must be well-formed XHTML with the XHTML namespace', code="integration.invalid_xhtml", remediation='Render well-formed XHTML with the XHTML namespace using the selected template.')
    git("ls-files", "--error-unmatch", "--", str(report), root=root)
    git("diff", "--exit-code", "HEAD", "--", str(report), root=root)
    commit = git("log", "-1", "--format=%H", "--", str(report), root=root)
    if not commit or not pushed_ref_contains(commit, root=root):
        raise Refusal('platform report has not been committed and pushed to origin', code="integration.report_not_pushed", remediation='Commit the report and case results together and push the commit to origin.')
    artifact_verifier(run)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", default=None)
    args = parser.parse_args()
    import os
    try:
        candidate = args.candidate or os.environ.get("CANDIDATE_SHA") or git("rev-parse", "HEAD")
        # Refresh only remote branch evidence; never infer publication from an unpushed local commit.
        git("fetch", "--prune", "origin")
    except (Exception, SystemExit) as error:
        detail = failure_detail(error)
        print(json.dumps({"status": "FAIL", "error": detail["id"], "reason": detail["message"], "remediation": detail["remediation"], "detail": detail}))
        return 1
    reasons = []
    failures = []
    for path in sorted((ROOT / str(REPORT_ROOT)).glob("*.json")):
        try:
            git("ls-files", "--error-unmatch", "--", str(path.relative_to(ROOT)))
            git("diff", "--exit-code", "HEAD", "--", str(path.relative_to(ROOT)))
            run = json.loads(path.read_text())
            validate_run_schema(run)
            record_commit = git("log", "-1", "--format=%H", "--", str(path.relative_to(ROOT)))
            report_commit = git("log", "-1", "--format=%H", "--", str(report_path(run["report"])))
            if record_commit != report_commit or not pushed_ref_contains(record_commit):
                raise Refusal('case results and XHTML must be committed together and pushed to origin', code="integration.report_not_pushed", remediation='Commit the report and case results together and push the commit to origin.')
            validate_run(run, candidate)
            print(json.dumps({"status": "PASS", "report": run["report"], "source_commit": run["source_commit"]}))
            return 0
        except (Exception, SystemExit) as error:
            # No provider text or credential bytes are included in gate errors.
            detail = failure_detail(error)
            reasons.append(f"{path.name}: {detail['message']}")
            failures.append({"report": path.name, "error": detail})
    print(json.dumps({"status": "FAIL", "error": "integration.no_passing_platform", "reason": "No complete passing platform matches candidate source and verified artifacts", "reports": reasons, "failures": failures, "remediation": "Resolve the per-report failures, or run and publish a complete matching live integration report."}))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
