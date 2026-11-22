"""One native wheel for decisions, syntax queries and the entry point's logger."""
from ._native import logger, log, flush, send, send_batch, digest, _new_cancel, _cancel

from importlib.metadata import version as _version

__version__ = _version("sc-decidr")
__all__ = ["Cancel", "digest", "flush", "log", "logger", "send", "send_batch", "sitr"]

class Cancel:
    def __init__(self) -> None:
        self._flag: object = _new_cancel()["value"]

    def cancel(self) -> dict[str, object]:
        return _cancel(self._flag)

from . import sitr
