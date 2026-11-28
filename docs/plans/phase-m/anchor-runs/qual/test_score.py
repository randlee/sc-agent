#!/usr/bin/env python3
"""Offline test of score.py with hand-built answer records. Run: python3 test_score.py"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import score  # noqa: E402


def rec(rel, comp, conf, kind="directive", item="m-x", anchor="ANC-T"):
    a = lambda c, p: {"choice": c, "confidence": p}  # noqa: E731
    return {"item_id": item, "anchor_id": anchor, "kind": kind,
            "answers": {"relevant": a(*rel), "addressed": a(*comp), "contradicted": a(*conf)}}


class ScoreTest(unittest.TestCase):
    def test_confident_no_both_repeats_stops(self):
        c = score.score_cell([rec(("no", 0.9), ("no", 0.9), ("no", 0.9))] * 2)
        self.assertTrue(c["stop"])
        self.assertEqual(c["verdict"], "candidate_not_applicable")
        self.assertEqual(c["full"], "not_addressed")
        self.assertTrue(c["false_stop"])

    def test_confident_no_one_repeat_only_continues(self):
        c = score.score_cell([rec(("no", 0.9), ("yes", 0.9), ("no", 0.9)), rec(("no", 0.55), ("yes", 0.9), ("no", 0.9))])
        self.assertFalse(c["stop"])
        self.assertEqual(c["verdict"], "satisfied")

    def test_low_yes_is_not_deciding(self):
        c = score.score_cell([rec(("yes", 0.3), ("yes", 0.8), ("no", 0.8))] * 2)
        self.assertEqual(c["verdict"], "satisfied")

    def test_unclear_relevance_never_underspecified(self):
        c = score.score_cell([rec(("unclear", 0.9), ("no", 0.8), ("no", 0.9))] * 2)
        self.assertEqual(c["verdict"], "not_addressed")

    def test_stop_with_na_full_is_not_false_stop(self):
        c = score.score_cell([rec(("no", 0.7), ("unclear", 0.7), ("no", 0.9))] * 2)
        self.assertTrue(c["stop"])
        self.assertFalse(c["false_stop"])

    def test_prohibition_ignores_compliance(self):
        c = score.score_cell([rec(("yes", 0.9), ("no", 0.99), ("no", 0.8), kind="prohibition")] * 2)
        self.assertEqual(c["verdict"], "satisfied")
        c = score.score_cell([rec(("yes", 0.9), ("no", 0.99), ("yes", 0.7), kind="prohibition")] * 2)
        self.assertEqual(c["verdict"], "contradicted")

    def test_low_compliance_underspecified_and_unstable(self):
        c = score.score_cell([rec(("yes", 0.9), ("yes", 0.5), ("no", 0.9)), rec(("yes", 0.9), ("yes", 0.8), ("no", 0.9))])
        self.assertEqual(c["full_per_repeat"], ["underspecified", "satisfied"])
        self.assertEqual(c["verdict"], "unstable")

    def test_deterministic_only_passthrough_and_cli_files(self):
        d = {"item_id": "m-x", "anchor_id": "ANC-D", "kind": "structural", "verdict": "deterministic_only", "answers": {}}
        with tempfile.TemporaryDirectory() as tmp:
            paths = []
            for i in (1, 2):
                p = os.path.join(tmp, f"r{i}.jsonl")
                with open(p, "w") as fh:
                    fh.write(json.dumps(d) + "\n" + json.dumps(rec(("no", 0.95), ("no", 0.9), ("no", 0.9))) + "\n")
                paths.append(p)
            cells = {c["anchor"]: c for c in score.score(paths)}
        self.assertEqual(cells["ANC-D"]["verdict"], "deterministic_only")
        self.assertEqual(cells["ANC-T"]["verdict"], "candidate_not_applicable")


if __name__ == "__main__":
    unittest.main()
