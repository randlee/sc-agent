"""Fake producer: prints one sc/subjects/1 document for the path in argv[1].

argv[2], when present, is a marker file the script creates first, so a test
can tell whether the producer ran at all.
"""

import json
import sys

path = sys.argv[1]
if len(sys.argv) > 2:
    open(sys.argv[2], "w").close()
text = open(path, encoding="utf-8").read().strip() or "empty"
doc = {
    "schema": "sc/subjects/1",
    "source": {"path": path, "rev": "WORKTREE", "blob_sha": "b" * 40},
    "producer": {
        "name": "fake-producer",
        "version": "0.1.0",
        "language": "python",
        "grammar": "fixture",
        "grammar_version": "0",
        "query_sha256": "a" * 64,
        "mode": "matches",
    },
    "subjects": [
        {
            "id": f"{path}:1",
            "capture": "arm",
            "text": text,
            "span": {
                "start_line": 1,
                "end_line": 1,
                "start_byte": 0,
                "end_byte": len(text.encode("utf-8")),
            },
        }
    ],
}
sys.stdout.write(json.dumps(doc))
