"""Payload built from the committed branches subjects matches across calls."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

import sc_jev_py

FIXTURES = Path(__file__).resolve().parents[2]
BRANCHES = FIXTURES / "cross-impl" / "branches"


def test_branches_fixture_digest_is_stable(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    repo.mkdir()
    questions = {
        "schema": "plan-anchoring/questions/1",
        "version": 1,
        "questions": {
            "relevant": {
                "type": "choice",
                "instructions": "Review the branches:\n\n{branches}",
                "criteria": {"no": "No.", "yes": "Yes."},
            }
        },
    }
    (repo / "questions.json").write_text(json.dumps(questions), encoding="utf-8")
    (repo / "check.toml").write_text(
        "\n".join(
            [
                "[questions]",
                'path = "questions.json"',
                "",
                "[inputs.branches]",
                'kind = "subjects"',
                'path = "subjects.json"',
                "",
            ]
        ),
        encoding="utf-8",
    )
    shutil.copy(BRANCHES / "subjects.json", repo / "subjects.json")
    document = json.loads((BRANCHES / "subjects.json").read_text(encoding="utf-8"))

    built = sc_jev_py.build_request("check.toml", repo_root=str(repo))
    dry = sc_jev_py.run_check(
        "check.toml",
        backends=["jev"],
        dry_run=True,
        repo_root=str(repo),
        log_dir=str(tmp_path / "log"),
        session_dir=str(tmp_path / "sessions"),
    )
    assert built["prompt_sha256"] == dry.prompt_sha256
    assert built["payload"] == dry.payload
    assert sc_jev_py.canonical_sha256(built["payload"]) == built["prompt_sha256"]

    payload = json.loads(built["payload"])
    prompt = payload["questions"][0]["prompt"]
    cursor = 0
    for subject in document["subjects"]:
        found = prompt.find(subject["text"], cursor)
        assert found >= cursor, subject["text"]
        cursor = found + len(subject["text"])

    producer = built["producer"]
    assert producer["name"] == "sc-treesitter"
    assert producer["language"] == "python"
    assert producer["mode"] == "matches"
    assert producer["query_sha256"] == document["producer"]["query_sha256"]
    again = sc_jev_py.build_request("check.toml", repo_root=str(repo))
    assert again["prompt_sha256"] == built["prompt_sha256"]
