# plan-anchor 0.16.1: two tool issues for Rand (written up, NOT filed)

## 1. No supersede event for an owner-approved oracle change

`jev_judge.py tournament` refuses any oracle whose digest differs from the last tuning-history event's oracle (`freeze_oracle`, around jev_judge.py line 4313). There is no supported way for an owner-approved new oracle to supersede the old one: not for an added case (ANC-008), and not for a statement version change with unchanged labels (ANC-023, 027, 030, 031). jev-api.md describes it as "tampering is refused, never re-frozen".

Command (ANC-008, from `~/github/sc-agent-worktrees/integrate/phase-m`):

    source ~/.venvs/sc-decidr-0.4.1/bin/activate; export SC_PROJECT=sc-agent SC_PHASE=m; python3 .claude/skills/plan-anchoring/scripts/jev_judge.py tournament --request <scratch>/tune/anc008r3/request.json --oracle <scratch>/tune/anc008r3/oracle.json --candidates <scratch>/tune/anc008r3/candidates.json --out docs/plans/phase-m/anchor-runs/tune-ANC-008-planning-3

Output (exit 2):

    TOURNAMENT.ORACLE_CHANGED: oracle digest ce6df3dd376490d5e1b2de2b0671edc892a5bcb47ac6ba38f7cded9227fc39c7 differs from the tuning history's frozen oracle f1e862de9f53f42e298c3cee637442dd5cc706a4984590058fd63b689a7a155a
    recoverable: false
    suggested_action: Never add, remove or relabel an oracle case after an answer; evaluate new candidates against the frozen oracle.

The same error came for ANC-027 (28564ec9… vs cbf96da8…), ANC-030 (59107ac3… vs 48706e98…), ANC-031 (a790dcf1… vs 8e8e96eb…) and ANC-023 (0056ad92… vs 7da9a1bb…).

Workaround used, owner-approved 2026-11-28 08:13 ("1 - a"): start a fresh history chain (`events: []`, same history_id) and record the supersession by hand in SUPERSEDES.md in the new run folder. Wanted: a supersede event in the tuning history that names the earlier chain, the old and new oracle digests, the reason and the approval.

## 2. Auto-named cross-product bundles fail the runner's own id pattern

When the candidates file lists per-role wordings but no explicit `bundles`, the runner names the cross-product bundles like `Bc1+c1+c1`. Its own bundle id pattern rejects `+`, so the run fails with `TOURNAMENT.CANDIDATES_INVALID` before any Jev call.

Reproduce: the same tournament command with a candidates file that has three wordings per role and no `bundles` key.

Workaround used by the anchor-tuning agent: list the bundles explicitly with pattern-valid ids (b1..bN). Wanted: generate ids that match the pattern, or allow the generated form.
