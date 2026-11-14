"""Installed-wheel integration evidence boundary. These synthetic records are parser tests only."""
import json
from pathlib import Path
import tomllib
import pytest
import sc_agent

SOURCE = "a" * 40


def passing_records():
    return [dict(case_id=case, platform="macos-aarch64", status="PASS", reason="controlled parser case", prerelease_tag="prerelease/v0.1.1", source_commit=SOURCE, artifact_sha256="b" * 64) for case in sc_agent.integration_cases()]


def test_installed_rust_case_contract_matches_formula():
    root = Path(__file__).resolve().parents[3]
    formula = tomllib.loads((root / "formulas/integration.formula.toml").read_text())
    cases = {step["id"].replace("_", "-") for step in formula["steps"]} - {"verify-assets", "report"}
    assert set(sc_agent.integration_cases()) == cases
    records = passing_records()
    assert json.loads(sc_agent.validate_integration_results(json.dumps(records), SOURCE)) == records


@pytest.mark.parametrize("status", ["SKIP", "FAIL", "unknown"])
def test_nonpassing_case_is_typed_refusal(status):
    records = passing_records()
    records[0]["status"] = status
    with pytest.raises(sc_agent.SagError) as info:
        sc_agent.validate_integration_results(json.dumps(records), SOURCE)
    assert info.value.args[0] == "decidr.integration_evidence"
    assert json.loads(info.value.args[2])["id"] == "decidr.integration_evidence"


def test_missing_source_and_case_are_refused():
    records = passing_records()
    for invalid, source in [(records[:-1], SOURCE), (records, "c" * 40), (records, "short")]:
        with pytest.raises(sc_agent.SagError):
            sc_agent.validate_integration_results(json.dumps(invalid), source)


def test_owner_selected_report_is_xml_and_escapes_consumer_text(tmp_path):
    import sc_compose
    import xml.etree.ElementTree as ET
    root = Path(__file__).resolve().parents[3]
    text = 'consumer <script>& "text"'
    records = passing_records()
    records[0]["reason"] = text
    values = dict(run_id="controlled-report", platform="macos-aarch64", status="PASS", generated_at="2026-11-14T00:00:00Z", prerelease_tag="prerelease/v0.1.1", source_commit=SOURCE, artifacts=[dict(name="archive<&>.tar.gz", sha256="b" * 64)], results=records)
    request = sc_compose.ComposeRequest(root=root, mode=sc_compose.ComposeMode.file("templates/integration-report/platform.xhtml.j2"), vars_input=values, policy=sc_compose.ComposePolicy(strict_undeclared_variables=True))
    rendered = sc_compose.compose_file(request).rendered_text
    path = tmp_path / "platform.html"
    path.write_text(rendered)
    element = ET.parse(path).getroot()
    assert element.tag == "{http://www.w3.org/1999/xhtml}html"
    assert text in "".join(element.itertext())
    assert element.find(".//{http://www.w3.org/1999/xhtml}script") is None
    assert rendered.startswith('<?xml version="1.0" encoding="UTF-8"?>')
    assert '<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"' in rendered
    assert 'class="facts"' in rendered
