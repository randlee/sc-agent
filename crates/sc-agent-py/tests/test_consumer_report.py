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
                            capture_output=True, text=True, timeout=30)
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
