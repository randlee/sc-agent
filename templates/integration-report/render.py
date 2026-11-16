"""Render one platform's pytest JUnit results through the supplied XHTML template."""
import argparse
from datetime import datetime, timezone
import html
import json
from pathlib import Path
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
    rows = "".join("<tr>" + "".join(f"<td>{html.escape(str(case.get(k, '')))}</td>" for k in
                                  ("case", "provider", "model", "status", "result", "detail")) + "</tr>" for case in cases)
    metadata = "".join(f"<p><strong>{html.escape(k)}</strong>: {html.escape(str(v))}</p>" for k, v in identity.items())
    body = metadata + '<table><thead><tr><th>Case</th><th>Provider</th><th>Model</th><th>Result</th><th>Observed</th><th>Detail</th></tr></thead><tbody>' + rows + '</tbody></table>'
    return dict(title=f"sc-agent integration: {platform}", header_color="#1f2937", accent_color="#2563eb",
                fragment_source=f"pytest / {mode}", copy_json=json.dumps({**identity, "cases": cases}),
                copy_context=f"{platform} {run_id}", body_html=body)


def render(junit, output, **identity):
    template = Path(__file__).with_name("section-template.xhtml.j2")
    values = report_values(junit, **identity)
    output = Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sc-agent-report-") as scratch:
        variables = Path(scratch) / "vars.json"
        variables.write_text(json.dumps(values))
        subprocess.run(["sc-compose", "render", "--root", str(template.parent), "--file", str(template),
                        "--var-file", str(variables), "--strict", "--output", str(output)], check=True, timeout=60)
    ET.parse(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("junit", type=Path)
    parser.add_argument("--platform", choices=["macos", "colima"], required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--wheel-revision", required=True, help="Installed prerelease tag and source revision")
    parser.add_argument("--mode", choices=["live", "loopback"], required=True)
    parser.add_argument("--run-id", default=datetime.now(timezone.utc).isoformat())
    parser.add_argument("--output", type=Path, required=True)
    options = vars(parser.parse_args())
    junit, output = options.pop("junit"), options.pop("output")
    render(junit, output, **options)
