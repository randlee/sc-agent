"""Test 9: the sc_treesitter extension module (built with maturin)."""

import pytest

import sc_treesitter


def test_caller_written_query_returns_capture_objects():
    """AC: REQ-SAG-0303 PyO3."""
    captures = sc_treesitter.run_query(
        "python",
        "(function_definition name: (identifier) @name)",
        b"def hello():\n    pass\n",
    )
    assert len(captures) == 1
    capture = captures[0]
    assert isinstance(capture, sc_treesitter.Capture)
    assert capture.name == "name"
    assert capture.text == "hello"
    assert (capture.start_line, capture.end_line) == (1, 1)
    assert (capture.start_byte, capture.end_byte) == (4, 9)

    rust = sc_treesitter.run_query(
        "rust", "(function_item name: (identifier) @name)", "fn hello() {}\n"
    )
    assert [c.text for c in rust] == ["hello"]


def test_listings():
    assert sc_treesitter.languages() == ["python", "rust"]
    assert sc_treesitter.queries("python") == [("python", "branches")]
    assert "case_pattern" in sc_treesitter.query_source("python", "branches")


def test_non_utf8_bytes_raise_not_utf8():
    """AC: REQ-SAG-0002 lowest-layer PyO3."""
    with pytest.raises(sc_treesitter.TreesitterError) as raised:
        sc_treesitter.run_query("python", "(identifier) @id", b"ab = 1\n\xff\n")
    error_id, message = raised.value.args
    assert error_id == "treesitter.not_utf8"
    assert message == "source is not UTF-8 at byte 7"


def test_other_errors_keep_their_ids():
    with pytest.raises(sc_treesitter.TreesitterError) as raised:
        sc_treesitter.run_query("cobol", "(x) @x", b"x")
    assert raised.value.args[0] == "treesitter.unknown_language"
    with pytest.raises(sc_treesitter.TreesitterError) as raised:
        sc_treesitter.run_query("python", "(identifier) @id", 42)
    assert raised.value.args[0] == "treesitter.python"
