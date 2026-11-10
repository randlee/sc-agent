#!/usr/bin/env python3
"""Run a command while holding an exclusive lock on anchors.json, so parallel agents never overwrite each other.
Usage: python3 locked.py <command> [args...]"""
import fcntl, subprocess, sys, pathlib
lock = pathlib.Path(__file__).resolve().parent.parent / "anchors.json.lock"
with open(lock, "w") as f:
    fcntl.flock(f, fcntl.LOCK_EX)
    sys.exit(subprocess.call(sys.argv[1:]))
