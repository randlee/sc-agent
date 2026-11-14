"""Checksum verification for the pinned sc-lint release download."""

import hashlib
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_sc_lint_checksum import verify


class SetupScLintChecksum(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.asset = self.root / "sc-lint_0.8.9_x86_64-unknown-linux-gnu.tar.gz"
        self.asset.write_bytes(b"controlled release asset")
        self.checksums = self.root / "checksums.txt"
        self.digest = hashlib.sha256(self.asset.read_bytes()).hexdigest()

    def tearDown(self):
        self.temp.cleanup()

    def test_matching_asset_is_verified(self):
        self.checksums.write_text(f"{self.digest}  {self.asset.name}\n", encoding="utf-8")
        verify(self.checksums, self.asset, self.asset.name)

    def test_mismatched_archive_is_rejected(self):
        self.checksums.write_text(f"{'0' * 64}  {self.asset.name}\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            verify(self.checksums, self.asset, self.asset.name)

    def test_missing_or_duplicate_asset_entry_is_rejected(self):
        self.checksums.write_text(f"{self.digest}  other-asset.tar.gz\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "exactly one"):
            verify(self.checksums, self.asset, self.asset.name)
        self.checksums.write_text(
            f"{self.digest}  {self.asset.name}\n{self.digest}  {self.asset.name}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "exactly one"):
            verify(self.checksums, self.asset, self.asset.name)


if __name__ == "__main__":
    unittest.main()
