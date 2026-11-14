#!/usr/bin/env python3
"""Derive the poured integration workflow's case steps from the packaged case records."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "crates/decidr/integration-cases.json"
OUTPUT = ROOT / "formulas/integration.formula.toml"


def render_formula() -> str:
    records = json.loads(MANIFEST.read_text(encoding="utf-8"))
    ids = [record["id"] for record in records]
    if len(ids) != len(set(ids)):
        raise ValueError("integration case ids must be unique")
    lines = [
        'formula = "integration"',
        'description = "Live checksum-verified sc-agent prerelease provider integration"',
        "version = 1",
        'type = "workflow"',
        "",
        "[[steps]]",
        'id = "verify_assets"',
        'title = "Verify prerelease assets for {{run_id}}"',
        'description = "Download and verify the target binary archive and both wheel SHA-256 values before any case."',
        'type = "task"',
        "",
    ]
    step_ids = []
    for record in records:
        case_id = record["id"]
        step_id = case_id.replace("-", "_")
        step_ids.append(step_id)
        description = f'Run scripts/integration/run.py --case {case_id}; persist its sanitized PASS, FAIL or SKIP result. SKIP never passes publication.'
        if record.get("operation") == "invalid-key":
            description += f' Expected authentication cause chain: {" -> ".join(record["error_chain"])}.'
        lines.extend([
            "[[steps]]",
            f"id = {json.dumps(step_id)}",
            f"title = {json.dumps('Live ' + case_id + ' for {{run_id}}')}",
            f"description = {json.dumps(description)}",
            'type = "task"',
            'needs = ["verify_assets"]',
            "",
        ])
    lines.extend([
        "[[steps]]",
        'id = "report"',
        'title = "Render and publish XHTML for {{run_id}}"',
        'description = "Render the owner-selected XHTML template with sc-compose; validate XML, commit and push this platform report and its per-case results."',
        'type = "task"',
        f"needs = [{', '.join(json.dumps(step_id) for step_id in step_ids)}]",
        "",
    ])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the checked-in formula is stale")
    args = parser.parse_args()
    generated = render_formula()
    if args.check:
        current = OUTPUT.read_text(encoding="utf-8")
        if current != generated:
            raise SystemExit("integration formula is stale; run scripts/integration/generate_formula.py")
        return 0
    OUTPUT.write_text(generated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
