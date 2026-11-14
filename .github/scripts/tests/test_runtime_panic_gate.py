"""Exercise the installed SCB-RUNTIME-008 analyzer against controlled Rust source."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]


class RuntimePanicGate(unittest.TestCase):
    def test_release_manifest_does_not_select_source_install(self):
        manifest = tomllib.loads((ROOT / "release/publish-artifacts.toml").read_text())
        self.assertNotIn("sc_lint_source_revision", manifest["project"])
        consumer_input = json.loads((ROOT / "release/sc-publish-consumer-input.json").read_text())
        self.assertNotIn("sc_lint_source_revision", consumer_input["project"])

    def test_every_production_package_is_opted_in(self):
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
        names = {
            tomllib.loads((ROOT / member / "Cargo.toml").read_text())["package"]["name"]
            for member in workspace["members"]
        }
        # This crate contains only hang guards and doubles for test consumers.
        names.remove("sc-agent-testkit")
        for member in workspace["members"]:
            manifest = tomllib.loads((ROOT / member / "Cargo.toml").read_text())
            sections = [manifest, *manifest.get("target", {}).values()]
            for section in sections:
                for kind in ["dependencies", "build-dependencies"]:
                    self.assertNotIn("sc-agent-testkit", section.get(kind, {}), f"{member} acquired a production testkit dependency")
        config = tomllib.loads((ROOT / "sc-lint.toml").read_text())
        self.assertEqual(set(config["tool"]["sc-lint"]["analyzers"]["runtime"]["panic_rule_packages"]), names)

    def test_ci_report_runs_runtime_rule_with_complete_workspace_coverage(self):
        report_path = os.environ.get("SC_LINT_RUNTIME_REPORT")
        self.assertTrue(report_path, "CI must pass the JSON report from its live sc-lint invocation")
        report = json.loads(Path(report_path).read_text())

        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
        package_names = {
            tomllib.loads((ROOT / member / "Cargo.toml").read_text())["package"]["name"]
            for member in workspace["members"]
        }
        package_names.remove("sc-agent-testkit")
        runtime_config = tomllib.loads((ROOT / "sc-lint.toml").read_text())
        configured = (
            runtime_config["tool"]["sc-lint"]["analyzers"]["runtime"]["panic_rule_packages"]
        )

        self.assertEqual(report["command"], "lint.sc-runtime", report)
        self.assertTrue(report["ok"], report)
        data = report["data"]
        self.assertEqual(data["tool"], "sc-lint-runtime", report)
        self.assertEqual(data["version"], "0.8.9", report)
        self.assertEqual(data["status"], "pass", report)
        self.assertEqual(data["findings"], [], report)
        self.assertEqual(set(configured), package_names)
        self.assertEqual(len(configured), 16)
        self.assertEqual(data["scanned_crates"], len(workspace["members"]), report)
        print(
            f"SCB-RUNTIME-008 configured for all {len(configured)} production packages; "
            f"sc-lint {data['version']} scanned all {data['scanned_crates']} workspace crates.",
            flush=True,
        )

    def test_all_six_production_constructs_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            (root / "boundaries").mkdir()
            (root / "Cargo.toml").write_text('[package]\nname="negative"\nversion="0.1.0"\nedition="2024"\n')
            (root / "sc-lint.toml").write_text('[tool.sc-lint.analyzers.runtime]\npanic_rule_packages=["negative"]\n')
            (root / "src/lib.rs").write_text('''
pub fn a(x: Option<u8>) -> u8 { x.unwrap() }
pub fn b(x: Option<u8>) -> u8 { x.expect("bad") }
pub fn c() { panic!("bad"); }
pub fn d() { unreachable!("bad"); }
pub fn e() { todo!("bad"); }
pub fn f() { unimplemented!("bad"); }
#[cfg(test)]
mod tests { fn helper() { panic!("test only"); } }
''')
            result = subprocess.run(
                [os.environ.get("SC_LINT", "sc-lint"), "--root", str(root), "--json", "lint", "sc-runtime", "--rule-filter", "SCB-RUNTIME-008"],
                capture_output=True, text=True, check=False, timeout=60,
            )
            self.assertEqual(result.returncode, 10, (result.stdout, result.stderr))
            report = json.loads(result.stdout)
            self.assertTrue(report["ok"])
            findings = report["data"]["findings"]
            self.assertEqual(len(findings), 6, findings)
            self.assertTrue(all(item["rule_id"] == "SCB-RUNTIME-008" for item in findings))
            for construct in [".unwrap", ".expect", "panic!", "unreachable!", "todo!", "unimplemented!"]:
                self.assertTrue(any(construct in item["message"] for item in findings), construct)


if __name__ == "__main__":
    unittest.main()
