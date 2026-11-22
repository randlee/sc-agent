"""One native wheel for decisions, syntax queries and the entry point's logger."""
from ._native import logger, log, flush, send, send_batch, digest, _new_cancel, _cancel

__version__ = "0.3.0"

class Cancel:
    def __init__(self):
        self._flag = _new_cancel()["value"]

    def cancel(self):
        return _cancel(self._flag)

from . import sitr
