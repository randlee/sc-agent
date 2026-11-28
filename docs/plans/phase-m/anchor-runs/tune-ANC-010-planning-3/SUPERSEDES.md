# tune-ANC-010-planning-3 continues the chain from tune-ANC-010-planning-2

This run continues the tuning history `phase-m/ANC-010/planning` from planning-2: the request's
`tuning_history` is planning-2's `tuning_history_after`. planning-2 stays as the earlier record.

Oracle: unchanged. Frozen digest `ff8ac6a67259d15e90a4847f88cb3cfb0c3e37fe822137eccacc612f7314358f`, the same as planning-2 (14 cases).

Candidate bundle: `r4-cur-cur` only. The runner adds the baseline itself. No padding bundles were added. The
candidates file still lists unused wordings (relevant r5, r6, sig2; addressed a2, a3; contradicted c2, c3)
only so that each role meets the three-candidate minimum. They are in no bundle, so they were not evaluated.

Reason for the override (arch-ai ruling dec4, 2026-11-28): ANC-010 is a prohibition, so relevance and
conflict decide its verdict. r4-a2-c2 answers conflict at 0.28 to 0.33 on the compliant probe, below the
0.6 threshold, while r4-cur-cur holds 0.67 or higher on both deciding roles.
