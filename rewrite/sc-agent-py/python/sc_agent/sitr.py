"""Tree-sitter operations using the same native logger as decidr."""
from . import _native

def languages(*args, **kwargs):
    return _native.invoke("sitr.languages", args, kwargs)

def queries(*args, **kwargs):
    return _native.invoke("sitr.queries", args, kwargs)

def query(*args, **kwargs):
    return _native.invoke("sitr.query", args, kwargs)

def run(*args, **kwargs):
    return _native.invoke("sitr.run", args, kwargs)
