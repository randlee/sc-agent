"""Typed integration failures with safe infrastructure diagnostics."""
import json
import os
import re
import shlex
import subprocess
import xml.etree.ElementTree as ET


class Refusal(Exception):
    """A refused operation with a stable machine id and an actionable recovery."""

    def __init__(self, message, *, code="integration.refused", remediation="Correct the integration inputs and retry."):
        super().__init__(message)
        self.code = code
        self.remediation = remediation


def redact(value):
    text = value.decode("utf-8", "replace") if isinstance(value, bytes) else str(value or "")
    secrets = {value for key, value in os.environ.items()
               if value and re.search(r"KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL", key, re.I)}
    for secret in sorted(secrets, key=len, reverse=True):
        text = text.replace(secret, "[redacted]")
    text = re.sub(r"(https?://)[^/\s@]+@", r"\1[redacted]@", text)
    text = re.sub(r"(?i)((?:bearer|basic)\s+)[^\s\"']+", r"\1[redacted]", text)
    return re.sub(r"(?i)((?:access_token|api[_-]?key|token|password)=)[^&\s]+", r"\1[redacted]", text)


def process_detail(error):
    args = list(map(str, error.cmd)) if isinstance(error.cmd, (list, tuple)) else [str(error.cmd)]
    validator = "-c" in args and any("validate_integration_results" in arg for arg in args)
    if "-c" in args and args.index("-c") + 1 < len(args):
        args[args.index("-c") + 1] = "<integration-validator>" if validator else "<python-code>"
    for position, arg in enumerate(tuple(args)):
        if re.fullmatch(r"--?(?:api[-_]?key|token|password|secret)", arg, re.I) and position + 1 < len(args):
            args[position + 1] = "[redacted]"
    args = [redact(arg) for arg in args]
    stderr = redact(error.stderr)
    if validator and stderr:
        # ValueErrors can quote provider evidence from the validator's stdin.
        safe = [line for line in stderr.splitlines()
                if line.startswith(("ModuleNotFoundError: No module named", "ImportError:", "OSError:"))]
        stderr = "\n".join(safe) if safe else "[withheld: validator stderr may contain provider evidence]"
    tail = stderr[-4096:]
    if isinstance(error, subprocess.TimeoutExpired):
        code, message = "integration.subprocess_timeout", f"command {shlex.join(args)} exceeded {error.timeout} seconds"
        remediation = "Check the named command's connectivity and execution limits, then retry the operation."
    else:
        code, message = "integration.subprocess_failed", f"command {shlex.join(args)} failed (exit {error.returncode})"
        remediation = "Correct the cause in stderr_tail and rerun the named command before retrying integration."
    detail = {"id": code, "message": message, "remediation": remediation, "argv": args, "stderr_tail": tail}
    if isinstance(error, subprocess.CalledProcessError):
        detail["returncode"] = error.returncode
    return detail


def failure_detail(error):
    if isinstance(error, Refusal):
        return {"id": error.code, "message": redact(error), "remediation": error.remediation}
    if isinstance(error, (subprocess.CalledProcessError, subprocess.TimeoutExpired)):
        return process_detail(error)
    if isinstance(error, json.JSONDecodeError):
        code, remediation = "integration.invalid_json", "Correct the JSON syntax at the reported line and column, then retry."
    elif isinstance(error, ET.ParseError):
        code, remediation = "integration.invalid_xhtml", "Render a well-formed XHTML report with the selected template and retry."
    elif isinstance(error, OSError):
        code, remediation = "integration.io_failed", "Check the named file or executable and its permissions, then retry."
    elif isinstance(error, (KeyError, ValueError, TypeError)):
        code, remediation = "integration.invalid_record", "Repair the run, release or case configuration named by the error and retry."
    elif isinstance(error, SystemExit):
        code, remediation = "integration.helper_refused", "Correct the prerelease helper's reported prerequisite and retry."
    else:
        code, remediation = "integration.internal", "Report this failure with the command and run id; preserve the run directory for recovery."
    return {"id": code, "message": redact(error), "remediation": remediation,
            "cause": {"id": f"python.{type(error).__module__}.{type(error).__qualname__}", "message": redact(error)}}
