"""One native wheel for decisions, syntax queries and the entry point's logger."""
from . import _native

__version__ = "0.3.0"

def logger(*args, **kwargs):
    return _native.invoke("logger", args, kwargs)

def log(*args, **kwargs):
    return _native.invoke("log", args, kwargs)

def flush(*args, **kwargs):
    return _native.invoke("flush", args, kwargs)

def send(*args, **kwargs):
    return _native.invoke("send", args, kwargs)

def send_batch(*args, **kwargs):
    return _native.invoke("send_batch", args, kwargs)

def digest(*args, **kwargs):
    return _native.invoke("digest", args, kwargs)

def category(*args, **kwargs):
    return _native.invoke("category", args, kwargs)

def retryable(*args, **kwargs):
    return _native.invoke("retryable", args, kwargs)

class Cancel:
    def __init__(self):
        self._flag = _native.invoke("cancel_new", (), {})["value"]

    def cancel(self):
        return _native.invoke("cancel", (self._flag,), {})

from . import sitr
