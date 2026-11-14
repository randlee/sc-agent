"""Shared release-manifest and formula configuration for integration producer and gate."""
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[2]


class Refusal(Exception):
    """Actionable refusal: invalid configuration cannot become passing evidence."""


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
        raise Refusal("platform must identify exactly one declared release target")
    return matches[0]


def module_distribution(manifest, module="sc_agent"):
    packages = [p["package"] for p in manifest["python_packages"] if p["module"] == module]
    distributions = [d for d in manifest["python_distributions"] if d["name"] in packages]
    if len(distributions) != 1:
        raise Refusal("integration module must have exactly one declared Python distribution")
    return distributions[0]


def distribution_wheel(helper, names, distribution, version):
    wheels, _ = helper.distribution_assets(set(names), distribution["name"], version)
    if len(wheels) != 1:
        raise Refusal("expected exactly one selected platform wheel for the integration module")
    return wheels[0]


def case_specs(root=ROOT):
    formula = tomllib.loads((root / "formulas/integration.formula.toml").read_text())
    cases = {}
    for step in formula["steps"]:
        if step["id"] in {"verify_assets", "report"}:
            continue
        case = step["id"].replace("_", "-")
        spec = step.get("metadata", {}).get("integration")
        if not isinstance(spec, dict) or any(not isinstance(spec.get(k), str) or not spec[k] for k in ("provider", "model")):
            raise Refusal("each integration case requires provider/model formula metadata")
        if spec.get("interface") not in {"cli", "wheel"} or spec.get("operation") not in {"models", "send"} or spec.get("expectation") not in {"models", "noul", "auth_401"}:
            raise Refusal("case interface, operation and expectation must be supported")
        if spec["expectation"] == "auth_401" and (not isinstance(spec.get("credential_env"), str) or not spec["credential_env"] or not isinstance(spec.get("error_chain"), list) or not spec["error_chain"] or any(not isinstance(item, str) or not item for item in spec["error_chain"])):
            raise Refusal("invalid-key cases require a credential variable and expected error chain")
        if case in cases:
            raise Refusal("formula case identifiers must be unique")
        cases[case] = spec
    return cases
