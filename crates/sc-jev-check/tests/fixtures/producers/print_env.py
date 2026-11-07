"""Fake producer: prints its whole environment as one subject."""

import json
import os
import sys

text = "\n".join(f"{k}={v}" for k, v in sorted(os.environ.items())) or "EMPTY"
n = len(text.encode("utf-8"))
doc = {
    "schema": "sc/subjects/1",
    "source": {"path": "env.txt", "rev": "WORKTREE", "blob_sha": "b" * 40},
    "producer": {
        "name": "fake-producer",
        "version": "0.1.0",
        "language": "text",
        "grammar": "fixture",
        "grammar_version": "0",
        "query_sha256": "a" * 64,
        "mode": "matches",
    },
    "subjects": [
        {
            "id": "env.txt:1",
            "capture": "env",
            "text": text,
            "span": {"start_line": 1, "end_line": 1, "start_byte": 0, "end_byte": n},
        }
    ],
}
sys.stdout.write(json.dumps(doc))
