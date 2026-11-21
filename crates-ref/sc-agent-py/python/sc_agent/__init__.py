"""Public sc-agent API backed by the native decision harness."""
from . import _native
from ._native import *

__doc__ = _native.__doc__
