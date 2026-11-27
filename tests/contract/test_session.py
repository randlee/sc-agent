"""Session and cache contracts through the installed native wheel."""
import importlib
import json
import os
from pathlib import Path
import subprocess
import shutil
from unittest.mock import patch

from test_contract import ANSWER, REQUEST, Isolated, server

SC_COMPOSE = shutil.which("sc-compose")


class Session(Isolated):
    @classmethod
    def setUpClass(cls):
        cls.api = importlib.import_module("sc_agent")

    def test_run_id_and_links_and_reader(self):
        with server([(200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            with self.api.Run(tool="decidr", tool_version="1.2.0", project="contract", agent_role="decidr", config_dir=self.directory.name, base_url=url) as run:
                self.assertEqual(len(run.run_id), 26)
                first = run.send(REQUEST)
                second = run.send(REQUEST)
                self.assertTrue(first["ok"], first)
                self.assertTrue(second["ok"], second)
                self.assertFalse(first["value"]["cached"])
                self.assertTrue(second["value"]["cached"])
                self.assertEqual(first["value"]["session_id"], second["value"]["session_id"])
                run_id = run.run_id
                self.assertTrue(run.log({"target": "consumer", "action": "client.event", "correlation_id": "c1", "fields": {"key": "value"}})["ok"])
                self.assertEqual(run.log("invalid")["error"]["id"], "sc_agent_py.invalid_option")
                self.assertEqual(run.log()["error"]["id"], "sc_agent_py.invalid_option")
                self.assertEqual(run.log({}, 1)["error"]["id"], "sc_agent_py.invalid_option")
            self.assertEqual(len(received), 1)
            session = first["value"]["session_id"]
            found = self.api.session_rows([session], agent_role="decidr", project="contract")
            self.assertTrue(found["ok"], found)
            self.assertEqual(found["value"][session]["tool_version"], "1.2.0")
            missing = self.api.session_rows(["missing"], agent_role="decidr", project="contract")
            self.assertEqual(missing["error"]["id"], "session.not_found")
            links = list(Path(self.directory.name).rglob("runs.jsonl"))
            self.assertEqual(len(links), 1)
            rows = [json.loads(line)["fields"] for line in links[0].read_text().splitlines()]
            self.assertEqual([row["run_id"] for row in rows], [run_id, run_id])
            self.assertEqual([row["cached"] for row in rows], [False, True])

    def test_missing_tool_and_version_refuse(self):
        for opts in ({"tool_version": "1"}, {"tool": "decidr"}, {"tool": "", "tool_version": "1"}, {"tool": "decidr", "tool_version": ""}):
            run = self.api.Run(agent_role="decidr", **opts)
            self.assertIsNone(run.run_id)
            self.assertEqual(run.send(REQUEST)["error"]["id"], "decision.missing_tool")

    def test_send_cache_override_and_distinct_runs(self):
        with server([(200, {}, ANSWER), (200, {}, ANSWER)]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            with self.api.Run(tool="decidr", tool_version="1.2.0", cache=False, project="contract", agent_role="decidr", config_dir=self.directory.name, base_url=url) as first_run:
                first = first_run.send(REQUEST)
                second = first_run.send(REQUEST, cache=False)
                hit = first_run.send(REQUEST, cache=True)
                self.assertTrue(first["ok"] and second["ok"] and hit["ok"], (first, second, hit))
                self.assertFalse(first["value"]["cached"])
                self.assertFalse(second["value"]["cached"])
                self.assertTrue(hit["value"]["cached"])
                previous_run_id = first_run.run_id
            with self.api.Run(tool="decidr", tool_version="1.2.0", project="contract", agent_role="decidr", config_dir=self.directory.name, base_url=url) as next_run:
                self.assertNotEqual(previous_run_id, next_run.run_id)
            self.assertEqual(len(received), 2)

    def test_sitr_tool_names_log(self):
        with patch.dict(os.environ, {"CUSTOM_SITR_LOG_ROOT": self.directory.name}):
            result = self.api.sitr.languages(tool="custom-sitr")
        self.assertTrue(result["ok"], result)
        self.assertTrue((Path(self.directory.name) / "logs/custom-sitr.log.jsonl").exists())

    def test_oversized_provider_error_is_not_replaced_by_log_error(self):
        message = "x" * (2 * 1024 * 1024)
        with server([(401, {}, {"error": message})]) as (url, received), patch.dict(os.environ, {"TYPESAFE_API_KEY": "contract-secret"}):
            with self.api.Run(tool="decidr", tool_version="1.2.0", cache=False, project="contract", agent_role="decidr", config_dir=self.directory.name, base_url=url) as run:
                result = run.send(REQUEST)
            self.assertEqual(len(received), 1)
            self.assertEqual(result["error"]["id"], "http.auth")
            self.assertEqual(json.loads(result["error"]["message"])["error"], message)
            self.assertFalse((Path(self.directory.name) / "sessions").exists())

    def test_cli_and_wheel_rows_match(self):
        root = Path(self.directory.name)
        binary = Path(__file__).resolve().parents[2] / "target/debug/decidr"
        with server([(200, {}, ANSWER), (200, {}, ANSWER)]) as (url, received):
            port = int(url.split(":")[2].split("/")[0])
            (root / "decidr.json").write_text(json.dumps({"project": "contract", "laya": {"host": "127.0.0.1", "port": port}}))
            command = [str(binary), "--config-dir", str(root), "send", "--agent-role", "decidr", "--model", "laya"]
            cli = subprocess.run(command, input=json.dumps(REQUEST), text=True, capture_output=True, timeout=20, cwd=root, env=dict(os.environ))
            self.assertEqual(cli.returncode, 0, cli.stderr)
            self.assertTrue(json.loads(cli.stdout)["ok"], cli.stdout)
            with self.api.Run(tool="decidr", tool_version="wheel-test", cache=False, model="laya", project="contract", agent_role="decidr", config_dir=str(root), base_url=url) as run:
                value = run.send(REQUEST)
                self.assertTrue(value["ok"], value)
            self.assertEqual(len(received), 2)
        files = list(root.rglob("sessions.jsonl"))
        self.assertEqual(len(files), 1)
        rows = [json.loads(line)["fields"] for line in files[0].read_text().splitlines()]
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["tool_version"], self.api.__version__)
        self.assertEqual(rows[1]["tool_version"], "wheel-test")
        for row in rows:
            row.pop("session_id")
            row.pop("tool_version")
            row["result"]["value"].pop("session_id")
        self.assertEqual(rows[0], rows[1])

    def test_rendered_template_requires_and_passes_tool_version(self):
        self.assertIsNotNone(SC_COMPOSE, "sc-compose is required for template contracts")
        template = Path(self.api.__file__).parent / "templates"
        output = Path(self.directory.name) / "client.py"
        variables = Path(self.directory.name) / "template-vars.json"
        values = {"client_name": "contract-client", "sitr": False, "sc_decidr_version": "0.4.1"}
        variables.write_text(json.dumps(values))
        command = [SC_COMPOSE, "render", "--root", str(template), "--file", "consumer.py.j2", "--strict", "--var-file", str(variables), "--output", str(output)]
        missing = subprocess.run(command, text=True, capture_output=True, timeout=20, cwd=self.directory.name)
        self.assertNotEqual(missing.returncode, 0, missing.stdout)
        variables.write_text(json.dumps({**values, "tool_version": "1.2.3"}))
        rendered = subprocess.run(command, text=True, capture_output=True, timeout=20, cwd=self.directory.name)
        self.assertEqual(rendered.returncode, 0, rendered.stderr)
        source = output.read_text()
        self.assertIn('TOOL_VERSION = "1.2.3"', source)
        self.assertEqual(source.count("tool_version=TOOL_VERSION"), 2)
