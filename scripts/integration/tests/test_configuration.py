"""Controlled configuration probes; these fixtures are not live provider evidence."""
import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import configuration
import preflight
import run


class Configuration(unittest.TestCase):
    def manifest(self):
        return {
            "release_targets": [{"target": "riscv64-unknown-linux-gnu", "archive": "tar.gz"}],
            "python_packages": [{"module": "sc_agent", "package": "renamed-agent"}],
            "python_distributions": [{"name": "renamed-agent", "wheels": [{"id": "penguin-rv64", "target": "riscv64-unknown-linux-gnu", "platform": "linux_riscv64"}]}],
        }

    def test_new_target_and_aliases_come_from_manifest(self):
        manifest = self.manifest()
        for alias in ("penguin-rv64", "penguin-riscv64", "riscv64-unknown-linux-gnu"):
            self.assertEqual(configuration.platform_target(manifest, alias), manifest["release_targets"][0])
        with self.assertRaises(configuration.Refusal):
            configuration.platform_target(manifest, "macos-aarch64")
        ambiguous = copy.deepcopy(manifest)
        ambiguous["release_targets"].append({"target": "other-unknown-linux-gnu"})
        ambiguous["python_distributions"][0]["wheels"].append({"id": "penguin-rv64", "target": "other-unknown-linux-gnu", "platform": "linux_other"})
        with self.assertRaises(configuration.Refusal):
            configuration.platform_target(ambiguous, "penguin-rv64")

    def test_renamed_distribution_selects_its_wheel_without_a_filename_prefix(self):
        helper = preflight.prerelease_helper()
        distribution = configuration.module_distribution(self.manifest())
        name = "renamed_agent-9.2.0-py3-none-linux_riscv64.whl"
        self.assertEqual(configuration.distribution_wheel(helper, {name, "other-9.2.0-py3-none-linux_riscv64.whl"}, distribution, "9.2.0"), name)
        for names in ({name}, {name, "renamed_agent-9.2.0-cp311-none-linux_riscv64.whl"}):
            with self.assertRaises(configuration.Refusal):
                configuration.distribution_wheel(helper, names, distribution, "0.0.0" if len(names) == 1 else "9.2.0")

    def test_provider_and_model_formula_changes_drive_the_installed_command(self):
        source = (run.ROOT / "formulas/integration.formula.toml").read_text()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "formulas").mkdir()
            formula = root / "formulas/integration.formula.toml"
            formula.write_text(source.replace('provider = "typesafe"', 'provider = "configured-provider"').replace('model = "jev"', 'model = "configured-model"'))
            run_dir = root / "run"
            run_dir.mkdir()
            state = {"source_commit": "a" * 40, "platform": "fixture", "prerelease_tag": "prerelease/v9.2.0", "results": [], "beads": {"typesafe_decision_cli": "fixture-bead"}, "artifacts": [{"name": "archive.tar.gz", "sha256": "b" * 64}]}
            answer = {"response": {"answers": {"q": {"type": "noul", "noul": 0.5}}}}
            with patch.object(run, "ROOT", root), patch.object(run.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout=json.dumps(answer), stderr="")) as process, contextlib.redirect_stdout(io.StringIO()):
                run.execute_case(state, run_dir, "typesafe-decision-cli")
            command = process.call_args_list[0].args[0]
            self.assertEqual(command[command.index("--provider") + 1], "configured-provider")
            self.assertEqual(command[command.index("--model") + 1], "configured-model")
            self.assertEqual(json.loads((run_dir / "request.json").read_text())["model"], "configured-model")
            formula.write_text(source.replace('model = "jev"', 'model = ""'))
            with self.assertRaises(configuration.Refusal):
                configuration.case_specs(root)


if __name__ == "__main__":
    unittest.main()
