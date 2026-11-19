"""Wheel logging through real caller-owned loggers and temporary roots."""
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest
import sc_observability as obs
import sc_treesitter

QUERY = "(identifier) @id"


def test_injected_logger_and_attached_logger(tmp_path):
    root = tmp_path / "owned"
    logger = obs.create_logger(obs.LoggerConfig(service="consumer", log_root=str(root))).value
    attached = obs.AttachedLogger(logger._native)
    for target in (logger, attached):
        sc_treesitter.run_query("python", QUERY, b"x = 1", general_log=target, log_root=tmp_path)
    assert logger.flush().kind == "ok"
    rows = [json.loads(line) for line in (root / "logs/consumer.log.jsonl").read_text().splitlines()]
    assert [row["action"] for row in rows] == ["sitr.run_query", "sitr.run_query"]
    assert all(row["fields"]["language"] == "python" for row in rows)
    assert all(row["fields"]["query_size"] == len(QUERY) for row in rows)
    assert not (tmp_path / "logs/sitr.log.jsonl").exists()
    assert logger.log(obs.LogEvent(level="info", target="consumer", action="still.open")).kind == "ok"
    assert logger.shutdown().kind == "ok"


def test_callable_and_failure_are_recorded(tmp_path):
    records = []
    sc_treesitter.run_query("python", QUERY, "x", general_log=records.append, log_root=tmp_path)
    with pytest.raises(sc_treesitter.TreesitterError) as raised:
        sc_treesitter.run_query("unknown", QUERY, "x", general_log=records.append, log_root=tmp_path)
    assert len(records) == 2
    assert records[0]["fields"]["outcome"] == "ok"
    assert records[1]["fields"]["outcome"] == "error"
    assert records[1]["fields"]["error_id"] == raised.value.args[0] == "treesitter.unknown_language"
    assert not (tmp_path / "logs/sitr.log.jsonl").exists()


def test_default_logger_records_every_entry_point(tmp_path):
    sc_treesitter.languages(log_root=tmp_path)
    sc_treesitter.queries("python", log_root=tmp_path)
    source = sc_treesitter.query_source("python", "branches", log_root=tmp_path)
    sc_treesitter.run_query("python", QUERY, "x", log_root=tmp_path)
    rows = [json.loads(line) for line in (tmp_path / "logs/sitr.log.jsonl").read_text().splitlines()]
    assert [row["action"] for row in rows] == ["sitr.languages", "sitr.queries", "sitr.query_source", "sitr.run_query"]
    assert all(row["fields"]["outcome"] == "ok" for row in rows)
    assert rows[2]["fields"]["query_size"] == len(source.encode("utf-8"))


def test_default_home_in_child_process(tmp_path):
    env = os.environ.copy()
    env["HOME"] = str(tmp_path)
    env["USERPROFILE"] = str(tmp_path)
    script = 'import sc_treesitter; sc_treesitter.run_query("python", "(identifier) @id", "x")'
    result = subprocess.run([sys.executable, "-c", script], env=env, cwd=tmp_path,
                            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=30)
    assert result.returncode == 0, (result.stdout, result.stderr)
    log = tmp_path / ".sc-agent/logs/sitr.log.jsonl"
    assert len(log.read_text().splitlines()) == 1
    assert sorted(path.relative_to(tmp_path) for path in tmp_path.rglob("*") if path.is_file()) == [Path(".sc-agent/logs/sitr.log.jsonl")]


@pytest.mark.parametrize("entrypoint,args", [
    (sc_treesitter.languages, ()),
    (sc_treesitter.queries, ("python",)),
    (sc_treesitter.query_source, ("python", "branches")),
    (sc_treesitter.run_query, ("python", QUERY, "x")),
])
@pytest.mark.parametrize("failure", ["invalid_target", "blocked_root", "raising_target"])
def test_logging_failures_preserve_results_and_report_ids(tmp_path, capfd, entrypoint, args, failure):
    records = []
    expected = entrypoint(*args, general_log=records.append)
    kwargs = {"log_root": tmp_path}
    if failure == "invalid_target":
        kwargs["general_log"] = 42
        error_id = "log.invalid_target"
    elif failure == "blocked_root":
        blocked = tmp_path / "file"
        blocked.write_text("unchanged")
        kwargs["log_root"] = blocked
        error_id = "log.open_sink"
    else:
        def raising(record):
            raise ValueError("controlled logging refusal")
        kwargs["general_log"] = raising
        error_id = "log.python_rejected"
    actual = entrypoint(*args, **kwargs)
    if entrypoint is sc_treesitter.run_query:
        assert [c.text for c in actual] == [c.text for c in expected]
    else:
        assert actual == expected
    diagnostic = capfd.readouterr()
    assert diagnostic.out == ""
    assert f"sitr logging failed: {error_id}:" in diagnostic.err
    if failure == "raising_target":
        assert "controlled logging refusal" in diagnostic.err
    assert not (tmp_path / "logs").exists()
    if failure == "blocked_root":
        assert blocked.read_text() == "unchanged"


def test_logger_open_failure_preserves_query_error(tmp_path, capfd):
    with pytest.raises(sc_treesitter.TreesitterError) as raised:
        sc_treesitter.run_query("unknown", QUERY, "x", general_log=42, log_root=tmp_path)
    assert raised.value.args[0] == "treesitter.unknown_language"
    assert "log.invalid_target" in capfd.readouterr().err
