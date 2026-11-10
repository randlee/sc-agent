"""A loopback decision runner for the sc-agent tests (no external network).

It speaks the Mapika/decider shape: ``GET /health`` says the model is loaded, ``GET /v1/models``
lists it, and ``POST /v1/systemone`` either never answers (``hang``) or answers one fixed
error (``error``). Every systemone request is recorded with its headers and body.
"""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODEL = "jev-latest"
FIXED_ERROR = {"detail": "the loopback runner always refuses"}


class Loopback:
    """A runner on 127.0.0.1 in a background thread."""

    def __init__(self, mode: str) -> None:
        if mode not in ("hang", "error"):
            raise ValueError(mode)
        self.mode = mode
        self.requests: list[dict] = []
        self.started = threading.Event()
        self.release = threading.Event()
        self._lock = threading.Lock()
        loopback = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *args) -> None:  # keep pytest output clean
                pass

            def _reply(self, status: int, body: dict) -> None:
                data = json.dumps(body).encode()
                self.send_response(status)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def do_GET(self) -> None:
                if self.path == "/health":
                    self._reply(200, {"ok": True, "model": MODEL})
                elif self.path == "/v1/models":
                    self._reply(200, {"models": [{"name": MODEL, "description": "loopback", "release_date": "2026-09-15"}]})
                else:
                    self._reply(404, {"detail": "not found"})

            def do_POST(self) -> None:
                length = int(self.headers.get("content-length", "0"))
                body = self.rfile.read(length)
                with loopback._lock:
                    loopback.requests.append(
                        {"path": self.path, "headers": {k.lower(): v for k, v in self.headers.items()}, "body": body}
                    )
                loopback.started.set()
                if loopback.mode == "hang":
                    loopback.release.wait(60.0)
                    self.close_connection = True
                    return
                self._reply(503, FIXED_ERROR)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.port = self.server.server_address[1]
        self._thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self._thread.start()

    def close(self) -> None:
        self.release.set()
        self.server.shutdown()
        self.server.server_close()
