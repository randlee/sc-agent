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
    assert set(sc_agent.integration_cases()) == {step["id"] for step in formula["steps"] if step["id"] not in {"verify-assets", "report"}}
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
