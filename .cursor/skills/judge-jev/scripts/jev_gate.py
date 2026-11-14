"""The confidence gate over repeated ``quality`` runs of identical record text.

``jev_gate.py [--threshold 0.6] RUN.json [RUN.json ...]`` reads the result files of
one or more ``jev_judge.py quality`` runs of the same records and prints one JSON
envelope. For every gate answer it takes the middle (median) confidence across the
runs. A record passes when its verdict is ``yes`` in more than half of the runs and
every gate answer's median is at or above the threshold. Answers whose id starts
with ``d_`` are diagnostic and never gate.

Each answer below the gate is listed with its ``runner_up``: the option Jev most
nearly chose instead, with the median of its probability and its criterion. A low
confidence is Jev torn between its choice and that option, so the runner-up says
what the statement is being read as.

Runs are comparable only on identical text: a record whose statements differ
between runs, or that is missing from a run, is reported with ``comparable: false``
and does not pass. Exit 0 whenever the gate was computed (failing records
included); exit 2 with the typed error envelope when a run file cannot be used.

Standard library only. Calls no Jev and needs no key.
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from collections import Counter

from result import run_main

SCRIPT = "jev_gate.py"
DEFAULT_THRESHOLD = 0.6


class Usage(Exception):
    pass


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise Usage(message)


def failure(code: str, message: str, action: str) -> int:
    envelope = {"success": False, "data": None,
                "error": {"code": code, "message": message, "recoverable": False, "suggested_action": action}}
    print(json.dumps(envelope, indent=2, sort_keys=True, ensure_ascii=False))
    return 2


def gate_answers(result: dict) -> dict:
    """``{(statement index or 0, question id): (choice, confidence, statement text, answer)}`` for one record's gate
    answers; ``answer`` is the reason entry, which holds ``probabilities`` and ``runner_up`` when the run reported them."""
    found = {}
    for wave in result.get("waves") or []:
        for reason in wave.get("reason") or []:
            if "answers" in reason:
                statement = reason.get("statement") or {}
                for answer in reason["answers"]:
                    if not str(answer["id"]).startswith("d_"):
                        found[(statement.get("index"), answer["id"])] = (
                            answer["choice"], float(answer["confidence"]), statement.get("text"), answer)
            elif "id" in reason and not str(reason["id"]).startswith("d_"):
                found[(0, reason["id"])] = (reason["choice"], float(reason["confidence"]), None, reason)
    return found


def runner_up(entries: list, choice: str):
    """The option Jev most nearly chose instead of ``choice``, across the runs' reason entries: its name, the median of
    its probability and its criterion. None when a run reported no probabilities or no other option has any."""
    tables = [entry.get("probabilities") for entry in entries]
    if not tables or not all(isinstance(table, dict) and table for table in tables):
        return None
    options = sorted(set().union(*(set(table) for table in tables)) - {choice})
    medians = {option: round(statistics.median(float(table.get(option, 0.0)) for table in tables), 4) for option in options}
    if not medians:
        return None
    best = min(medians, key=lambda option: (-medians[option], option))
    if medians[best] <= 0:
        return None
    criterion = next((entry["runner_up"].get("criterion", "") for entry in entries
                      if isinstance(entry.get("runner_up"), dict) and entry["runner_up"].get("choice") == best), "")
    return {"choice": best, "probability": medians[best], "criterion": criterion}


def gate(runs: list, threshold: float) -> dict:
    """The gate over ``runs``: each is ``{record id: quality result}`` for one run."""
    ids = sorted(set().union(*(set(run) for run in runs)))
    records = []
    for record_id in ids:
        present = [run[record_id] for run in runs if record_id in run]
        answers = [gate_answers(result) for result in present]
        verdicts = [result.get("verdict") for result in present]
        kind = (present[0].get("record") or {}).get("kind")
        texts = [{key: value[2] for key, value in found.items()} for found in answers]
        comparable = len(present) == len(runs) and all(text == texts[0] for text in texts)
        low, lowest = [], None
        for key in sorted(answers[0], key=lambda k: (k[0] or 0, k[1])) if comparable else []:
            confidences = [found[key][1] for found in answers]
            median = round(statistics.median(confidences), 4)
            choice = Counter(found[key][0] for found in answers).most_common(1)[0][0]
            entry = {"statement": key[0] or None, "question": key[1], "choice": choice,
                     "median": median, "confidences": confidences,
                     "runner_up": runner_up([found[key][3] for found in answers], choice)}
            if lowest is None or median < lowest["median"]:
                lowest = entry
            if median < threshold:
                low.append(entry)
        yes = sum(1 for verdict in verdicts if verdict == "yes")
        passed = comparable and yes * 2 > len(runs) and not low
        records.append({"id": record_id, "kind": kind, "pass": passed, "comparable": comparable,
                        "verdicts": verdicts, "lowest": lowest, "low": low})
    return {"threshold": threshold, "runs": len(runs),
            "passed": sum(1 for record in records if record["pass"]),
            "failed": sum(1 for record in records if not record["pass"]),
            "records": records}


def load(path: str):
    """``{record id: result}`` of one run file, or an error tuple ``(code, message)``."""
    try:
        with open(path, encoding="utf-8") as handle:
            envelope = json.load(handle)
    except OSError as exc:
        return ("INPUT.RUN_UNREADABLE", f"{path}: {exc.strerror or exc}")
    except json.JSONDecodeError as exc:
        return ("INPUT.RUN_NOT_JSON", f"{path}: not JSON ({exc.msg} at line {exc.lineno})")
    if not isinstance(envelope, dict) or not envelope.get("success"):
        code = ((envelope.get("error") or {}).get("code") if isinstance(envelope, dict) else None) or "unknown"
        return ("INPUT.RUN_FAILED", f"{path}: the run did not succeed ({code}); a failed call is not a verdict")
    results = (envelope.get("data") or {}).get("results")
    if not isinstance(results, list) or not results:
        return ("INPUT.RUN_EMPTY", f"{path}: no results in data.results")
    return {(result.get("record") or {}).get("id"): result for result in results}


def main() -> int:
    parser = Parser(prog=SCRIPT, description="Median confidence gate over repeated quality runs of identical text.")
    parser.add_argument("runs", nargs="+", metavar="RUN.json", help="result files of quality runs of the same records")
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD,
                        help=f"lowest passing median confidence (default {DEFAULT_THRESHOLD})")
    try:
        args = parser.parse_args()
    except Usage as exc:
        return failure("VALIDATION.INPUT", f"{SCRIPT}: {exc}", f"Run '{SCRIPT} --help'.")
    if not 0.0 <= args.threshold <= 1.0:
        return failure("VALIDATION.INPUT", f"{SCRIPT}: --threshold must be between 0 and 1", f"Run '{SCRIPT} --help'.")
    runs = []
    for path in args.runs:
        loaded = load(path)
        if isinstance(loaded, tuple):
            return failure(loaded[0], loaded[1], "Rerun that quality call once, then pass its result file again.")
        runs.append(loaded)
    print(json.dumps({"success": True, "data": gate(runs, args.threshold), "error": None},
                     indent=2, sort_keys=True, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(run_main(main))
