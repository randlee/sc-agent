# tune-ANC-010-planning-2 supersedes tune-ANC-010-planning-1

Earlier folder: `tune-ANC-010-planning-1`. It stays as the earlier record.

Old oracle digest: `3809e9b1b1e0c8b979d4cf6dda8c7ca0f054e5fad222b3059aa7644621ec777e` (9 cases, ANC-010 v3)
New oracle digest: `ff8ac6a67259d15e90a4847f88cb3cfb0c3e37fe822137eccacc612f7314358f` (14 cases, ANC-010 v4)

What changed: the scope was rewritten as bead-level signals (anchor v4). It now covers sc-agent's own
crates, judge-jev, plan-anchor and the shared client template. Relevance probes were added:
irrelevant (docs-only, CI config, tracker), topical-only (plan note, retro agenda; no code change,
so relevant=no) and sc-agent crate probes (silent-crate, conflict-crate). The v3 irrelevant probe
(sc-agent model table) was dropped because v4 brings it into scope. The tuning history
`phase-m/ANC-010/planning` starts a fresh chain here (events: []).

Round 1 (round-1/) accepted no bundle. Round 2, run against the same frozen oracle, sealed `r4-a2-c2`.

owner approved 2026-11-28 08:13
