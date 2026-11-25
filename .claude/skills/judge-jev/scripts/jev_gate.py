"""The confidence gate over repeated ``quality`` runs of identical record text.

``jev_gate.py [--threshold 0.6] RUN.json [RUN.json ...]`` reads the result files of
one or more ``jev_judge.py quality`` runs of the same records and prints one JSON
envelope. For every gate answer it takes the middle (median) confidence across the
runs. A record passes when its verdict is ``yes`` in more than half of the runs and
every gate answer's median is at or above the threshold. Answers whose id starts
with ``d_`` are diagnostic and never gate.

Every gate answer carries its median and the original choice, criterion, confidence
and runner-up from each run. Missing reason fields are null, not inferred.

Runs are comparable only on identical text: a record whose statements differ
between runs, or that is missing from a run, is reported with ``comparable: false``
and does not pass. A gate answer that only some runs asked (a later wave that runs
only for some answers, such as ``joined_by`` after ``obligations`` 2+) is scored over
the runs that asked it, carries ``asked`` (how many did), and gates only when more
than half of the runs asked it; otherwise it is listed with ``gating: false``. Exit 0 whenever the gate was computed (failing records
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
    criterion = next((entry["runner_up"].get("criterion") for entry in entries
                      if isinstance(entry.get("runner_up"), dict) and entry["runner_up"].get("choice") == best), None)
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
        keys = set().union(*(set(found) for found in answers))
        # comparable: every run present, and every answer any two runs share judged the same statement text
        comparable = len(present) == len(runs) and all(
            len({found[key][2] for found in answers if key in found}) == 1 for key in keys)
        low, all_answers, lowest = [], [], None
        for key in sorted(keys, key=lambda k: (k[0] or 0, k[1])) if comparable else []:
            asking = [found for found in answers if key in found]  # a later wave may ask a key in some runs only
            gating = len(asking) * 2 > len(runs)
            confidences = [found[key][1] for found in asking]
            median = round(statistics.median(confidences), 4)
            choice = Counter(found[key][0] for found in asking).most_common(1)[0][0]
            originals = [found[key][3] for found in asking]
            criterion = next((item.get("criterion") for item in originals
                              if item.get("choice") == choice and item.get("criterion") is not None), None)
            per_run = []
            for item in originals:
                alternative = item.get("runner_up")
                per_run.append({"choice": item.get("choice"), "criterion": item.get("criterion"),
                                "confidence": item.get("confidence"),
                                "runner_up": {key: alternative.get(key) for key in ("choice", "probability", "criterion")}
                                if isinstance(alternative, dict) else None})
            entry = {"statement": key[0] or None, "question": key[1], "choice": choice, "criterion": criterion,
                     "median": median, "confidences": confidences, "runs": per_run,
                     "runner_up": runner_up(originals, choice), "asked": len(asking), "gating": gating}
            all_answers.append(entry)
            if not gating:
                continue
            if lowest is None or median < lowest["median"]:
                lowest = entry
            if median < threshold:
                low.append(entry)
        yes = sum(1 for verdict in verdicts if verdict == "yes")
        passed = comparable and yes * 2 > len(runs) and not low
        records.append({"id": record_id, "kind": kind, "pass": passed, "comparable": comparable,
                        "verdicts": verdicts, "lowest": lowest, "low": low, "answers": all_answers})
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
