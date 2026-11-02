---
name: quality-management-gh
version: 1.1.0
description: Reusable QA orchestration skill for GitHub PRs. Use for multi-pass QA, CI monitoring, and template-driven findings and final quality reports.
---

# Quality Management (GitHub)

This skill defines a reusable quality-management workflow for teams that run QA
across one or more passes before merge.

## Scope

Use this skill when you need to:
- run QA in multiple passes (`IN-FLIGHT`, `FAIL`, `PASS`)
- monitor CI progression for a PR
- publish structured findings to PR plus ATM
- publish a final QA closeout report on PASS

This skill is intentionally generic. Team-specific teammate names, branch
policy, and background-agent ownership stay in the repo’s `quality-mgr` agent
prompt.

## Template Installation and Vars

Before composing or sending a QA report, install daemon-readable copies with:

```bash
mkdir -p ~/.atm/templates/quality-management-gh && cp .claude/skills/quality-management-gh/*.j2 ~/.atm/templates/quality-management-gh/
```

Build the `--vars` JSON for this run from the selected template's frontmatter
`required_variables`; both templates list them. Every value comes from the
run: `task_id` and `commit` unchanged from the QA assignment, and counts from
the reviewer outputs. Keep numeric fields as JSON numbers and
`blocking_ids_json` as a JSON string because the templates embed it as JSON.
Write the vars file outside the repository working tree (in the session
scratchpad or a temp directory); never commit or stage it, and delete it or
let it expire after the send.
A vars file must never be copied from a previous report or a sample; a report
whose `sprint_id`, `task_id`, or `commit` do not match the assignment is a
false report.

## Required QA Status Contract

Every QA update, both ATM and PR, must include:
- sprint or task identifier
- branch, commit, PR number
- verdict (`PASS | FAIL | IN-FLIGHT`)
- deliverable completion (`complete`, `total`, `percent`)
- finding counts by severity (`blocking`, `important`, `minor`)
- blocking ids with concise summaries
- next required action plus owner
- merge readiness (`ready | not ready`) plus reason

**Cursor orchestration (optional):** when rendering via `sc-compose` for
`cursor-quality-mgr`, also supply `reviewer_spawn_gate`,
`reviewer_manifest_json`, `evidence_chain_json`, and on spawn failure
`missing_reviewers_json` / `unparsed_reviewers_json`. Templates embed them in
Machine Status JSON when `reviewer_spawn_gate` is non-empty. Codex/ATM callers
may omit those keys.

Use fenced JSON for machine-readable status payloads:

```json
{
  "sprint": "a-2",
  "task": "run-a-2",
  "branch": "sprint/a-2-config",
  "commit": "abc1234",
  "pr": 123,
  "verdict": "FAIL",
  "deliverables": {
    "complete": 9,
    "total": 11,
    "percent": 82
  },
  "findings": {
    "blocking": 1,
    "important": 2,
    "minor": 0
  },
  "blocking_ids": ["QA-001"],
  "merge_readiness": "not ready",
  "merge_reason": "Blocking findings remain",
  "next_action": "Fix lock acquisition rollback semantics",
  "owner": "team-lead"
}
```

A `PASS` report (`quality-report.md.j2`) emits this shape:

```json
{
  "sprint": "a-2",
  "task": "run-a-2",
  "branch": "sprint/a-2-config",
  "commit": "abc1234",
  "pr": 123,
  "verdict": "PASS",
  "findings": {
    "blocking": 0,
    "important": 0,
    "minor": 0
  },
  "blocking_ids": [],
  "merge_readiness": "ready",
  "merge_reason": "All reviewers PASS",
  "next_action": "none",
  "owner": "none",
  "recommendation": "Merge"
}
```

## QA Lifecycle (Multi-Pass)

1. Initial pass: usually `FAIL` with findings.
   - If Rust best-practices review is in scope, run its open-ended pass in QA-1.
2. Fix passes: `IN-FLIGHT` or `FAIL` while fixes are in progress.
   - QA-2 and later rounds must not re-run subjective reviewers open-ended on
     the same sprint branch.
   - Dispatch a subjective reviewer only to verify explicitly assigned
     carry-forward findings it owns, with its output scope-locked to those ids.
   - Every prior finding remains in the merge gate until its owning reviewer
     verifies it as fixed; do not defer unresolved findings to a later phase.
     The only exclusion is a finding the lead upholds as `rejected: ceremony`
     under `quality-mgr.md` "Ceremony Disputes"; record it in the next report.
3. Final pass: `PASS` with final quality report and merge recommendation.
   A plan-review round that leaves only minor findings reports
   `PASS — minor fixes required, no re-QA`, listing them.

Do not treat QA as single-shot.

## CI Monitoring

Preferred repo-specific flow:
- use `atm gh monitor status` to verify monitor health when available
- use `atm gh monitor pr <PR> --start-timeout 120` to start or attach a PR monitor when available
- use `atm gh pr report <PR> --json` for one-shot structured status when available

Fallback when repo-specific `atm gh` tooling is unavailable or not yet wired:
- `gh pr checks <PR> --watch`
- `gh pr view <PR> --json mergeStateStatus,reviewDecision`

If monitoring cannot start, include the failure in QA status and proceed with
one-shot PR report data.

## Findings Report to PR (Blocking)

Template:
- `~/.atm/templates/quality-management-gh/findings-report.md.j2`

Recommended flow:
1. Gather findings from QA agents.
2. Render the installed template with required variables.
3. When rechecking prior findings, include a resolved-findings section for
   items closed since the previous pass.
4. Post to the PR as a blocking review or status comment.

Suggested commands:
- blocking review:
  `atm compose --template ~/.atm/templates/quality-management-gh/findings-report.md.j2 --vars <scratch>/qa-<pr>-vars.json | gh pr review <PR> --request-changes --body-file -`
- in-flight update:
  `atm compose --template ~/.atm/templates/quality-management-gh/findings-report.md.j2 --vars <scratch>/qa-<pr>-vars.json | gh pr comment <PR> --body-file -`
- ATM verdict:
  `atm task close <task-id> completed --template ~/.atm/templates/quality-management-gh/findings-report.md.j2 --vars <scratch>/qa-<pr>-vars.json`

## Final Quality Report to PR (Closeout)

Template:
- `~/.atm/templates/quality-management-gh/quality-report.md.j2`

Recommended flow:
1. Confirm final QA pass and summarize validation scope.
2. Render the installed template with required variables.
3. Post as final closeout review or comment.

Suggested command:
- PR closeout:
  `atm compose --template ~/.atm/templates/quality-management-gh/quality-report.md.j2 --vars <scratch>/qa-<pr>-vars.json | gh pr review <PR> --approve --body-file -`
- ATM verdict:
  `atm task close <task-id> completed --template ~/.atm/templates/quality-management-gh/quality-report.md.j2 --vars <scratch>/qa-<pr>-vars.json`

Use the final template only for `PASS` closeout.

## PR Update Conventions

- First QA pass posts detailed findings with `FAIL` and should use
  `--request-changes`.
- Fix-pass updates revise status and open findings.
- Final pass posts `PASS` closeout with residual risk and readiness and should
  use `--approve`.
- Do not keep QA results ATM-only when a PR exists; append every completed QA
  update to the PR.
- Rendered reports must include a fenced JSON block for machine parsing.

## ATM Coordination Protocol

The sequence for every QA task assignment is defined once in
[`docs/team-protocol.md`](../../../docs/team-protocol.md) (Required Flow):
task start, work, task close. The verdict travels in the `atm task close` report
above; the close is terminal and the lead never acknowledges it. No silent
processing.
