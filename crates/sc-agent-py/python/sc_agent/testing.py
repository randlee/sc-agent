"""Deterministic test doubles for consumers of sc-agent runs."""
from __future__ import annotations

import contextlib
from collections.abc import Callable, Generator, Mapping, Sequence
from typing import Any
from unittest.mock import patch


class FakeRun:
    def __init__(self, script: Mapping[str, Any], **open_args: Any) -> None:
        self.script = dict(script)
        self.envelopes = list(script.get("envelopes", []))
        self.open_args = open_args
        self.sent: list[list[Any]] = []
        self.logged: list[Mapping[str, Any]] = []
        self.cancels = 0
        self.run_id: str | None = script.get("run_id")

    def sends_to(self, provider: str) -> dict[str, Any]:
        return self.script.get("sends_to_error") or {"ok": True, "value": provider == self.script.get("provider")}

    def log(self, event: Mapping[str, Any]) -> dict[str, Any]:
        self.logged.append(event)
        return {"ok": True, "value": None}

    def __enter__(self) -> FakeRun:
        return self

    def __exit__(self, *exc: object) -> bool:
        return False

    def send(self, request: Any, *, cache: bool | None = None) -> dict[str, Any]:
        return self.send_batch([request], cache=cache)["value"][0]["result"]

    def send_batch(self, requests: Sequence[Any], *, cache: bool | None = None,
                   on_result: Callable[[int, dict[str, Any]], object] | None = None) -> dict[str, Any]:
        self.sent.append([list(requests), cache])
        values: list[dict[str, Any]] = []
        for index, _ in enumerate(requests):
            if index == self.script.get("interrupt_at"):
                self.cancel()
            if self.cancels:
                envelope = {"ok": False, "error": {"id": "decision.cancelled", "category": "cancelled", "message": "cancelled"}}
            else:
                envelope = self.envelopes.pop(0)
            values.append({"index": index, "result": envelope})
            if on_result:
                on_result(index, envelope)
        return {"ok": True, "value": values}

    def cancel(self) -> dict[str, Any]:
        self.cancels += 1
        return {"ok": True, "value": None}


@contextlib.contextmanager
def fake_open_run(module: Any, script: Mapping[str, Any]) -> Generator[list[FakeRun]]:
    import sc_agent
    runs: list[FakeRun] = []

    def open_fake(**kwargs: Any) -> Any:
        run = FakeRun(script, **kwargs)
        runs.append(run)
        return module.Ok(run)

    def rows(ids: Sequence[str], **kwargs: Any) -> dict[str, Any]:
        del kwargs
        values = script.get("rows", {})
        missing = [key for key in ids if key not in values]
        if missing:
            return {"ok": False, "error": {"id": "session.not_found", "category": "validation", "message": f"missing session ids: {missing}"}}
        return {"ok": True, "value": {key: values[key] for key in ids}}

    with patch.object(module, "open_run", open_fake), patch.object(sc_agent, "session_rows", rows):
        yield runs
