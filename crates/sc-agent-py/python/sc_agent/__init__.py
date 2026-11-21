"""Decision and tree-sitter calls with native errors as values."""
import json
from ._native import CancelToken, Logger, logger, send, send_batch, sitr, log_root


def category(detail):
    if isinstance(detail, str):
        detail = json.loads(detail)
    return detail.get("category") or "other"


def retryable(detail):
    return category(detail) in {"busy", "network", "response_invalid"}


class SagError(Exception):
    """Native command error; args preserve the exact detail JSON for pickle."""
    def __init__(self, id, message, detail):
        super().__init__(id, message, detail)
        self._detail = json.loads(detail)

    id = property(lambda self: self.args[0])
    message = property(lambda self: self.args[1])
    detail = property(lambda self: self.args[2])
    status = property(lambda self: self._detail.get("status"))
    attempts = property(lambda self: self._detail.get("attempts"))
    retry_after_ms = property(lambda self: self._detail.get("retry_after_ms"))
    category = property(lambda self: category(self._detail))
    retryable = property(lambda self: retryable(self._detail))

__all__ = ["CancelToken", "Logger", "logger", "send", "send_batch", "sitr", "SagError", "category", "retryable"]


def client_logger(name):
    """Obtain a caller-owned sc-observability Logger under the wheel's log root."""
    import sc_observability as obs
    root, error = log_root()
    if error is not None:
        detail = json.loads(error)["error"]
        raise SagError(detail["id"], detail["message"], json.dumps(detail))
    result = obs.create_logger(obs.LoggerConfig(service=name, log_root=root))
    if result.kind != "ok":
        raise RuntimeError(result.error.message)
    return result.value
