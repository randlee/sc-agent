from ._native import (
    Envelope as Envelope,
    Request as Request,
    digest as digest,
    flush as flush,
    logger as logger,
    log as log,
    send as send,
    send_batch as send_batch,
)
from . import sitr as sitr

class Cancel:
    def __init__(self) -> None: ...
    def cancel(self) -> Envelope: ...

__version__: str
__all__ = ["Cancel", "digest", "flush", "log", "logger", "send", "send_batch", "sitr"]
