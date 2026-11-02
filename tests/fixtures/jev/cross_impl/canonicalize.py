#!/usr/bin/env python3
"""Python reference for ADR-SAG-0002 canonical judge payload bytes.

Sorted object keys, compact separators, and UTF-8. Non-ASCII characters
stay as UTF-8 (`ensure_ascii=False`), matching `serde_json`'s default
formatter.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


def canonical_bytes(document: object) -> bytes:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    return encoded.encode("utf-8")


def main() -> None:
    folder = Path(__file__).resolve().parent
    document = json.loads((folder / "logical.json").read_text(encoding="utf-8"))
    encoded = canonical_bytes(document)
    digest = hashlib.sha256(encoded).hexdigest()
    if "--write" in sys.argv[1:]:
        (folder / "reference.bytes").write_bytes(encoded)
        (folder / "reference.sha256").write_text(digest + "\n", encoding="utf-8")
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
