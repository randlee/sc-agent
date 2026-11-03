"""Rust canonical bytes match the a-1 Python reference fixture."""

from __future__ import annotations

from pathlib import Path

import sc_jev_py

CROSS = Path(__file__).resolve().parents[2] / "jev" / "cross_impl"


def test_canonical_bytes_match_a1_reference() -> None:
    text = (CROSS / "logical.json").read_text(encoding="utf-8")
    got = sc_jev_py.canonical_bytes(text)
    digest = sc_jev_py.canonical_sha256(text)
    assert got == (CROSS / "reference.bytes").read_bytes()
    assert digest == (CROSS / "reference.sha256").read_text(encoding="utf-8").strip()
    assert digest == sc_jev_py.canonical_sha256(got.decode("utf-8"))
