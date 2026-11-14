"""Exercise real release asset/checksum checks with controlled downloads; no live evidence."""
import copy
import hashlib
from pathlib import Path
import sys
import tomllib
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import preflight


class ArtifactContract(unittest.TestCase):
    def test_wrong_digest_tampered_download_and_wrong_remote_source_are_refused(self):
        real = preflight.prerelease_helper()
        manifest = tomllib.loads((preflight.ROOT / "release/publish-artifacts.toml").read_text())
        version = "0.1.1"
        names = real.expected_assets(manifest, version)
        for distribution in manifest["python_distributions"]:
            name = distribution["name"].replace("-", "_")
            for wheel in distribution["wheels"]:
                names.add(f"{name}-{version}-cp311-abi3-{wheel['platform']}.whl")
            if distribution.get("sdist"):
                names.add(f"{name}-{version}.tar.gz")
        bodies = {name: ("controlled asset " + name).encode() for name in names if name != "checksums.txt"}
        hashes = {name: hashlib.sha256(body).hexdigest() for name, body in bodies.items()}
        bodies["checksums.txt"] = ("\n".join(f"{value}  {name}" for name, value in hashes.items()) + "\n").encode()
        release = {"assets": [{"name": name} for name in names]}
        selected = [real.archive_name(manifest, version, next(target for target in manifest["release_targets"] if target["target"] == "aarch64-apple-darwin")),
                    next(name for name in names if name.startswith("sc_decidr-") and "macosx_11_0_arm64" in name),
                    next(name for name in names if name.startswith("sitr-") and "macosx_11_0_arm64" in name)]
        source = "a" * 40
        run = {"platform": "macos-aarch64", "prerelease_tag": "prerelease/v0.1.1", "source_commit": source, "artifacts": [{"name": name, "sha256": hashes[name]} for name in selected]}
        def download(_tag, name, root):
            path = root / name
            path.write_bytes(bodies[name])
            return path
        helper = SimpleNamespace(release_for_tag=lambda _: release, require_release_assets=real.require_release_assets,
                                 release_asset_names=real.release_asset_names, archive_name=real.archive_name,
                                 distribution_assets=real.distribution_assets, parse_wheel=real.parse_wheel,
                                 download_asset=download, checksums=real.checksums, verify_checksum=real.verify_checksum)
        remote = source + "\trefs/tags/prerelease/v0.1.1^{}"
        with patch.object(preflight, "git", return_value=remote):
            preflight.verify_artifacts(run, helper=helper)
            wrong_platform = copy.deepcopy(run)
            wrong_platform["platform"] = "linux-aarch64"
            with self.assertRaises(preflight.Refusal):
                preflight.verify_artifacts(wrong_platform, helper=helper)
            mismatch = copy.deepcopy(run)
            mismatch["artifacts"][0]["sha256"] = "c" * 64
            with self.assertRaises(preflight.Refusal):
                preflight.verify_artifacts(mismatch, helper=helper)
            bodies[selected[0]] += b"tampered"
            with self.assertRaises(SystemExit):
                preflight.verify_artifacts(run, helper=helper)
        with patch.object(preflight, "git", return_value="b" * 40 + "\trefs/tags/prerelease/v0.1.1^{}"):
            with self.assertRaises(preflight.Refusal):
                preflight.verify_artifacts(run, helper=helper)


if __name__ == "__main__":
    unittest.main()
