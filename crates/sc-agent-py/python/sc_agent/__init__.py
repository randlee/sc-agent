"""One native wheel for decisions, syntax queries and the entry point's logger."""
from ._native import logger, log, flush, send, send_batch, digest, _new_cancel, _cancel

from importlib.metadata import version as _version
from typing import Self, overload

__version__ = _version("sc-decidr")
__all__ = ["Cancel", "digest", "flush", "log", "logger", "send", "send_batch", "sitr"]

class Cancel:
    _flag: object

    @overload
    def __new__(cls) -> Self: ...
    @overload
    def __new__(cls, *args: object, **kwargs: object) -> Self | dict[str, object]: ...
    def __new__(cls, *args: object, **kwargs: object) -> Self | dict[str, object]:
        result = _new_cancel(*args, **kwargs)
        if not result["ok"]:
            return result
        instance = super().__new__(cls)
        instance._flag = result["value"]
        return instance

    def cancel(self, *args: object, **kwargs: object) -> dict[str, object]:
        return _cancel(self._flag, *args, **kwargs)

from . import sitr
