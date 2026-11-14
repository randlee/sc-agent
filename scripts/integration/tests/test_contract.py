"""Deterministic integration composition tests. Fixture outcomes are never live acceptance evidence."""
import base64
import json
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parents[1]
sys.path.insert(0, str(SCRIPTS))
import preflight
import remote
import run


class FormulaAndTransport(unittest.TestCase):
    def test_formula_renders_through_sc_compose(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "input.formula.toml.j2").write_text((ROOT / "formulas/integration.formula.toml").read_text())
            result = subprocess.run(["sc-compose", "render", "--file", str(root / "input.formula.toml.j2"), "--var", "run_id=controlled", "--root", str(root), "--output", str(root / "integration.formula.toml")], capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, (result.stdout, result.stderr))
            import tomllib
            formula = tomllib.loads((root / "integration.formula.toml").read_text())
            # sc-compose graph pour accepts rendered formulas, not bd runtime vars,
            # and its step grammar excludes hyphens (the public case IDs keep them).
            self.assertNotIn("vars", formula)
            for step in formula["steps"]:
                self.assertRegex(step["id"], r"^[A-Za-z0-9_]{1,64}$")
                self.assertIn("controlled", step["title"])
            steps = {step["id"].replace("_", "-"): step for step in formula["steps"]}
            self.assertEqual(len(steps), 12)
            self.assertEqual({value.replace("_", "-") for value in steps["report"]["needs"]}, set(run.formula_cases()))
            for case in run.formula_cases():
                self.assertEqual(steps[case]["needs"], ["verify_assets"])

    def test_xhtml_render_uses_sc_compose_and_rejects_invalid_xml(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "fixture.xhtml.j2"
            source.write_text('---\nname: controlled-report\nrequired_variables: [run_id, platform]\n---\n<html xmlns="http://www.w3.org/1999/xhtml"><head><title>{{ run_id }}</title></head><body>{{ platform }}</body></html>')
            output = root / "report.html"
            with patch.object(run, "ROOT", root):
                run.render_report({"run_id": "controlled", "platform": "fixture"}, root, source, output)
            self.assertIn("controlled", output.read_text())
            source.write_text("<html>not closed")
            import xml.etree.ElementTree as ET
            with self.assertRaises(ET.ParseError), patch.object(run, "ROOT", root):
                run.render_report({"run_id": "controlled", "platform": "fixture"}, root, source, output)

    def test_colima_and_windows_arguments_remain_data(self):
        values = ["--run-id", "x'; $env:SECRET; 'y", "--platform", "windows-x86_64"]
        command = remote.remote_command("colima", "/work/a b'c", "python3", values)
        self.assertEqual(command[:6], ["colima", "--profile", "default", "ssh", "--", "sh"])
        self.assertIn(shlex.join(["python3", "scripts/integration/run.py", *values]), command[-1])
        command = remote.remote_command("windows-ssh", "D:\\work space's", "py", values, "test-host")
        self.assertEqual(command[:4], ["ssh", "-o", "BatchMode=yes", "test-host"])
        decoded = base64.b64decode(command[-1]).decode("utf-16le")
        self.assertIn("'x''; $env:SECRET; ''y'", decoded)
        self.assertIn("'D:\\work space''s'", decoded)
        with self.assertRaises(ValueError):
            remote.remote_command("windows-ssh", "D:\\work", "python", [], "-oProxyCommand=bad")

    def test_real_output_judging_requires_auth_chain_and_success_schema(self):
        self.assertEqual(run.result_outcome("typesafe-invalid-key-cli", '{"error":{"id":"decision.api_status","message":"HTTP 401"}}', 1)[0], "PASS")
        self.assertEqual(run.result_outcome("anthropic-invalid-key-wheel", '{"error":{"id":"decision.backend","cause":{"id":"decision_llm.llm","cause":{"id":"llm.api_status","message":"HTTP 401"}}}}', 1)[0], "PASS")
        self.assertEqual(run.result_outcome("anthropic-invalid-key-cli", 'HTTP 401', 1)[0], "FAIL")
        self.assertEqual(run.result_outcome("anthropic-invalid-key-cli", '{"error":{"id":"wrong.type","message":"decision.backend decision_llm.llm llm.api_status HTTP 401"}}', 1)[0], "FAIL")
        self.assertEqual(run.result_outcome("typesafe-models-cli", '{"models":[]}', 0)[0], "FAIL")
        self.assertEqual(run.result_outcome("typesafe-decision-wheel", '{"error":{"id":"secrets.key_missing"}}', 1)[0], "SKIP")
        answer = {"session_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV", "response": {"answers": {"q": {"type": "noul", "noul": 0.5}}}}
        self.assertEqual(run.result_outcome("anthropic-decision-wheel", json.dumps(answer), 0)[0], "PASS")


class GateHistory(unittest.TestCase):
    def test_only_report_descendants_are_allowed(self):
        preflight.check_source_history("source", "candidate", ["site/reports/integration/run.html", "site/reports/integration/run.json"])
        for changed in ["crates/decidr/src/lib.rs", "scripts/integration/run.py", "site/reports/integration-old/run.html"]:
            with self.assertRaises(preflight.Refusal):
                preflight.check_source_history("source", "candidate", [changed])
        for bad in ["../run.html", "/tmp/run.html", "site/reports/integration/../escape.html"]:
            with self.assertRaises(preflight.Refusal):
                preflight.report_path(bad)

    def test_actual_git_history_requires_committed_pushed_xhtml(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args):
                return preflight.git(*args, root=root)
            git("init", "-q")
            git("config", "user.email", "fixture@example.invalid")
            git("config", "user.name", "Controlled fixture")
            (root / "product.rs").write_text("source")
            git("add", ".");git("commit", "-qm", "source")
            source = git("rev-parse", "HEAD")
            report = root / "site/reports/integration/run.html"
            report.parent.mkdir(parents=True)
            report.write_text('<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Fixture</title></head><body>controlled</body></html>')
            git("add", ".");git("commit", "-qm", "report")
            candidate = git("rev-parse", "HEAD")
            run_record = {"source_commit": source, "platform": "fixture", "prerelease_tag": "prerelease/v0.1.1", "artifacts": [{"name": "binary.tar.gz", "sha256": "b" * 64}], "results": [{"case_id": "future-cli", "source_commit": source, "platform": "fixture", "prerelease_tag": "prerelease/v0.1.1", "artifact_sha256": "b" * 64}], "report": "site/reports/integration/run.html", "report_sha256": preflight.digest(report)}
            # The real installed Rust parser has separate boundary tests; this seam isolates git/artifact composition.
            validator = lambda results, _: results
            verified = []
            check = lambda: preflight.validate_run(run_record, candidate, root=root, validator=validator, artifact_verifier=lambda value: verified.append(value))
            with self.assertRaises(preflight.Refusal):
                check()
            git("update-ref", "refs/remotes/origin/fixture", candidate)
            self.assertEqual(len(check()), 1)
            self.assertEqual(len(verified), 1)
            run_record["results"][0]["artifact_sha256"] = "c" * 64
            with self.assertRaises(preflight.Refusal):
                check()
            run_record["results"][0]["artifact_sha256"] = "b" * 64
            report.write_text("not XHTML")
            with self.assertRaises(preflight.Refusal):
                check()
            report.write_text('<html xmlns="http://www.w3.org/1999/xhtml"><head/><body/></html>')
            run_record["report_sha256"] = preflight.digest(report)
            with self.assertRaises(subprocess.CalledProcessError):
                check()
            git("add", ".");git("commit", "-qm", "report-only")
            candidate = git("rev-parse", "HEAD")
            git("update-ref", "refs/remotes/origin/fixture", candidate)
            self.assertEqual(len(check()), 1)
            (root / "product.rs").write_text("changed")
            git("add", ".");git("commit", "-qm", "new product")
            candidate = git("rev-parse", "HEAD")
            with self.assertRaises(preflight.Refusal):
                check()


if __name__ == "__main__":
    unittest.main()
