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
