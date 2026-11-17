"""Render integration panels, their run page, and the newest-first reports index."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET


def report_values(junit, *, platform, source_revision, wheel_revision, mode, run_id):
    cases = []
    for case in ET.parse(junit).getroot().iter("testcase"):
        failure = case.find("failure")
        error = case.find("error")
        skipped = case.find("skipped")
        status = "FAIL" if failure is not None or error is not None else "SKIP" if skipped is not None else "PASS"
        # Do not copy failure messages/stdout: a failing redaction assertion could expose a key.
        # Preserve the result and exception type; raw diagnostics remain in the local JUnit file.
        reason = (failure if failure is not None else error)
        detail = reason.get("type", "failure") if reason is not None else "missing key or explicit test skip" if skipped is not None else "assertions passed"
        properties = {p.get("name"): p.get("value") for p in case.findall("properties/property")
                      if p.get("name") in {"provider", "model", "resolved_model", "mode", "result"}}
        cases.append({"case": case.get("name", ""), "status": status, "detail": detail, **properties})
    identity = dict(run_id=run_id, platform=platform, source_revision=source_revision,
                    wheel_revision=wheel_revision, mode=mode)
    return dict(title=f"sc-agent integration: {platform}", header_color="#1f2937", accent_color="#2563eb",
                fragment_source=f"pytest / {mode}", copy_json=json.dumps({**identity, "cases": cases}),
                copy_context=f"{platform} {run_id}", identity=[dict(label=k, value=v) for k, v in identity.items()], cases=cases)


def render_template(template, output, values):
    with tempfile.TemporaryDirectory(prefix="sc-agent-report-") as scratch:
        variables = Path(scratch) / "vars.json"
        variables.write_text(json.dumps(values), encoding="utf-8")
        subprocess.run(["sc-compose", "render", "--root", str(template.parent), "--file", str(template),
                        "--var-file", str(variables), "--strict", "--output", str(output)], check=True, timeout=60)


def new_run_id(reports_dir):
    day = datetime.now(timezone.utc).strftime("%Y%m%d")
    numbers = [int(match[1]) for path in Path(reports_dir).glob(f"{day}-*-integration.html")
               if (match := re.fullmatch(rf"{day}-(\d+)-integration", path.stem))]
    return f"{day}-{max(numbers, default=0) + 1}-integration"


def publish(panels, reports_dir, *, run_id=None, title="Integration report", created_at=None):
    """Publish rendered panels and rebuild the run page and newest-first index."""
    reports_dir = Path(reports_dir)
    run_id = run_id or new_run_id(reports_dir)
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", run_id):
        raise ValueError("run id must be a filename without path separators")
    directory = reports_dir / run_id
    directory.mkdir(parents=True, exist_ok=True)
    record = directory / f"{run_id}.json"
    state = json.loads(record.read_text(encoding="utf-8")) if record.exists() else dict(
        run_id=run_id, title=title, created_at=created_at if created_at is not None else
        datetime.now(timezone.utc).isoformat(), panels=[])
    names = set(state["panels"])
    for panel in map(Path, panels):
        ET.parse(panel)
        target = directory / panel.name
        if panel.resolve() != target.resolve():
            shutil.copyfile(panel, target)
        names.add(panel.name)
    state["panels"] = sorted(names)
    page = reports_dir / f"{run_id}.html"
    template = Path(__file__).with_name("page.html.j2")
    render_template(template, page, dict(title=state["title"], back_href="index.html", runs=[],
        panels=[dict(name=name, href=f"{run_id}/{name}", content=(directory / name).read_text(encoding="utf-8"))
                for name in state["panels"]]))
    record.write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")
    runs = [json.loads(path.read_text(encoding="utf-8")) for path in reports_dir.glob("*/*.json")
            if path.name == f"{path.parent.name}.json"]
    runs.sort(key=lambda run: (run["created_at"], run["run_id"]), reverse=True)
    render_template(template, reports_dir / "index.html", dict(
        title="Integration reports", back_href="../index.html", panels=[], runs=runs))
    return page


def render(junit, output, *, reports_dir=None, title="Integration report", created_at=None, **identity):
    template = Path(__file__).with_name("section-template.xhtml.j2")
    output = Path(output)
    reports_dir = Path(reports_dir) if reports_dir is not None else (
        output.parent.parent if output.parent.name == "integration" else output.parent)
    identity["run_id"] = identity.get("run_id") or new_run_id(reports_dir)
    values = report_values(junit, **identity)
    output.parent.mkdir(parents=True, exist_ok=True)
    render_template(template, output, values)
    ET.parse(output)
    return publish([output], reports_dir, run_id=identity["run_id"], title=title, created_at=created_at)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("junit", type=Path, nargs="?")
    parser.add_argument("--platform", choices=["macos", "colima"])
    parser.add_argument("--source-revision")
    parser.add_argument("--wheel-revision", help="Installed prerelease tag and source revision")
    parser.add_argument("--mode", choices=["live", "loopback"])
    parser.add_argument("--run-id")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--panel", type=Path, action="append", default=[])
    parser.add_argument("--reports-dir", type=Path)
    parser.add_argument("--title", default="Integration report")
    parser.add_argument("--created-at", help="UTC ISO timestamp for ordering; retained on a same-id rerun")
    args = parser.parse_args()
    if args.junit:
        if args.panel or not all((args.output, args.platform, args.source_revision, args.wheel_revision, args.mode)):
            parser.error("JUnit needs --output, --platform, --source-revision, --wheel-revision and --mode")
        render(args.junit, args.output, platform=args.platform, source_revision=args.source_revision,
               wheel_revision=args.wheel_revision, mode=args.mode, run_id=args.run_id,
               reports_dir=args.reports_dir, title=args.title, created_at=args.created_at)
    elif args.panel and args.reports_dir:
        publish(args.panel, args.reports_dir, run_id=args.run_id, title=args.title, created_at=args.created_at)
    else:
        parser.error("pass JUnit results or --panel with --reports-dir")
