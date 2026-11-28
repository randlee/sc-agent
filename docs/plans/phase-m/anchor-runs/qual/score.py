#!/usr/bin/env python3
"""Shared T5 scorer for the phase-m qualification pass (one script for every agent).

Input: one results.jsonl per repeat. Each line is one judged cell:
  {"item_id", "anchor_id", "kind", "answers": {qid: {"choice", "confidence"}}, "roles": {...} | null}
(the flattened `results[]` of a `jev_judge.py judge` run, plus the item id).

Rule (arch-ai, qual.txt):
- relevance "no" at confidence >= 0.6 on every repeat (at least two) -> candidate not_applicable;
  it becomes not_applicable only when the agent agrees (recorded outside this script).
- otherwise the tree continues: the verdict comes from the other gating answers at 0.6.
  A relevance "yes" (any confidence), "unclear", or a low "no" is not deciding and never makes
  a cell underspecified.
- for every stop the full verdict from the other answers is computed too, so false stops show.
- repeats whose full verdicts differ are reported as "unstable" with both verdicts.

Deterministic, standard library only, no network. Only the first ask (`answers`) of each repeat is read.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter

THRESHOLD = 0.6
DEFAULT_ROLES = {"relevance": "relevant", "compliance": "addressed", "conflict": "contradicted"}


def choice_of(answers: dict, qid: str | None) -> tuple:
    a = (answers or {}).get(qid) if qid else None
    if not isinstance(a, dict) or a.get("choice") is None:
        return None, 0.0
    return a["choice"], float(a.get("confidence") or 0.0)


def decide_others(answers: dict, roles: dict, kind: str, t: float = THRESHOLD) -> str:
    """Verdict from the compliance and conflict answers only; relevance is not consulted."""
    comp, comp_c = choice_of(answers, roles.get("compliance"))
    conf, conf_c = choice_of(answers, roles.get("conflict"))
    if conf == "yes" and conf_c >= t:
        return "contradicted"
    if kind == "prohibition":
        if conf != "no" or conf_c < t:
            return "underspecified"
        return "satisfied"
    if comp is None or comp == "unclear" or comp_c < t:
        return "underspecified"
    if comp == "no":
        return "not_addressed"
    if comp == "yes" and conf == "no" and conf_c >= t:
        return "satisfied"
    return "underspecified"


def score_cell(records: list, t: float = THRESHOLD) -> dict:
    first = records[0]
    out = {"item": first["item_id"], "anchor": first["anchor_id"], "kind": first.get("kind"), "repeats": len(records)}
    if not any(r.get("answers") for r in records):
        out.update(verdict=first.get("verdict") or "no_answers", stop=False, full=None, relevance=[])
        return out
    rels, fulls = [], []
    for r in records:
        roles = dict(DEFAULT_ROLES)
        roles.update(r.get("roles") or {})
        rels.append(list(choice_of(r["answers"], roles["relevance"])))
        fulls.append(decide_others(r["answers"], roles, r.get("kind") or "directive", t))
    stop = len(records) >= 2 and all(c == "no" and p >= t for c, p in rels)
    full = fulls[0] if len(set(fulls)) == 1 else "unstable"
    out.update(relevance=rels, stop=stop, full=full, full_per_repeat=fulls,
               verdict="candidate_not_applicable" if stop else full,
               false_stop=stop and full in ("satisfied", "contradicted", "not_addressed"))
    return out


def load(paths: list) -> dict:
    cells: dict = {}
    for p in paths:
        with open(p, encoding="utf-8") as fh:
            for n, line in enumerate(fh, 1):
                if not line.strip():
                    continue
                rec = json.loads(line)
                cells.setdefault((rec["item_id"], rec["anchor_id"]), []).append(rec)
    return cells


def score(paths: list, t: float = THRESHOLD) -> list:
    cells = load(paths)
    return [score_cell(cells[k], t) for k in sorted(cells)]


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("results", nargs="+", help="results.jsonl, one per repeat")
    ap.add_argument("--threshold", type=float, default=THRESHOLD)
    ap.add_argument("--out", help="write the cell list as JSON here")
    a = ap.parse_args(argv)
    cells = score(a.results, a.threshold)
    summary = {"cells": len(cells), "verdicts": dict(sorted(Counter(c["verdict"] for c in cells).items())),
               "stops": sum(c["stop"] for c in cells), "false_stops": sum(bool(c.get("false_stop")) for c in cells)}
    if a.out:
        with open(a.out, "w", encoding="utf-8") as fh:
            json.dump({"threshold": a.threshold, "inputs": a.results, "summary": summary, "cells": cells}, fh, indent=1)
    json.dump(summary, sys.stdout, indent=1)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
