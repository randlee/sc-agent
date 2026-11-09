# Anchor tuning record (phase-a)

Kept for post-mortem analysis of how anchors and their wording were tuned.

- `rubric-2026-11-09/`: every testability-rubric run made while capturing anchors, as returned by
  `jev_judge.py rubric` (live Jev). `c*.json` are candidate anchors before locking (each file name
  carries its rewording round: no suffix, `b`, `c`, `d`); `cands*.tsv` hold the quote, statement and
  scope sent in each round. `rubric-all.json`, `r2-*.json` and `rubric-3.json` are runs on the locked
  anchors; `anchors.before-requote.json` is the anchor record before the REQ/ADR anchors were re-added
  with their records' own sentences as quotes, and `requote.tsv` lists what was re-added.
- Planning-gate runs are written by `jev_judge.py judge` to `../anchor-runs/`.
