"""Types of the native sc_agent._native module (crates/sc-agent-py/src/lib.rs).

Every function returns a result value, never raises: {"ok": True, "value": ...} or
{"ok": False, "error": {"id", "message", "action", "category", ...}}.
The public Cancel class and sitr package module are declared by sc_agent/__init__.pyi.
"""
from collections.abc import Callable, Iterable
from typing import Any


Envelope = dict[str, Any]
Request = str | dict[str, Any]

def logger(name: str) -> Envelope: ...
def log(logger: object, event: dict[str, Any]) -> Envelope: ...
def flush(logger: object) -> Envelope: ...
def digest(request: dict[str, Any], question: str, *, general_log: object = None) -> Envelope: ...
def send(
    request: Request,
    *,
    model: str | None = None,
    provider: str | None = None,
    project: str | None = None,
    agent_role: str | None = None,
    config_dir: str | None = None,
    general_log: object = None,
    timeout_secs: float | None = None,
    cancel: object | None = None,
    base_url: str | None = None,
    sleep: Callable[[float], object] | None = None,
    keyring_lookup: Callable[[str], str] | None = None,
) -> Envelope: ...
def send_batch(
    requests: Iterable[Request],
    *,
    model: str | None = None,
    provider: str | None = None,
    project: str | None = None,
    agent_role: str | None = None,
    config_dir: str | None = None,
    general_log: object = None,
    timeout_secs: float | None = None,
    workers: int | None = None,
    on_result: Callable[[int, Envelope], object] | None = None,
    cancel: object | None = None,
    base_url: str | None = None,
    sleep: Callable[[float], object] | None = None,
    keyring_lookup: Callable[[str], str] | None = None,
) -> Envelope: ...
def _new_cancel() -> Envelope: ...
def _cancel(flag: object) -> Envelope: ...
def languages(*, general_log: object = None) -> Envelope: ...
def queries(language: str, *, general_log: object = None) -> Envelope: ...
def query(language: str, source: str, query: str, *, general_log: object = None) -> Envelope: ...
def run(language: str, name: str, source: str, *, general_log: object = None) -> Envelope: ...
