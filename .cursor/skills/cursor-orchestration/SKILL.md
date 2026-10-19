---
name: cursor-orchestration
description: >-
  Orchestrate Wyvern sprint/phase work inside a single Cursor session.
  Parent spawns reviewers and rust-developer; Task(cursor-quality-mgr) enforces
  spawn proof, aggregates fenced JSON evidence, triages findings, publishes PR
  reports, and owns the QA gate. Use when the user asks for
  /cursor-orchestration, Cursor-session phase/sprint orchestration, or
  same-session QA via cursor-quality-mgr. Never use ATM quality-mgr or
  codex-orchestration while this skill governs the session.
disable-model-invocation: true
---

# Cursor Orchestration

Same-session Cursor adaptation of the repo's codex-orchestration gate contract.
Does **not** modify `.claude/skills/codex-orchestration/` or
`.claude/agents/quality-mgr.md`.

## Hard role map (non-negotiable)

| Role | Agent | Invocation |
|------|-------|------------|
| Orchestrator | this parent session | spawns `rust-developer`, reviewers, `cursor-quality-mgr`, `qa-triage` |
| Developer | `rust-developer` | parent Task `subagent_type: rust-developer` |
| Quality manager | **`cursor-quality-mgr` only** | parent Task — **spawn/evidence enforcer** (see **cursor-quality-mgr enforcer role**) |
| Reviewers | shared `.claude` agents | **parent** Task spawns: `req-qa`, `arch-qa`, `rust-qa-agent`, `rust-best-practices-agent`, `rust-service-hardening-agent`, `flaky-test-qa` |

### Quality-mgr binding (critical)

While this skill governs the session:

1. The quality-mgr **role** is always fulfilled by **`cursor-quality-mgr`**.
2. **Never** spawn Task `subagent_type: quality-mgr` (the Claude/ATM agent).
3. If any template, sibling skill, triage note, or prompt says `quality-mgr` /
   `assignee="quality-mgr"`, **rewrite** to `cursor-quality-mgr` and launch
   once. Do not dual-dispatch.
4. Per QA round: **at most one** QA coordinator Task, and it must be
   `cursor-quality-mgr` (or the fallback that loads that prompt only).
   **Never** spawn a second QA Task for the same `task_id` / `qa_pass` while the
   first is still running.
5. Do not follow `codex-orchestration`, ATM team-lead QA handoffs, or any path
   that assigns ATM `quality-mgr` in parallel with this skill.
6. **Parent always spawns reviewers** before `cursor-quality-mgr` runs (see
   **Parent reviewer spawn**). `cursor-quality-mgr` enforces spawn proof and
   never spawns reviewer Tasks.
7. Parent does **not** merge on narrative QA PASS alone — see **Reviewer spawn
   merge gate** below.

## cursor-quality-mgr enforcer role (critical)

`cursor-quality-mgr` is **not** a reviewer dispatcher. It is the **spawn and
evidence enforcer** for each QA round:

1. **Verify spawn proof** — handoff manifest lists every required reviewer with
   non-empty `task_id` and `spawn_actor: parent-orchestrator`.
2. **Collect evidence** — parse fenced JSON from each reviewer response in
   `<parent-reviewer-handoff>` (no coordinator inference).
3. **Analyze and correlate** — union reviewer findings with TODO-scan hits;
   correlate prior triage `.ttl` records on follow-up rounds (parent owns
   `/triaging-findings` after finding FAIL).
4. **Publish findings report** — render `findings-report.md.j2` or
   `quality-report.md.j2` and post to the PR on **every** QA round.
5. **Reject incomplete rounds** — if the parent failed to spawn the proper
   reviewer set or any fenced JSON is missing/unparsed, FAIL with
   `reviewer_spawn_gate: fail` and `next_action: parent_respawn_reviewers`.
   Parent must spawn missing reviewers and re-submit a complete handoff.

Forbidden for `cursor-quality-mgr`: spawning any Task, cargo/clippy/test review,
grep-based deliverable verification, inventing reviewer verdicts.

## Parent constraints

- Coordinator only: no product code, no cargo/clippy/test **review** analysis.
- **Parent may spawn reviewer Tasks** and record manifests — that is dispatch,
  not reviewer substitution.
- Parent must **not** grep/read source to decide deliverable presence or invent
  reviewer verdict tables without fenced JSON from spawned reviewers.
- Persist state to disk (sprint docs, PR #, SHA, triage records, QA verdicts).
- Prefer short Task completion summaries over pasting full agent transcripts.
- Worktrees via `/sc-git-worktree` (never switch main repo off `develop`).
- Use ambient `git` / `gh` only — no account flags, no `gh auth login`, no
  `--hostname` / `--user` overrides unless the user explicitly requires them.

## Path portability (required)

All authored paths in this skill, its templates, the `/cursor-orchestration`
command, and `.cursor/agents/cursor-quality-mgr.md` must be portable:

- Use **repo-root-relative** paths (e.g. `.cursor/skills/...`,
  `.claude/agents/...`, `docs/plans/...`).
- Skill-local template names may be basename-only when the skill directory is
  already implied (e.g. `dev-template.xml.j2`).
- **Never** hardcode host-absolute paths (`/Users/...`, `/Volumes/...`,
  `/home/...`, `C:\...`, `D:\...`, `\\server\...`).
- Do not embed machine-specific roots in examples or defaults.
- Worktree layout is a **sibling of the primary repo root** named
  `wyvern-worktrees/<branch>`. Always create/resolve via `/sc-git-worktree`
  (or git common-dir → primary root → sibling `wyvern-worktrees`). **Never**
  assume `../wyvern-worktrees` is valid from the current cwd (it fails when
  already inside a worktree).
- Runtime assignment fields (`worktree_path`, etc.) may be absolute **only**
  when resolved dynamically for the current machine; authored skill/prompt
  content must still use relative forms and placeholders like
  `{{ worktree_path }}`.

## Top-level spawn requirement (critical)

The **parent orchestrator session** (this chat / top-level agent) must run the
full dev→QA loop. **Never** delegate the entire orchestration loop to a nested
Task subagent — nested agents cannot spawn `rust-developer`, reviewers, or
`cursor-quality-mgr` (same platform limit that drove parent reviewer spawn).

| Who spawns | Allowed |
|------------|---------|
| Top-level parent session | `rust-developer`, reviewers, `cursor-quality-mgr`, `qa-triage` |
| Nested Task subagent (orchestrator) | **Forbidden** — investigation/prep only |
| `cursor-quality-mgr` | **Forbidden** — spawn/evidence enforcer only (no Task spawns) |

Nested subagents may read skills, fetch PR comments, create triage directories,
or draft assignments — then **return control** to the parent for all Task spawns.

## Parent reviewer spawn (default — every QA round)

Nested `cursor-quality-mgr` Tasks cannot spawn reviewer Tasks in Cursor (c.11).
**Parent orchestrator always spawns reviewers**; `cursor-quality-mgr` enforces
spawn proof and publishes findings — it never spawns reviewers.

### Required parent behavior

1. Determine the required reviewer set for this `qa_pass` using
   `.cursor/agents/cursor-quality-mgr.md` **Default reviewer set** (QA-1 vs
   QA-2+, phase-ending, plan review).
2. Expand `review_targets` when `review_mode` is neither `round_limit` nor
   `plan` (same rules as `cursor-quality-mgr.md`):
   `git diff <integration_branch>...HEAD --name-only` in the sprint worktree.
3. Render each reviewer assignment with fenced `sc-compose` recipes in
   `.cursor/agents/cursor-quality-mgr.md` (or skill **Tool recipes**).
4. Spawn **every** selected reviewer as a **background Task** in parallel:
   - `subagent_type` = reviewer agent name
   - `model` from `.cursor/orchestration-agent-models.yaml`
   - prompt = **only** the rendered JSON assignment
5. Record each `agent` + `task_id` in the reviewer manifest **before** awaiting.
6. Await every reviewer Task; extract fenced JSON from each response.
7. If any required reviewer is missing, lacks `task_id`, or returns unparsed
   JSON → **FAIL the round** immediately (parent may spawn `cursor-quality-mgr`
   only to publish the FAIL report, or publish inline per agent recipes).
8. Attach manifest + fenced JSON to the QA assignment handoff (see
   `qa-template.xml.j2` `<parent-reviewer-handoff>`) before spawning
   `cursor-quality-mgr`.

Every `spawned_reviewers[]` entry must include
`"spawn_actor": "parent-orchestrator"`.

## Running cursor-quality-mgr

Try in order; stop at the first success. Never fall through to `quality-mgr`.
Parent must complete **Parent reviewer spawn** and fill `<parent-reviewer-handoff>`
before launching the enforcer.

1. **Preferred:** Cursor Task with `subagent_type: cursor-quality-mgr` and the
   rendered QA XML **including** `<parent-reviewer-handoff>` (manifest + fenced
   JSON from parent reviewer spawn) as the Task prompt (plus planned model).
2. **If the Task enum rejects `cursor-quality-mgr`:** spawn a Task whose prompt
   begins with: read and adopt `.cursor/agents/cursor-quality-mgr.md`, then
   execute the rendered QA XML assignment. Do **not** use
   `subagent_type: quality-mgr`.
3. **Custom subagent:** if the product exposes project agents by name, invoke
   the `cursor-quality-mgr` agent with the same QA XML payload.

If `cursor-quality-mgr` returns `reviewer_spawn_gate: fail`, parent spawns the
listed missing reviewers, rebuilds the handoff, and re-runs the enforcer — do
not merge and do not skip reviewers.

## Agent model defaults

Read `.cursor/orchestration-agent-models.yaml` before launching any subagent.
Override only when the user names a model for a role. Always pass `model:`
explicitly on Task spawns (do not rely on agent frontmatter defaults).

- Parent orchestrator: current session model (not in the table).
- `inherit` entries: use the agent frontmatter default.
- `alternates` on an agent: honor only when the user explicitly prefers that model.

### Phase-ending review model override

For phase-ending QA (`review_mode` / assignment indicating phase-end), use the
YAML defaults **except**:

- Prefer **`gpt-5.6-terra-medium`** (GPT-5.6 Terra) for **`rust-qa-agent`**
  when available in the Cursor Task model list; otherwise fall back to the YAML
  default for `rust-qa-agent`.
- Do not replace `arch-qa`'s Sonnet assignment; keep the Claude precision gate
  and the Terra comprehensive reviewer as a deliberate mix.

Other phase-end reviewers stay on their YAML defaults.

## Preconditions

1. Sprint/phase target defined in `docs/requirements.md`,
   `docs/architecture.md`, and `docs/plans/project-plan.md` (or linked phase plan).
2. Sprint worktree exists (create via `/sc-git-worktree` from `develop` if missing).
3. These exist and are readable (repo-root-relative):
   - `.cursor/orchestration-agent-models.yaml`
   - `.cursor/agents/cursor-quality-mgr.md`
   - `.claude/agents/{req-qa,arch-qa,flaky-test-qa,rust-qa-agent,rust-best-practices-agent,rust-service-hardening-agent,rust-developer}.md`
   - `.claude/skills/quality-management-gh/SKILL.md`
   - `.claude/assets/sc-rust/quality-mgr/quality-mgr.rust.md`
4. `sc-compose` is on `PATH` and usable (see **Tool recipes**).

## Dev–QA loop (mandatory per sprint)

Every sprint runs the same closed loop until **both** gates pass:

```
dev → push → PR → QA → [FAIL → triage → fix → push → re-QA]* → PASS + green CI → merge
```

| Gate | Requirement |
|------|-------------|
| **QA** | `cursor-quality-mgr` declares **PASS** only when deliverables are 100% complete, **`reviewer_spawn_gate: pass`**, every required reviewer returned **fenced JSON**, every required reviewer PASSes, **TODO scan clean or TODO findings included in open counts**, and **zero open findings** at any severity (**0 Blocking + 0 Important + 0 Minor**) from **reviewer JSON ∪ TODO scan**. |
| **CI** | All required PR checks green (`gh pr checks <PR> --watch`). Merge is blocked while any check fails or is pending. |

### Reviewer spawn merge gate (parent — non-negotiable)

Do **not** merge a sprint PR unless **all** of the following are true:

1. Latest PR QA comment Machine Status JSON includes `"reviewer_spawn_gate": "pass"`.
2. `reviewer_manifest` lists every required reviewer with non-empty `task_id`
   correlatable to **completed Task subagents in this parent session**.
3. `evidence_chain` includes `pr_comment_url` for this `qa_pass` (after successful
   `gh` post) and triage `.ttl` paths when the round was finding FAIL→fix.
4. Parent QA verdict duplicates the PR Machine Status JSON (dual publish).
5. `.cursor/<phase>-orchestration.json` has a `qa_rounds[]` entry for this pass.
6. Finding counts match **reviewer fenced JSON ∪ TODO-scan** union.

If the PR report lacks `reviewer_spawn_gate` / `reviewer_manifest`, treat QA as
**INCOMPLETE** — re-run `cursor-quality-mgr` for that `qa_pass`; do not merge.

If two conflicting QA comments exist on the same PR (e.g. PASS then FAIL),
**FAIL wins**; require fix + QA-2 before merge.

**Fix scope on FAIL:** route **every** finding id (Blocking, Important, and Minor) back to `rust-developer` via `fix-assignment.xml.j2`. Do not merge after fixing only Important/Blocking findings while Minors remain open.

**QA rounds:** QA-1 runs the full reviewer set (incl. RBP + service-hardening). QA-2+ omits RBP and service-hardening but the merge gate stays **0B+0I+0m** — prior-round findings must be fixed before re-QA.

**Sequence:** one sprint at a time. Do not start sprint N+1 until sprint N is merged to `integrate/phase-N` (or the sprint's `pr_target`).

## Chain of evidence (non-negotiable)

Merge is blocked unless an auditable evidence chain exists for the **latest**
`qa_pass` on that PR. Self-attested prose without correlatable artifacts is
invalid (c.9-class failure).

### Evidence layers

| Layer | What proves it | Required artifact |
|-------|----------------|-------------------|
| **Reviewer spawn** | Sub-agents actually ran | Parent-spawned `reviewer_manifest[].task_id` per required reviewer — ids from Cursor Task tool returns, not invented; `spawn_actor: parent-orchestrator` |
| **Reviewer output** | Findings came from reviewers | Parseable fenced JSON per agent; aggregated counts match **reviewer JSON ∪ TODO scan** |
| **Triage** | Findings were correlated before fix | `.triage/<phase_id>/findings/<finding_id>.ttl` paths; `qa-triage` Task ids when triage ran |
| **Fix** | Dev addressed triaged scope | Fix assignment lists every finding id + triage `.ttl` paths; push SHA after fix |
| **PR publish** | Stakeholders can audit QA | `gh pr comment` or review URL for **each** QA round; `detailed_findings_md` lists **all** severities |
| **Coordinator handoff** | Parent/ATM received same facts | Parent verdict + persisted orchestration state contain **identical** Machine Status JSON as PR post |

### Parent correlation (fail-closed)

Before merge, the parent orchestrator must:

1. Read the latest PR QA comment Machine Status JSON for this `qa_pass`.
2. Confirm every `reviewer_manifest[].task_id` matches a **completed** Task
   subagent from this session (notification id or transcript correlation).
3. If any required `task_id` is missing, empty, or uncorrelatable → **QA
   INCOMPLETE** — do not merge; re-run `cursor-quality-mgr`.
4. On FAIL rounds, confirm `/triaging-findings` produced `.ttl` records before
   fix dispatch; fix assignment must cite those paths.
5. Append a `qa_rounds[]` entry to `.cursor/<phase>-orchestration.json` (or
   sprint-local orchestration state) with: `qa_pass`, `commit`, `verdict`,
   `pr_comment_url`, `reviewer_manifest`, `triage_ttl_paths`, `finding_ids`,
   `coordinator_task_id`.

### Dual publish (PR + coordinator)

`cursor-quality-mgr` must publish the **same** rendered report body to:

1. **PR** — `gh pr comment` or `gh pr review` (every QA round)
2. **Parent** — final verdict message includes the same Machine Status JSON block

Codex `quality-mgr` equivalent: PR + ATM message per `quality-management-gh`.

### Machine Status vs sc-compose vars

| sc-compose `$_VARS` key (render input) | Rendered Machine Status key |
|---------------------------------------|----------------------------|
| `reviewer_spawn_gate` | `reviewer_spawn_gate` |
| `reviewer_manifest_json` | `reviewer_manifest` (JSON **array**) |
| `evidence_chain_json` | `evidence_chain` (JSON object) |

Cursor QA posts must include all three rendered keys. Parent merge gate and
correlation use **rendered** keys only. Missing `reviewer_spawn_gate` or
`reviewer_manifest` in a PR Machine Status block → QA **INCOMPLETE**.

### `evidence_chain` (mandatory on Cursor QA PR posts)

Populate `evidence_chain` on every Cursor QA report (FAIL, IN-FLIGHT, PASS).
The sc-compose input var is `evidence_chain_json`; the rendered key is
`evidence_chain`.

```json
{
  "qa_pass": "qa-c10-1",
  "commit": "<sha>",
  "pr_number": 39,
  "pr_comment_url": "<url from gh after post>",
  "coordinator_task_id": "<cursor-quality-mgr Task id>",
  "reviewer_tasks": [
    {"agent": "req-qa", "task_id": "<id>", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "PASS"}
  ],
  "triage": {
    "phase_id": "phase-C-web-server",
    "ttl_paths": [".triage/phase-C-web-server/findings/REQ-QA-001.ttl"],
    "qa_triage_task_ids": ["<id>"]
  },
  "parent_correlation_required": true
}
```

## Required message sequence

Every orchestration handoff follows codex ACK → Work → Completion → receiver ACK:

| Hop | Parties | Required messages |
|-----|---------|-------------------|
| Dev | parent → `rust-developer` | assignment → ACK → push+SHA → validation PASS/FAIL |
| Pre-QA RBP (optional dev fix) | parent → `rust-best-practices-agent` **or** parent → `rust-developer` fix pass | parent spawns RBP reviewer before QA-1; dev fixes findings if any |
| QA reviewers | parent → each reviewer | parallel background Tasks → fenced JSON per agent |
| QA enforcer | parent → `cursor-quality-mgr` | complete handoff → ACK → spawn-gate verify → aggregate → PR report → verdict |
| Triage | parent → `qa-triage` (per finding) | triage JSON → fenced JSON → `.ttl` path |
| Fix | parent → `rust-developer` | fix XML with finding ids + `.ttl` paths → push+SHA |

Parent must not spawn the next hop until the prior completion message exists.
Silent skips invalidate the evidence chain.

## Sprint flow

1. Render a dev assignment with `sc-compose` from
   `.cursor/skills/cursor-orchestration/dev-template.xml.j2`.
   Always include `sprint_doc` as authoritative scope.
2. Spawn Task `rust-developer` with the rendered assignment and planned model.
3. On push report (branch + SHA): open or update the PR targeting
   `integrate/phase-N` (or the sprint's `pr_target`).
4. **Before QA-1** (codex parity): parent may run a pre-QA `rust-best-practices-agent`
   Task on planned `review_targets` and route any findings to `rust-developer`
   for fix before the first full QA round. QA-1 still includes RBP in the
   required reviewer set — pre-QA sweep is optional dev cleanup, not a substitute.
5. Render QA assignment metadata with `sc-compose` from
   `.cursor/skills/cursor-orchestration/qa-template.xml.j2`
   with enforcer = **`cursor-quality-mgr`** (handoff block filled after step 7).
6. **Parent reviewer spawn** — follow **Parent reviewer spawn** above: render
   assignments, spawn all required reviewers in parallel, await fenced JSON,
   build `reviewer_manifest_json`.
7. Re-render or augment the QA assignment with `<parent-reviewer-handoff>`
   containing manifest + each reviewer's fenced JSON.
8. Spawn **one** `cursor-quality-mgr` enforcer with the completed handoff
   (see **Running cursor-quality-mgr**). Enforcer verifies spawn proof, parses
   fenced JSON, TODO-scans, checks CI, publishes PR findings report, returns
   verdict — **never** spawns reviewers.
9. On **spawn-gate FAIL** (`next_action: parent_respawn_reviewers`): spawn missing
   reviewers, rebuild handoff, re-run `cursor-quality-mgr` — do not merge.
10. On **finding FAIL** (any open finding or deliverable &lt; 100%): run
   `/triaging-findings`, then fix **all** findings via
   `.cursor/skills/cursor-orchestration/fix-assignment.xml.j2` →
   `rust-developer`, push, then parent re-spawns reviewers and re-runs
   `cursor-quality-mgr`. Repeat until QA PASS **and** CI green.
   Fix assignments must include:
   - authoritative `sprint_doc` = owning/promoted branch sprint plan (plus
     additional sprint docs when findings span multiple sprint origins)
   - specific requirement ids and ADR ids the fix must address (not whole
     requirements/architecture dumps; planning already embeds those in the
     sprint plan)
   - triage `.ttl` paths and concrete occurrences
   - **every** finding id from the FAIL round (all severities)
   Fresh `rust-developer` Tasks have no prior sprint memory — never omit
   these fields.
11. On **PASS + green CI + reviewer_spawn_gate pass**: merge to `pr_target`; then start the next sprint.

## Parity with codex-orchestration

Cursor orchestration is a **transport adapter** (parent + Task vs team-lead +
ATM). It must preserve codex gate semantics:

| Codex | Cursor equivalent |
|-------|-------------------|
| `team-lead` | parent session |
| `cwy` | `rust-developer` |
| `quality-mgr` | **`cursor-quality-mgr` only** (never ATM `quality-mgr`) — spawn/evidence enforcer |
| `codex-orchestration/*.j2` | `.cursor/skills/cursor-orchestration/*.j2` |
| Shared reviewers | same `.claude/agents/*` Task types — **parent spawns** |
| `quality-management-gh` reports | same templates + SKILL — **cursor-quality-mgr publishes** |
| Pre-QA-1 RBP (optional) | parent spawns `rust-best-practices-agent` before QA-1 |
| Reviewer spawn | **parent** Task spawns; `cursor-quality-mgr` enforces proof + aggregates |
| Multi-pass QA on PR | every round posts all findings to PR |
| 0B+0I+0m merge gate | unchanged |

**Cursor-only additions** (do not weaken codex):

- `reviewer_spawn_gate` + `reviewer_manifest` (rendered Machine Status keys) when
  Cursor QA runs (proves parent spawned reviewers and returned fenced JSON)
- Parent merge blocked without manifest proof (prevents coordinator self-review)
- Single QA coordinator per round (no duplicate `qa-c9-1` Tasks)

**Do not edit** `.claude/skills/codex-orchestration/` or
`.claude/agents/quality-mgr.md` from this skill. Shared report templates may
gain **optional** Cursor fields; codex callers omit them.

## Plan review flow

1. Complete `/plan-hardening` steps 1–5 when applicable.
2. **Parent reviewer spawn** — same as sprint QA: determine plan reviewers per
   `cursor-quality-mgr.md` Default reviewer set (`review_mode: plan`), spawn
   all required reviewers, collect fenced JSON, fill `<parent-reviewer-handoff>`.
3. Render QA assignment to **`cursor-quality-mgr`** with handoff; enforcer
   verifies spawn proof and publishes plan QA report.
4. Never also assign ATM `quality-mgr` for the same plan review.

## QA coverage rule

- Extract every deliverable, AC, deletion target, validation item, and artifact
  from `sprint_doc` before reviewers run.
- PASS requires 100% deliverable completion.
- `req-qa` owns completion percentage; `arch-qa` owns structural gates.

## Tool recipes (fenced)

All CLI examples use ambient auth. Do not add account, hostname, or login flags.

Skill template root (repo-root-relative):

`.cursor/skills/cursor-orchestration`

### Preconditions check

```bash
command -v sc-compose
command -v gh
command -v git
```

### Render dev assignment

```bash
_VARS=$(mktemp)
cat > "$_VARS" <<'JSON'
{
  "task_id": "dev-1",
  "sprint": "1a",
  "sprint_doc": "docs/plans/<sprint>.md",
  "description": "<summary>",
  "worktree_path": "<resolved-worktree-path>",
  "branch": "<branch>",
  "pr_target": "integrate/phase-N",
  "deliverables": "- <item>",
  "acceptance_criteria": "- <item>",
  "references": "- docs/requirements.md"
}
JSON
sc-compose render \
  --root .cursor/skills/cursor-orchestration \
  --file dev-template.xml.j2 \
  --var-file "$_VARS"
rm -f "$_VARS"
```

### Render QA assignment

Parent fills `reviewer_manifest_json` (JSON **array** of spawn records) and
`reviewer_handoff_json` (fenced JSON blocks) **after** spawning reviewers.

Cursor `$_VARS` for PR report render must always set non-empty `reviewer_spawn_gate`,
`reviewer_manifest_json`, and `evidence_chain_json` (codex callers may omit).

```bash
_VARS=$(mktemp)
cat > "$_VARS" <<'JSON'
{
  "task_id": "qa-1",
  "sprint": "1a",
  "sprint_doc": "docs/plans/<sprint>.md",
  "review_mode": "sprint_review",
  "description": "<summary>",
  "pr_number": "<pr>",
  "branch": "<branch>",
  "worktree_path": "<resolved-worktree-path>",
  "commits": "<sha>",
  "review_targets": "- <path>",
  "references": "- docs/requirements.md",
  "changed_files": "",
  "triage_records": "",
  "reviewer_manifest_json": "[{\"agent\":\"req-qa\",\"task_id\":\"<id>\",\"spawn_actor\":\"parent-orchestrator\"}]",
  "reviewer_handoff_json": "<fenced JSON blocks, one per manifest entry>"
}
JSON
sc-compose render \
  --root .cursor/skills/cursor-orchestration \
  --file qa-template.xml.j2 \
  --var-file "$_VARS"
rm -f "$_VARS"
```

### Render fix assignment

```bash
_VARS=$(mktemp)
cat > "$_VARS" <<'JSON'
{
  "task_id": "fix-1",
  "phase": "1",
  "sprint_doc": "docs/plans/<sprint>.md",
  "branch": "<branch>",
  "worktree_path": "<resolved-worktree-path>",
  "pr_target": "integrate/phase-N",
  "description": "<summary>",
  "finding_ids": "- <id>",
  "triage_records": "- .triage/<phase>/<finding>.ttl",
  "required_fixes": "- <fix>",
  "acceptance_criteria": "- <criterion>",
  "references": "- docs/requirements.md",
  "requirement_ids": "- REQ-0001",
  "adr_ids": "- ADR-0001"
}
JSON
sc-compose render \
  --root .cursor/skills/cursor-orchestration \
  --file fix-assignment.xml.j2 \
  --var-file "$_VARS"
rm -f "$_VARS"
```

### Render reviewer JSON (req-qa example)

```bash
_VARS=$(mktemp)
cat > "$_VARS" <<'JSON'
{
  "reference_docs": ["docs/requirements.md", "docs/architecture.md", "docs/plans/project-plan.md"],
  "sprint_doc": "docs/plans/<sprint>.md",
  "phase": "1",
  "sprint": "1a",
  "worktree_path": "<resolved-worktree-path>",
  "branch": "<branch>",
  "commit": "<sha>",
  "review_targets": ["<path>"],
  "round_limit": false,
  "changed_files": [],
  "carry_forward_findings_json": "[]",
  "triage_records": [],
  "notes": ""
}
JSON
sc-compose render \
  --root .cursor/skills/cursor-orchestration \
  --file req-qa-assignment.json.j2 \
  --var-file "$_VARS"
rm -f "$_VARS"
```

Render `arch-qa-assignment.json.j2` and `flaky-test-qa-assignment.json.j2` the
same way (see each template's `required_variables`). Rust reviewer templates:

```bash
sc-compose render \
  --root .claude/assets/sc-rust/quality-mgr/templates \
  --file rust-qa-assignment.json.j2 \
  --var-file "$_VARS"
```

(Use the var shapes in `.claude/assets/sc-rust/quality-mgr/quality-mgr.rust.md`.)

### CI status

```bash
gh pr checks <PR> --watch
gh pr view <PR> --json mergeStateStatus,reviewDecision
```

### PR findings / closeout (shared templates)

Required variable names come from the template frontmatter. Flat string map for
`sc-compose`. Blocking review example:

```bash
_VARS=$(mktemp)
# Fill every required_variables entry from
# .claude/skills/quality-management-gh/findings-report.md.j2
sc-compose render \
  --root .claude/skills/quality-management-gh \
  --file findings-report.md.j2 \
  --var-file "$_VARS" \
  | gh pr review <PR> --request-changes --body-file -
rm -f "$_VARS"
```

In-flight comment:

```bash
sc-compose render \
  --root .claude/skills/quality-management-gh \
  --file findings-report.md.j2 \
  --var-file "$_VARS" \
  | gh pr comment <PR> --body-file -
```

PASS closeout:

```bash
sc-compose render \
  --root .claude/skills/quality-management-gh \
  --file quality-report.md.j2 \
  --var-file "$_VARS" \
  | gh pr review <PR> --approve --body-file -
```

(Or `gh pr comment` if approval is not appropriate.)

## Templates (this skill)

- `dev-template.xml.j2`
- `fix-assignment.xml.j2`
- `qa-template.xml.j2`
- `review-template.xml.j2`
- `req-qa-assignment.json.j2`
- `arch-qa-assignment.json.j2`
- `flaky-test-qa-assignment.json.j2`
- `sprint-plan.md.j2`

Rust reviewer JSON templates remain shared at:

- `.claude/assets/sc-rust/quality-mgr/templates/`

## Isolation from Codex path

| Do | Do not |
|----|--------|
| Edit only under `.cursor/` for this skill | Edit `codex-orchestration` or `quality-mgr.md` |
| Spawn `cursor-quality-mgr` | Spawn `quality-mgr` while this skill is active |
| Reuse shared reviewers by Task type | Duplicate reviewer prompts under `.cursor/` |
| Remap stale `quality-mgr` mentions | Dual-launch both coordinators |
