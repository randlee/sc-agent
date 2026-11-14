"""Keep scripts referenced by the consumer release configuration in the tree."""

from __future__ import annotations

import shlex
import tomllib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
SCRIPT_SUFFIXES = {".py", ".sh", ".bash"}


def command_script_paths(command: str) -> list[str]:
    """Return script file arguments from a declared shell command."""
    return [token for token in shlex.split(command) if Path(token).suffix in SCRIPT_SUFFIXES]


def test_consumer_release_scripts_exist_in_repository() -> None:
    artifacts = tomllib.loads(
        (REPO_ROOT / "release" / "publish-artifacts.toml").read_text(encoding="utf-8")
    )
    prerelease = artifacts["prerelease"]
    configured_scripts = {
        f"publish-artifacts.toml [prerelease].{field}": script
        for field in ("post_install", "verify")
        for script in command_script_paths(prerelease[field])
    }

    preflight = tomllib.loads(
        (REPO_ROOT / "release" / "publish-preflight.toml").read_text(encoding="utf-8")
    )
    configured_scripts["publish-preflight.toml [docli].generate_script"] = preflight["docli"][
        "generate_script"
    ]
    for check in preflight.get("checks", []):
        configured_scripts[f"publish-preflight.toml check {check['id']}.run"] = (
            command_script_paths(check["run"])
        )

    assert configured_scripts["publish-artifacts.toml [prerelease].post_install"]
    assert configured_scripts["publish-artifacts.toml [prerelease].verify"]
    for source, scripts in configured_scripts.items():
        if isinstance(scripts, str):
            scripts = [scripts]
        assert scripts, f"{source} declares no script path"
        for script in scripts:
            path = Path(script)
            assert not path.is_absolute() and ".." not in path.parts, f"unsafe path in {source}: {script}"
            assert (REPO_ROOT / path).is_file(), f"{source} references missing script: {script}"


if __name__ == "__main__":
    test_consumer_release_scripts_exist_in_repository()
