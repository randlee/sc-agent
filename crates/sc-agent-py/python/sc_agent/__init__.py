"""One native wheel for decisions, syntax queries and the entry point's logger."""
from ._native import logger, log, flush, send, send_batch, digest, _new_cancel, _cancel

from importlib.metadata import version as _version

__version__ = _version("sc-decidr")
__all__ = ["Cancel", "digest", "flush", "log", "logger", "send", "send_batch", "sitr"]

class Cancel:
    def __new__(cls, *args, **kwargs):
        result = _new_cancel(*args, **kwargs)
        if not result["ok"]:
            return result
        instance = super().__new__(cls)
        instance._flag = result["value"]
        return instance

    def cancel(self, *args, **kwargs) -> dict[str, object]:
        return _cancel(self._flag, *args, **kwargs)

from . import sitr
