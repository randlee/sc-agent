"""Verify one sc-lint release asset against its checksums.txt entry."""

from __future__ import annotations

import hashlib
from pathlib import Path
import sys


def verify(checksums_path: Path, archive_path: Path, asset_name: str) -> None:
    entries: list[str] = []
    for line in checksums_path.read_text(encoding="utf-8").splitlines():
        fields = line.split(maxsplit=1)
        if len(fields) != 2:
            continue
        digest, listed_name = fields
        listed_name = listed_name.removeprefix("*")
        if listed_name == asset_name:
            entries.append(digest)

    if len(entries) != 1 or len(entries[0]) != 64 or any(c not in "0123456789abcdefABCDEF" for c in entries[0]):
        raise ValueError(f"checksums.txt must contain exactly one valid SHA-256 entry for {asset_name}")

    actual = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    if actual.casefold() != entries[0].casefold():
        raise ValueError(f"SHA-256 mismatch for {asset_name}")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        raise SystemExit("usage: verify_sc_lint_checksum.py CHECKSUMS ARCHIVE ASSET_NAME")
    try:
        verify(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3])
    except (OSError, ValueError) as error:
        print(f"sc-lint checksum verification failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
