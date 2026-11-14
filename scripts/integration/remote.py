#!/usr/bin/env python3
"""Execute the same integration runner in Colima Linux or a configured Windows SSH checkout."""
import argparse
import json
import shlex
import subprocess


def remote_command(transport, checkout, python, arguments, host=None, profile="default"):
    if transport == "colima":
        script = "cd -- " + shlex.quote(checkout) + " && exec " + shlex.join([python, "scripts/integration/run.py", *arguments])
        return ["colima", "--profile", profile, "ssh", "--", "sh", "-lc", script]
    if not host or host.startswith("-"):
        raise ValueError("a non-option SSH host is required")
    def ps(value):
        return "'" + value.replace("'", "''") + "'"
    script = "& { Set-Location -LiteralPath " + ps(checkout) + "; & " + " ".join(ps(value) for value in [python, "scripts/integration/run.py", *arguments]) + "; exit $LASTEXITCODE }"
    # OpenSSH joins the command tail. Quote the PowerShell argument for the Windows shell.
    encoded = __import__("base64").b64encode(script.encode("utf-16le")).decode("ascii")
    return ["ssh", "-o", "BatchMode=yes", host, "powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--transport", required=True, choices=["colima", "windows-ssh"])
    parser.add_argument("--checkout", required=True)
    parser.add_argument("--python", default="python3")
    parser.add_argument("--host")
    parser.add_argument("--profile", default="default")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    arguments = args.arguments[1:] if args.arguments[:1] == ["--"] else args.arguments
    if not arguments:
        parser.error("supply integration run arguments after --")
    try:
        return subprocess.run(remote_command(args.transport, args.checkout, args.python, arguments, args.host, args.profile), check=False).returncode
    except (ValueError, OSError) as error:
        print(json.dumps({"status": "FAIL", "error": "integration.transport", "reason": str(error)}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
