#!/usr/bin/env python3
"""One installed-wheel call in a fresh process. Keep the library's JSON error chain intact."""
import argparse
import json
from pathlib import Path
import sc_agent

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("operation", choices=["models", "send", "contract"])
parser.add_argument("--provider", default="typesafe")
parser.add_argument("--model", default="jev")
parser.add_argument("--config-dir", required=True)
parser.add_argument("--request", type=Path)
args = parser.parse_args()
try:
    if args.operation == "contract":
        print(json.dumps(sc_agent.integration_cases()))
    elif args.operation == "models":
        print(sc_agent.models(config_dir=args.config_dir))
    else:
        print(sc_agent.send(args.request.read_text(), model=args.model, provider=args.provider,
                            project="integration", agent_role="prerelease", config_dir=args.config_dir))
except sc_agent.SagError as error:
    print(json.dumps({"error": json.loads(error.args[2])}))
    raise SystemExit(1)
