"""One native wheel for decisions, syntax queries and the entry point's logger."""
from ._native import logger, log, flush, send, send_batch, digest, _new_cancel, _cancel

__version__ = "0.3.0"
__all__ = ["Cancel", "digest", "flush", "log", "logger", "send", "send_batch", "sitr"]

class Cancel:
    def __init__(self) -> None:
        self._flag: object = _new_cancel()["value"]

    def cancel(self) -> dict[str, object]:
        return _cancel(self._flag)

from . import sitr
