"""Shared release and case-record configuration for integration producer and gate."""
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[2]


from integration_errors import Refusal


def release_manifest(root=ROOT):
    return tomllib.loads((root / "release/publish-artifacts.toml").read_text())


def platform_target(manifest, platform):
    """Accept declared wheel IDs, target triples and their architecture-spelled aliases."""
    matches = []
    for target in manifest["release_targets"]:
        triple = target["target"]
        aliases = {triple}
        for distribution in manifest["python_distributions"]:
            for wheel in distribution["wheels"]:
                if wheel["target"] == triple:
                    aliases.add(wheel["id"])
                    family, _, _ = wheel["id"].rpartition("-")
                    aliases.add(f"{family}-{triple.split('-', 1)[0]}")
        if platform in aliases:
            matches.append(target)
    if len(matches) != 1:
        raise Refusal('platform must identify exactly one declared release target', code="integration.invalid_platform", remediation='Select one platform declared by the release manifest.')
    return matches[0]


def module_distribution(manifest, module="sc_agent"):
    packages = [p["package"] for p in manifest["python_packages"] if p["module"] == module]
    distributions = [d for d in manifest["python_distributions"] if d["name"] in packages]
    if len(distributions) != 1:
        raise Refusal('integration module must have exactly one declared Python distribution', code="integration.invalid_distribution_configuration", remediation='Declare exactly one Python distribution for the integration module in the release manifest.')
    return distributions[0]


def distribution_wheel(helper, names, distribution, version):
    wheels, _ = helper.distribution_assets(set(names), distribution["name"], version)
    if len(wheels) != 1:
        raise Refusal('expected exactly one selected platform wheel for the integration module', code="integration.ambiguous_platform_wheel", remediation='Select exactly one compatible wheel for the integration module.')
    return wheels[0]


def integration_case_records(root=ROOT):
    records = json.loads((root / "crates/decidr/integration-cases.json").read_text(encoding="utf-8"))
    if not isinstance(records, list) or any(not isinstance(record, dict) or not isinstance(record.get("id"), str) for record in records):
        raise Refusal('integration case manifest must be an array of records with string ids', code="integration.invalid_case_manifest", remediation='Correct integration-cases.json to an array of case records with string ids.')
    indexed = {record["id"]: record for record in records}
    if len(indexed) != len(records):
        raise Refusal('integration case identifiers must be unique', code="integration.duplicate_case_id", remediation='Give each integration case a unique id and regenerate the formula.')
    return indexed


def case_specs(root=ROOT):
    cases = {}
    for case, record in integration_case_records(root).items():
        operation = record["operation"]
        provider = record["provider"]
        if record.get("client") not in {"cli", "wheel"} or record.get("artifact") not in {"archive", "sc-decidr-wheel"}:
            raise Refusal('case client and artifact must be supported', code="integration.invalid_case_configuration", remediation='Select a supported cli/wheel client and its matching archive/wheel artifact.')
        if provider not in {"typesafe", "anthropic"} or not isinstance(record.get("model"), str) or not record["model"]:
            raise Refusal('case provider and model must be supported', code="integration.invalid_case_configuration", remediation='Configure a supported provider and non-empty model for each integration case.')
        if operation not in {"models", "decision", "invalid-key"}:
            raise Refusal('case operation must be supported', code="integration.invalid_case_configuration", remediation='Select a supported models, decision or invalid-key operation.')
        spec = {
            "provider": provider,
            "model": record["model"],
            "interface": record["client"],
            "operation": "models" if operation == "models" else "send",
            "expectation": {"models": "models", "decision": "noul", "invalid-key": "auth_401"}[operation],
        }
        if operation == "invalid-key":
            spec["credential_env"] = "TYPESAFE_API_KEY" if provider == "typesafe" else "ANTHROPIC_API_KEY"
            spec["error_chain"] = (
                ["decision.api_status"]
                if provider == "typesafe"
                else ["decision.backend", "decision_llm.llm", "llm.api_status"]
            )
        cases[case] = spec
    return cases
