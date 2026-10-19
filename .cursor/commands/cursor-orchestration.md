# /cursor-orchestration

Run Wyvern sprint/phase orchestration **inside this Cursor session**.

## Mandatory

1. Read and follow `.cursor/skills/cursor-orchestration/SKILL.md`.
2. Quality-mgr role → **only** `cursor-quality-mgr` (spawn/evidence enforcer;
   verifies parent-spawned reviewers + fenced JSON; never spawns reviewers).
3. **Parent always spawns reviewers** before `cursor-quality-mgr` runs.
4. **Never** spawn `quality-mgr` (ATM/Claude agent) while this command is active.
5. If any prompt or skill says `quality-mgr`, remap to `cursor-quality-mgr` and
   launch once — do not dual-dispatch.
6. Developer work → Task `rust-developer` with model from
   `.cursor/orchestration-agent-models.yaml` (user override wins).
7. Do not edit `.claude/skills/codex-orchestration/` or
   `.claude/agents/quality-mgr.md` as part of this flow.
8. Keep all authored paths repo-root-relative (see skill **Path portability**).
9. Use ambient `git` / `gh` / `sc-compose` only — no account or login flags.
10. Render assignments with the fenced `sc-compose` recipes in the skill.
11. Pass reviewer models from `.cursor/orchestration-agent-models.yaml`; for
    phase-ending QA prefer `gpt-5.6-terra-medium` on `rust-qa-agent` when
    available (else YAML default).

## Quick flow

Build `$_VARS` JSON maps with the fenced `sc-compose` recipes in
`.cursor/skills/cursor-orchestration/SKILL.md` (dev, QA, fix, and reviewer
assignments). Read `.cursor/orchestration-agent-models.yaml` before each Task
spawn.

1. Ensure sprint worktree via `/sc-git-worktree` from `develop`.
2. Render + assign dev:

```bash
sc-compose render \
  --root .cursor/skills/cursor-orchestration \
  --file dev-template.xml.j2 \
  --var-file "$_VARS"
```

3. On push (branch + SHA): open/update PR with ambient `gh`.
3b. **Pre-QA-1 RBP (optional):** parent spawns `rust-best-practices-agent` on
   planned QA-1 `review_targets`; route findings to `rust-developer` before
   first full QA round. QA-1 still requires RBP in the reviewer set.
4. **Parent reviewer spawn** (required before enforcer):
   - Determine required reviewers per `.cursor/agents/cursor-quality-mgr.md`
     **Default reviewer set**.
   - Render each reviewer assignment (skill + agent Tool recipes).
   - Spawn all reviewers as background Tasks in parallel; record `task_id`s.
   - Await fenced JSON from every reviewer.
5. Render QA assignment to **`cursor-quality-mgr`** with handoff filled:

```bash
sc-compose render \
  --root .cursor/skills/cursor-orchestration \
  --file qa-template.xml.j2 \
  --var-file "$_VARS"
```

`$_VARS` must include `reviewer_manifest_json` and `reviewer_handoff_json`.

6. Spawn **one** `cursor-quality-mgr` enforcer with the rendered QA XML (handoff
   required). If it returns `reviewer_spawn_gate: fail`, spawn missing reviewers
   and re-run — do not merge.

7. **Dev–QA loop** (repeat until both gates pass):
   - **Spawn gate:** `reviewer_spawn_gate=pass` with every required reviewer
     `task_id` + fenced JSON — enforcer rejects incomplete handoffs.
   - **QA gate:** PASS only with fenced JSON from every required reviewer,
     **0 Blocking + 0 Important + 0 Minor** open findings, and 100% deliverable
     completion.
   - **PR gate:** every QA round posts findings via `findings-report.md.j2` or
     `quality-report.md.j2` (spawn-gate fail included).
   - **CI gate:** all required PR checks green.
   - **Evidence gate:** parent correlates every `task_id` in PR Machine Status
     to completed Task subagents; `pr_comment_url` present; FAIL rounds cite
     triage `.ttl` paths; orchestration state `qa_rounds[]` updated.
   - On spawn-gate FAIL: parent re-spawns missing reviewers → re-run enforcer.
   - On finding FAIL: `/triaging-findings` → fix **all** finding ids + `.ttl`
     paths → push → parent re-spawns reviewers → re-run enforcer.
8. On **PASS + green CI + evidence chain complete**: merge; start next sprint.

```bash
gh pr checks <PR> --watch
gh pr view <PR> --json mergeStateStatus,reviewDecision
```

If the user names a sprint/phase/doc, use that as `sprint_doc` authority
(typically under `docs/plans/`).
