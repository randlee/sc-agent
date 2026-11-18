"""The requested platform report is XML and escapes test-controlled text."""
import importlib.util
from pathlib import Path
import xml.etree.ElementTree as ET


def test_platform_report_preserves_results_and_escapes_text(tmp_path):
    root = Path(__file__).resolve().parents[3]
    path = root / "templates/integration-report/render.py"
    spec = importlib.util.spec_from_file_location("integration_report", path)
    renderer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(renderer)
    junit = tmp_path / "results.xml"
    junit.write_text('''<testsuites><testsuite>
      <testcase name="success &amp; &lt;tag&gt;"><properties><property name="model" value="jev" /></properties></testcase>
      <testcase name="missing"><skipped message="no key" /></testcase>
      <testcase name="bad"><failure type="AssertionError">secret-in-raw-diagnostics</failure></testcase>
    </testsuite></testsuites>''')
    output = tmp_path / "macos.xhtml"
    renderer.render(junit, output, platform="macos", source_revision="source", wheel_revision="wheel",
                    mode="loopback", run_id="controlled-report-test")
    tree = ET.parse(output)
    text = "".join(tree.getroot().itertext())
    assert "success & <tag>" in text
    assert all(status in text for status in ("PASS", "FAIL", "SKIP"))
    assert "source" in text and "wheel" in text and "loopback" in text
    assert "secret-in-raw-diagnostics" not in output.read_text()
    assert not tree.findall(".//tag")


def test_integration_command_requires_typesafe_and_deletes_temporary_dotenv(tmp_path):
    import subprocess
    import sys
    root = Path(__file__).resolve().parents[3]
    empty = tmp_path / "keys.env"
    empty.write_text("")
    result = subprocess.run([sys.executable, str(root / "scripts/integration.py"), "--env-file", str(empty)],
                            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=30)
    assert result.returncode != 0
    assert "TypeSafe key is required" in result.stderr
    assert not empty.exists()


def integration_runner():
    path = Path(__file__).resolve().parents[3] / "scripts/integration.py"
    spec = importlib.util.spec_from_file_location("integration_runner", path)
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    return runner


def test_integration_setup_timeout_is_terminal_and_preserves_cause(monkeypatch):
    import subprocess
    import pytest
    runner = integration_runner()
    calls = []

    def expired(command, **kwargs):
        calls.append((command, kwargs))
        raise subprocess.TimeoutExpired(command, kwargs["timeout"])

    monkeypatch.setattr(runner.subprocess, "run", expired)
    for command, options, limit in ((["python", "-m", "pip", "install", "pytest"], {}, 600),
                                    (["python", "-m", "maturin", "build"], {"timeout": 1800}, 1800)):
        calls.clear()
        with pytest.raises(RuntimeError, match="integration command timed out") as caught:
            runner.run(command, **options)
        assert len(calls) == 1 and calls[0][1]["timeout"] == limit
        assert calls[0][1]["check"] is True
        assert isinstance(caught.value.__cause__, subprocess.TimeoutExpired)


def test_provider_timeout_names_provider_and_cleans_credentials(tmp_path, monkeypatch):
    import subprocess
    import pytest
    runner = integration_runner()
    credential = tmp_path / "temporary.env"
    provider = "typesafe"
    credential.write_text(runner.credential_variables()[f"{provider.upper()}_API_KEY"] + "=controlled-unit-key\n")
    monkeypatch.setattr(runner.sys, "argv", ["integration.py", "--venv", str(tmp_path / "venv"),
                                           "--env-file", str(credential), "--results", str(tmp_path / "results")])
    # Setup is outside this failure-path unit; no binaries or network are invoked.
    monkeypatch.setattr(runner, "run", lambda *args, **kwargs: None)
    calls = []

    def expired(command, **kwargs):
        calls.append((command, kwargs))
        raise subprocess.TimeoutExpired(command, kwargs["timeout"])

    monkeypatch.setattr(runner.subprocess, "run", expired)
    with pytest.raises(RuntimeError, match="typesafe: integration timed out") as caught:
        runner.main()
    assert len(calls) == 1 and calls[0][1]["timeout"] == 600
    assert "pytest" in calls[0][0]
    assert isinstance(caught.value.__cause__, subprocess.TimeoutExpired)
    assert not credential.exists()
    assert not Path(calls[0][1]["cwd"]).exists()


def report_renderer():
    path = Path(__file__).resolve().parents[3] / "templates/integration-report/render.py"
    spec = importlib.util.spec_from_file_location("integration_report", path)
    renderer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(renderer)
    return renderer


def test_report_site_embeds_panels_orders_runs_and_updates_same_run(tmp_path):
    import json
    from html.parser import HTMLParser

    class Page(HTMLParser):
        def __init__(self, path):
            super().__init__()
            self.links, self.panels = [], []
            self.feed(path.read_text())

        def handle_starttag(self, tag, attrs):
            attrs = dict(attrs)
            if tag == "a":
                self.links.append(attrs["href"])
            if tag == "iframe":
                self.panels.append(attrs["srcdoc"])

    renderer = report_renderer()
    reports = tmp_path / "site/reports"
    first, second = tmp_path / "first.xhtml", tmp_path / "second.xhtml"
    first.write_text('<html><body><p>observed &amp; unchanged</p></body></html>')
    second.write_text('<html><body><p>another platform</p></body></html>')
    # Publish out of chronological order; filenames and mtimes do not control order.
    renderer.publish([first], reports, run_id="20261117-1-integration", title="New <run>",
                     created_at="2026-11-17T01:00:00+00:00")
    renderer.publish([first], reports, run_id="legacy-integration", title="Earlier integration reports", created_at="")
    renderer.publish([first], reports, run_id="20261116-1-integration", created_at="2026-11-16T01:00:00+00:00")
    page = renderer.publish([first, second], reports, run_id="20261117-1-integration",
                            created_at="2026-11-18T01:00:00+00:00")
    index = Page(reports / "index.html")
    assert index.links == ["../index.html", "20261117-1-integration.html",
                           "20261116-1-integration.html", "legacy-integration.html"]
    rendered = Page(page)
    assert rendered.panels == [first.read_text(), second.read_text()]
    assert "New &lt;run&gt;" in page.read_text()
    for href in rendered.links:
        assert (page.parent / href).is_file()
    state = json.loads((reports / "20261117-1-integration/20261117-1-integration.json").read_text())
    assert state["created_at"] == "2026-11-17T01:00:00+00:00"
    assert state["panels"] == ["first.xhtml", "second.xhtml"]


def test_junit_render_publishes_a_run_page_and_index(tmp_path):
    renderer = report_renderer()
    junit = tmp_path / "junit.xml"
    junit.write_text('<testsuite><testcase name="controlled" /></testsuite>')
    panel = tmp_path / "site/reports/integration/macos.xhtml"
    page = renderer.render(junit, panel, platform="macos", source_revision="source", wheel_revision="wheel",
                           mode="loopback", run_id="20261117-1-integration")
    assert page == panel.parent.parent / "20261117-1-integration.html"
    assert "controlled" in page.read_text()
    assert "20261117-1-integration.html" in (page.parent / "index.html").read_text()


def test_pre_rendered_consumer_panel_cli(tmp_path):
    import json
    import subprocess
    import sys
    renderer = Path(__file__).resolve().parents[3] / "templates/integration-report/render.py"
    panel = tmp_path / "consumer.xhtml"
    panel.write_text('<html><body><p>Existing consumer result</p></body></html>')
    reports = tmp_path / "reports"
    command = [sys.executable, str(renderer), "--panel", str(panel), "--reports-dir", str(reports),
               "--run-id", "20261117-1-integration", "--title", "Consumer result",
               "--created-at", "2026-11-17T01:00:00+00:00"]
    subprocess.run(command, stdin=subprocess.DEVNULL, check=True, timeout=60)
    subprocess.run(command, stdin=subprocess.DEVNULL, check=True, timeout=60)
    assert (reports / "index.html").read_text().count('href="20261117-1-integration.html"') == 1
    state = json.loads((reports / "20261117-1-integration/20261117-1-integration.json").read_text())
    assert state["panels"] == ["consumer.xhtml"]


def test_integration_runner_groups_provider_panels_in_one_run(tmp_path, monkeypatch):
    import subprocess
    import xml.etree.ElementTree as ET
    runner = integration_runner()
    credential = tmp_path / "temporary.env"
    variable = runner.credential_variables()[f"{'typesafe'.upper()}_API_KEY"]
    credential.write_text(f"{variable}=controlled-unit-key\n")
    renders = []
    monkeypatch.setattr(runner.sys, "argv", ["integration.py", "--venv", str(tmp_path / "venv"),
        "--env-file", str(credential), "--results", str(tmp_path / "results"),
        "--pin-tag", "controlled-pin", "--pin-sha", "controlled-sha"])
    monkeypatch.setattr(runner, "run", lambda command, **kwargs: renders.append(command)
                        if "render.py" in str(command[1]) else None)

    def completed(command, **kwargs):
        junit = Path(next(arg.split("=", 1)[1] for arg in command if arg.startswith("--junitxml=")))
        suite = ET.Element("testsuite")
        ET.SubElement(suite, "testcase", name="controlled transport")
        ET.ElementTree(suite).write(junit, encoding="unicode")
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(runner.subprocess, "run", completed)
    assert runner.main() == 0
    assert len(renders) == 3
    assert len({command[command.index("--run-id") + 1] for command in renders}) == 1


def test_run_names_increment_numerically_with_a_controlled_date(tmp_path, monkeypatch):
    from datetime import datetime, timezone
    from types import SimpleNamespace
    renderer = report_renderer()
    fixed = datetime(2026, 11, 17, tzinfo=timezone.utc)
    monkeypatch.setattr(renderer, "datetime", SimpleNamespace(now=lambda zone: fixed))
    for number in (2, 9, 11):
        (tmp_path / f"20261117-{number}-integration.html").touch()
    (tmp_path / "20261116-99-integration.html").touch()
    assert renderer.new_run_id(tmp_path) == "20261117-12-integration"
