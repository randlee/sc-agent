"""Type stubs for the sc_jev_py extension module (PyO3 over sc-jev-check)."""

import os
import pathlib
from typing import Any, Sequence

StrPath = str | os.PathLike[str]

class JevError(Exception):
    """Jev check failed. `code` is the stable id and `message` is the fix hint."""

    code: str
    message: str

class Session:
    """Live judge session returned by `CheckResult.session`."""

    @property
    def session_id(self) -> str: ...
    def ask(self, follow_up: str) -> dict[str, Any]: ...

class CheckResult:
    """Result of `run_check`. Immutable."""

    @property
    def session_id(self) -> str | None: ...
    @property
    def prompt_sha256(self) -> str: ...
    @property
    def payload(self) -> str: ...
    @property
    def outcomes(self) -> list[dict[str, Any]]: ...
    @property
    def equal_input(self) -> dict[str, Any] | None: ...
    @property
    def system_prompt(self) -> str | None: ...
    @property
    def session(self) -> Session | None: ...
    def ask(self, follow_up: str) -> dict[str, Any]: ...

def build_request(
    check_path: StrPath,
    *,
    repo_root: StrPath | None = None,
    rev: str = "WORKTREE",
) -> dict[str, Any]: ...
def run_check(
    check_path: StrPath,
    *,
    backends: Sequence[str] | None = None,
    dry_run: bool = False,
    repo_root: StrPath | None = None,
    rev: str = "WORKTREE",
    log_dir: StrPath | None = None,
    session_dir: StrPath | None = None,
    system_prompt_file: StrPath | None = None,
) -> CheckResult: ...
def canonical_bytes(json_text: str) -> bytes: ...
def canonical_sha256(json_text: str) -> str: ...
def default_log_root() -> pathlib.Path: ...
def resolve_log_root(log_dir: StrPath | None = None) -> pathlib.Path: ...
def resolve_session_dir(
    log_dir: StrPath, *, session_dir: StrPath | None = None
) -> pathlib.Path: ...
def log_file(log_dir: StrPath) -> pathlib.Path: ...

