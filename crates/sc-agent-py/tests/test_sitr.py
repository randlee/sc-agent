"""ADR-SAG-0021: one wheel carries the decidr and sitr APIs, and one injected Logger takes the
records of both. Each test sets T_LOG_ROOT to its own tmp_path before opening the Logger."""
import json

import pytest

import sc_agent
from sc_agent import sitr


@pytest.fixture
def log(tmp_path, monkeypatch):
    monkeypatch.setenv("T_LOG_ROOT", str(tmp_path))
    logger, error = sc_agent.logger("t")
    assert error is None
    return logger, tmp_path


def actions(root):
    path = root / "logs" / "t.log.jsonl"
    return [json.loads(line)["action"] for line in path.read_text().splitlines() if line.strip()]


def test_query_returns_captures_and_is_logged(log):
    logger, root = log
    captures = sitr.run_query("python", "(function_definition name: (identifier) @name)", b"def hello():\n    pass\n", logger)
    assert [(c.name, c.text, c.start_line) for c in captures] == [("name", "hello", 1)]
    assert isinstance(captures[0], sitr.Capture)
    assert actions(root) == ["sitr.query"]
    assert not (root / "sessions").exists()


def test_error_keeps_its_id_and_is_logged(log):
    logger, root = log
    with pytest.raises(sitr.TreesitterError) as raised:
        sitr.run_query("cobol", "(x) @x", b"x", logger)
    assert raised.value.args[0] == "treesitter.unknown_language"
    assert actions(root) == ["sitr.query"]


def test_decidr_and_sitr_records_share_one_log(log):
    logger, root = log
    sc_agent.send("not json", "haiku", "p", "t", logger)
    sitr.run_query("rust", "(function_item name: (identifier) @name)", "fn f() {}\n", logger)
    assert actions(root) == ["decidr.send", "sitr.query"]


def test_listings_and_import_path():
    import sc_agent.sitr as by_path

    assert by_path is sitr
    assert sitr.languages() == ["python", "rust"]
    assert ("python", "branches") in sitr.queries("python")
