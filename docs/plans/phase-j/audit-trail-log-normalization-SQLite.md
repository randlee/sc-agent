# Decider audit trail: log normalization and per-repo SQLite

Nov 24, 2026 · @Rand Lee

## Purpose and scope

Every decider tool (judge-jev, plan-anchor, sanity and later ones) writes one consistent audit trail into a per-repo SQLite database next to the session logs, and a reporting tool (HTML report, TUI, web UI) reads only that database. The work runs in three phases, in order: minor fixes, log clean-up for consistency, then the SQLite implementation.

This design covers sc-agent (Rust core and the sc-decidr wheel's client template), judge-jev and plan-anchor. It is written against the develop branches as of 2026-11-23: sc-agent `d2fcd3e`, judge-jev `0dd4719`, plan-anchor `b55d927`. It was not checked against the files on the owner's machine.

**Owner decisions this design rests on (2026-11-24):**

- The session log is written in Rust through sc-observability, which accepts events up to 1 MiB including the wrapper. The largest request that fits is recorded whole; a larger one gets no row and a `log.session_write` error. The session log is always on and cannot be turned off.
- One SQLite database per repo, in WAL mode: several tools write at once. Cross-repo queries are rare and serve tool evaluation only.
- Records are append-only and immutable; they change only during a schema upgrade or similar maintenance.
- Fully typed tables, with JSON columns for extension.
- judge-jev and plan-anchor application results move into SQLite, next to the sessions. Every report is an HTML report following the existing sc-compose report pattern.
- The general log stays JSONL for now.
- Every tool logs each run's start and end: session id, model, mode; the end shows pass or fail with an error code and the elapsed time.
- `run_id` is a ULID everywhere. The ULID carries the time, so the session row needs no start or end time.
- A batch result references its `session_id` and does not repeat the session's data. The shared result line holds generic fields only; tool-specific ids (plan-anchor `anchor_id` and `candidate_id`, judge-jev's record id) live in each tool's own records, which reference `session_id`. There is no `subject_id`.
- The result cache keys on the sha256 of the full request text sent to Jev and the requested model: jev, haiku and decider answer differently. The per-question `digest` stays a content-only hash (1.4).
- Tree walks are linked records, designed for arbitrary decision trees, at least when the tree is injected into the client.
- Extension tables can be added after release, and a tool may own several.
- Schema versions: sc-agent's core schema uses major versions; a tool's extension schema uses minor versions on top of the core major it builds on. A shared schema toolset (rules, checks, tests) keeps the tools consistent.
- A send with no project/phase keeps today's behavior: it is refused. There is no `default` project.

## Current state

Today decider data lands in three places with three conventions: a Rust-written session file, sc-observability general logs, and tool-specific files, some of them inside the repo.

### Where it lands

| Data | Writer | Path | Notes |
| --- | --- | --- | --- |
| Session log | Rust `sc_agent::send`, direct file append under a mutex (`crates/sc-agent/src/lib.rs` \~1160) | `~/.sc-agent/sessions/<repo-id>/<agent_role>/<project>/sessions.jsonl` | Not written through sc-observability; ADR-SAG-0021 says "Session rows are appended directly". `<SERVICE>_LOG_ROOT` overrides the root. |
| General log | sc-observability 1.5.0 | `~/.sc-agent/logs/<service>.log.jsonl` (+ `.1…N` rotation) | One file each for `decidr`, `sitr`, `judge-jev`, `plan-anchor`. |
| judge-jev results | `jev_judge.py` | Events in `judge-jev.log.jsonl`, target `judge-jev.batch` | Plus in-repo `.sc/judge-jev/fix/<ID>/` and `.sc/judge-jev/accepted.jsonl`. |
| plan-anchor results | `jev_judge.py`, `change_report.py` | In the repo: `docs/plans/phase-N/anchor-runs/<run_id>.json`, `anchor-runs/tournament-<run>.jsonl`, `anchor-runs/fixtures/`, tournament `--out` folders, `<name>.html` + `<name>.json` | plan-anchor writes nothing of its own to its general log. |

### What is logged in each general log

| Tool | Actions | Run start/end |
| --- | --- | --- |
| sc-decidr wheel (Rust) | `decision.result`, `error`, `sitr.languages`, `sitr.queries`, `sitr.query`, `sitr.run` | none |
| judge-jev | `request`, `answer`, `result` (per batch line); `statement`, `wave` (per tree node); `verdict` (per record walk); `compare`, `drift` (per command) | none |
| plan-anchor | only the wheel's actions | none |

### The data types

| # | Type (schema tag) | Owner | Key fields |
| --- | --- | --- | --- |
| 1 | Session row (no tag) | sc-agent | `session_id` (ULID), `repo`, `agent_role`, `project`, `request`, `question_digests{qid: sha256}`, `result{ok, value\|error}` |
| 2 | General log event (`v1`) | sc-observability | `timestamp`, `level`, `service`, `target`, `action`, `identity`, `fields` |
| 3 | `jev-batch-result/1` | shared client template, used by both tools | `request_id`, `run_id`, `item_id`, `anchor_id`, `candidate_id`, `repeat`, `request_sha256`, `questions_sha256`, `request`, `roles`, `kind`, `dry_run`, `ok`, `raw_response`, `answers`, `model_answered`, `sends`, `threshold`, `verdict`, `min_confidence`, `leaning`, `retry_count`, `attempts`, `error` |
| 4 | Tree-walk events | judge-jev | `statement` / `wave` / `verdict` with `run`, `tree_id`, `tree_sha256`, `node`, `answers`, `choices`, `reason`, `next`, `verdict`, `rule_rank`, `path` |
| 5 | Command summaries | judge-jev | `compare` (run, arms); `drift` (run, `old_index`, `new_index`, summary, per-record `id`/`verdict`/`min_confidence`) |
| 6 | Fix state | judge-jev | `state.jsonl` `{round, step, …}`; `accepted.jsonl` `{id, sha256, runs, lowest}` |
| 7 | `plan-anchoring/run/1` | plan-anchor | `run_id`, `gate`, `created_at`, `plan`, `item{}`, `model_requested`, `model_answered`, `threshold`, `dry_run`, `results[]` per anchor, `summary{counts, failing, unstable, errors, excluded, pass}` |
| 8 | Tournament records | plan-anchor | `tune-oracle/1`, `tune-candidates/1`, `results.jsonl` (type 3), ranking rows (`rank`, `bundle_id`, `roles`, `question_sha256`, `semantic_accepted`, `confidence`, `selected`, `reason`, `mismatches`) |
| 9 | `change-report/1`, `drift-review/1` | plan-anchor | report: `generated_at`, `plan`, `status`, `baseline_rev`, `commit`, `gate`, `items`, `anchors`, `drift_review`; findings: `id`, `severity`, `location`, `what_changed`, `authority`, `suggested_fix`, `status` |

### Problems found

- **Session log bypasses sc-observability.** The owner ruled it is written in Rust through sc-observability (1 MiB event limit). REQ-SAG-0003/0005 do not say so, and ADR-SAG-0021 says the opposite.
- **The answer union is not typed.** On success `result.value` is the parsed Jev answer keyed by question id, with `attempts`, `session_id` and `question_digests` added (lib.rs \~1153). The answer kind (noul, choice, score, box) is recovered only by joining back to `request.questions[qid].type`.
- **No model or provider in the session row.** The provider envelope, including token usage, is thrown away for the `messages` and `chat/completions` wires (`fn response`, lib.rs \~867).
- **The cache key ignores the model.** `digest` (lib.rs \~744) hashes the request with one question and nothing else.
- **`run_id` means three things:** judge-jev `sha256(request_id)[:16]`, plan-anchor `<gate>-<stamp>-<item>`, the new run start/end proposal a ULID.
- **Session row and app result have no shared key at top level.** `session_id` sits only inside `raw_response` on success, or inside the error on failure.
- **Batch results duplicate the session.** `request`, `raw_response`, `answers`, `model_answered`, `attempts` and `sends` are all repeated.
- **No run start/end anywhere,** so a failed or stalled run cannot be found from the logs.
- **Stale text in judge-jev.** `references/installation-and-troubleshooting.md.j2` still names `~/.judge-jev/<repository name>/`. The `wave_log_events` docstring blames "the logger's event size limit", but sc-observability 1.5.0 allows 1 MiB per event.

## Phase 1: minor fixes

Phase 1 fixes the existing logs in place, with no new storage: stale docs, the session log written through sc-observability, the missing model fields, and ULID run ids.

### 1.1 Stale judge-jev text (judge-jev, patch)

- `references/installation-and-troubleshooting.md.j2`: replace `~/.judge-jev/<repository name>/` with the real locations, `~/.sc-agent/logs/judge-jev.log.jsonl` and `~/.sc-agent/sessions/<repo-id>/judge-jev/<project>/`.
- `scripts/jev_judge.py.j2`, `wave_log_events` docstring: remove the claim that one event per wave exceeds the logger's size limit (sc-observability 1.5.0 allows 1 MiB). Keep the per-statement events: they are what the tree-walk records in Phase 2 build on.

### 1.2 Session log through sc-observability (sc-agent)

- Replace the hand-written append in `send` (`SESSION_APPEND` mutex, `std::fs::OpenOptions`, lib.rs \~1160–1190) with an sc-observability Logger that writes the session file.
- Path and behavior stay as REQ-SAG-0003/0005 state: `<log_root>/sessions/<repo-id>/<agent_role>/<project>/sessions.jsonl`, never rotated or pruned, appended by every process.
- The session log is always on; no setting turns it off. One query gives one row. A session event is at most 1 MiB including the wrapper: a request that fits is recorded whole, and a larger one gets no row and a `log.session_write` error. A cache hit (1.4) writes no row.
- Phase A's sprint a-8 already designed this: a session Logger with a registered `JsonlFileSink` that never rotates. The first step of the sprint confirms that sc-observability 1.5.0's public API supports a non-rotating sink at a caller-chosen path. If it does not, that change lands in sc-observability first.
- Records: amend ADR-SAG-0021 (remove "Session rows are appended directly…; no session Logger is injected"), add the sc-observability rule to REQ-SAG-0003 and REQ-SAG-0008, and the row and size rules to REQ-SAG-0005.

### 1.3 Model and provider in the session row (sc-agent)

- Add the schema tag `sc-agent-session/2`, `model_requested`, `provider` and `model_answered` to the row.
- Add `tool` and `tool_version` (the client name, or `decidr`), so every row traces back to the tool that sent it (REQ-SAG-0004).
- Keep the provider's token usage instead of discarding it in `fn response` for the `messages` and `chat/completions` wires: add `usage` with `input_tokens` and `output_tokens` as the provider reports them, null when it reports none.
- No start or end time: the `session_id` ULID carries the time.

### 1.4 Result cache keyed on the full request and the model (sc-agent)

- The per-question sha256 (`digest`, public) stays a hash of content only, so the same question can be compared across models.
- The result cache is the owner's design in the rulings below, specified by REQ-SAG-0703: a flag, default on, as a config key and a per-call `cache=` argument on `send` and `send_batch` (per job in a batch).
- Before each model call, a row in the same session log (same repo, `agent_role`, `project`) with the same sha256 of the full request text and the same requested model is a hit. `model_answered` is not known before the call, so it is not part of the key.
- A hit returns that row's result with `cached: true` and that row's `session_id`, makes no API call and writes no row. Batches are decided per item.
- Callers that resend an identical request on purpose (repeats, stability or variance checks, near-threshold re-asks) pass `cache=False`.

### 1.5 One meaning for run\_id (client template + both tools)

- `run_id` = a ULID minted once per tool run.
- `session_id` = the ULID of one query (unchanged).
- `request_id` = the consumer's deterministic key for a batch line (unchanged).
- A run is a batch (one result line per query) or a decision tree (many queries with tool processing between them). Its result and step records carry `run_id` and `session_id`; they are the link from a run to its queries (2.2).
- judge-jev: `run_id` is no longer `sha256(request_id)[:16]`. `--run-name` stays as a human label carried beside the ULID.
- plan-anchor: `run_id` is no longer `<gate>-<stamp>-<item>`. The run file is named `<run_id>.json` until Phase 3 moves it. Check `change_report.py` and `coverage` for any reliance on the old name's order or parts.

### 1.6 Batch result v2 (client template, both tools)

- `jev-batch-result/2` replaces `/1`.
- It carries `session_id` at top level and drops what the session row already holds: `request`, `raw_response`, `answers`, `model_answered`, `sends`, `attempts`, `retry_count`.
- It holds generic fields only: `schema`, `tool`, `tool_version`, `run_id`, `session_id`, `request_id`, `project`, `agent_role`, `threshold`, `verdict`, `min_confidence`, `leaning`, `ok`, `error`.
- Tool-specific fields (plan-anchor `item_id`, `anchor_id`, `candidate_id`, `repeat`, `roles`, `kind`; judge-jev's record id) move to each tool's own records, keyed by `run_id` and `request_id` and referencing `session_id`. No tool copies the request, raw response, answers, `model_answered`, attempts or sends into its records or its general log.
- `session_id` is known only when the send returns; no record needs it earlier.
- A dry run only checks that the input and arguments compose into a valid request and shows the would-send request (stdout or `--out`). It writes no session row, no result line, no general-log result event and no run or result record; `dry_run` leaves the result line.
- `result_line` in `decidr_client.py.j2` is the one place this changes. Each tool's `jev-batch-result.schema.json` follows.
- Readers that use the dropped fields read them from the session row by `session_id`. A helper for that lookup belongs in the client template, so the two tools don't each write one. Known readers: judge-jev jev\_runs.py (answers, \~234/249); plan-anchor anchor\_tuning\_contract.py (raw\_response, \~197) and the tournament in jev\_judge.py (answers vs raw\_response \~4259, raw\_response\_sha256 evidence \~4287).

## Phase 2: log clean-up for consistency

Phase 2 gives every tool the same record shapes while the logs are still JSONL, so Phase 3 maps them one-to-one onto tables.

### 2.1 Standard header on every application record

Every record a tool writes to its general log carries the same leading fields:

| Field | Meaning |
| --- | --- |
| `schema` | the record's schema tag, e.g. `jev-batch-result/2` |
| `tool` | `judge-jev`, `plan-anchor`, `sanity`, … (the client's `CLIENT_NAME`) |
| `tool_version` | the tool's released version |
| `run_id` | the run's ULID (1.5) |
| `session_id` | the query's ULID, when the record is tied to one query; absent otherwise |
| `project`, `agent_role` | as in the session row |

No timestamp field: the general-log event already has one, and the ULIDs order everything else. `repo` is in the session row and becomes implicit in Phase 3's per-repo database.

### 2.2 Run start and end (Rust, exposed through the client template)

- Rust gains `run_start` and `run_end` in sc-agent.
- `run_start(tool, mode, model, project, agent_role)` returns a ULID `run_id` and writes a `run.start` general-log event. `mode` is the tool's subcommand or flow (`batch`, `quality`, `tournament`, `judge`, …).
- `run_end(run_id, outcome)` writes `run.end`: `ok`, the error code when not ok, `elapsed_ms`.
- Every `send` / `send_batch` takes `run_id`, and the session row carries the `run_id` of the run that created it. A run does not find its sessions through that field: a cache hit returns an older row created by another run. The run's result lines and walk steps, each carrying `run_id` and `session_id`, are the link; the run records do not repeat session data.
- The client template wraps this as one call: `with client.run(mode=…, model=…) as run:`. A run that raises or is cancelled still writes `run.end` with its error code (`CANCELED` included).
- judge-jev and plan-anchor call it once per subcommand that sends to a model.

### 2.3 Typed answers in the session row (sc-agent)

The row's result becomes three fields:

- `ok`: true or false.
- `answers`: on success, one entry per question: `question_id`, `kind` and that kind's typed fields:
  - `noul`: `noul`
  - `choice`: `choice`, `confidence`, `probabilities`
  - `score`: `score`, `confidence`, `legend`, `probabilities`
  - `box`: `top`, `bottom`, `left`, `right`, `confidence`
- `error`: on failure, `id`, `category`, `message`, `action`, `attempts`.

Rust builds these from the same per-kind structs it uses for the TypeSafe API (owner ruling: answer shapes are part of the typed API; consumers do not parse them). The template's `NoulAnswer`, `ChoiceAnswer`, `ScoreAnswer` and `BoxAnswer` read them back.

### 2.4 Tree walks as linked records (client template)

Tree-walk events move from judge-jev's `jev_judge.py` into the client template, so any tool walking any injected tree writes the same records:

- `walk.start`: `walk_id` (ULID), `run_id`, `tree_id`, `tree_sha256`. The tool's own record links the walk to what it judged (judge-jev's record id).
- `walk.step`, one per node visited: `walk_id`, `seq`, `prev_seq`, `node_id`, the `session_id`s it sent (one per statement in a per-statement wave), `matched_rule`, `next_node`, and `verdict` when the walk ends there.
- `walk.end`: `walk_id`, `verdict`, `rule_rank`, the node path.

The tree itself is stored once per `tree_sha256` (Phase 3), so a step names its node and never copies the tree. The template takes the tree as data and the tool supplies the branch-selection callback. A generic walker in Rust can replace the callback later without changing these records.

### 2.5 Same records from both tools

- plan-anchor logs its batch results to its general log as judge-jev does, through one template function (`log_result_line` moves into `decidr_client.py.j2`).
- plan-anchor's `tournament-<run>.jsonl` and run files stay where they are until Phase 3 moves them. Their `results[]` drop `answers`, `model_answered` and `attempts` and reference `session_id` (j-3).
- judge-jev's `compare` and `drift` events and plan-anchor's run summary use the 2.1 header.

## Phase 3: per-repo SQLite

Each repo gets one append-only database, `~/.sc-agent/sessions/<repo-id>/audit.sqlite`, next to that repo's session files. sc-agent owns its core tables, and each tool adds its own extension tables.

### 3.1 Location and access

- **One file per repo**, keyed by the same `<repo-id>` as the session tree (`github.com/randlee/judge-jev`, or `local/<host>/<dir>` with no remote). `agent_role` and `project` become columns, so a repo/project view needs only that one file.
- **Cross-repo evaluation** opens several files with `ATTACH`. A small read-only script does it; nothing routine depends on it.
- **Concurrent writers:** WAL mode, a busy timeout, and every write in a `BEGIN IMMEDIATE` transaction, so judge-jev, sanity and plan-anchor can append at once.
- **Immutable rows:** every table gets `BEFORE UPDATE` and `BEFORE DELETE` triggers that abort. A schema migration is the only code that drops and re-creates them, inside its own transaction.
- **One writer path:** Rust only. The wheel exposes `audit.insert(table, row)`; a tool never opens the file for writing itself. Reports and the TUI/web UI open it read-only.
- **Dual write first:** sessions go to both JSONL and SQLite until the SQLite path is verified. A one-shot importer loads existing `sessions.jsonl`, general-log app events and plan-anchor run files.

### 3.2 Core tables (sc-agent schema 1.0)

| Table | Key | Columns | From |
| --- | --- | --- | --- |
| `schema_versions` | `(owner, major, minor)` | `ddl_sha256`, `applied_ulid` | Schema versioning |
| `run_starts` | `run_id` | `tool`, `tool_version`, `mode`, `model`, `project`, `agent_role` | 2.2 |
| `run_ends` | `run_id` → `run_starts` | `ok`, `error_code`, `elapsed_ms` | 2.2 |
| `sessions` | `session_id` | `run_id`, `tool`, `tool_version`, `request_sha256`, `project`, `agent_role`, `model_requested`, `provider`, `model_answered`, `ok`, `error_id`, `error_category`, `error_message`, `attempts`, `usage_input_tokens`, `usage_output_tokens`, `request` (JSON) | 1.3, 2.3 |
| `session_questions` | `(session_id, question_id)` | `kind`, `question_sha256`, `model_answered` | per-question comparison across models |
| `session_answers` | `(session_id, question_id)` | `kind`, `noul`, `choice`, `confidence`, `score`, `top`, `bottom`, `left`, `right`, `probabilities` (JSON), `legend` (JSON) | 2.3; one CHECK per kind |
| `batch_results` | `(run_id, request_id)` | `session_id`, `tool`, `tool_version`, `project`, `agent_role`, `threshold`, `verdict`, `min_confidence`, `leaning`, `ok`, `error_code` | 1.6 |
| `trees` | `tree_sha256` | `tree_id`, `kind`, `definition` (JSON or text, as given) | 2.4 |
| `walks` | `walk_id` | `run_id`, `tree_sha256`, `verdict`, `rule_rank` | 2.4 |
| `walk_steps` | `(walk_id, seq)` | `prev_seq`, `node_id`, `matched_rule`, `next_node`, `verdict` | 2.4 |
| `step_sessions` | `(walk_id, seq, session_id)` | `statement_index` | 2.4 |

- The result cache (1.4) is one indexed read on `sessions (request_sha256, model_requested)`. `session_questions` copies `model_answered` from its session for per-question comparison; the copy is safe because rows never change.
- Every core table has an `ext` JSON column, null by default, for data a minor version has not yet given a typed column.
- `batch_results`, the tree tables and the run tables are core because the client template writes them for every tool.

### 3.3 Extension tables

- A tool registers its tables under its own prefix, `<tool>__<name>`, so a tool can own any number of them. A tool never alters a core table or another tool's.
- Extension rows carry the 2.1 header as columns (`run_id`, `session_id` when tied to one query) with foreign keys into the core tables, plus typed columns and an `ext` JSON column.
- Tools can add extensions after release: a new tool version ships its migrations and the database applies them the next time that tool opens it (see Schema versioning).

Tool-specific fields of a batch line (plan-anchor `item_id`, `anchor_id`, `candidate_id`, `repeat`, `roles`, `kind`; judge-jev's record id) go in a tool table keyed by `(run_id, request_id)` that references `session_id`.

First extensions, one table per record type in Current state:

| Tool | Table | Holds (type #) |
| --- | --- | --- |
| judge-jev | `judge_jev__compares` | compare summaries (5) |
| judge-jev | `judge_jev__drifts`, `judge_jev__drift_records` | drift summary and per-record verdicts (5) |
| judge-jev | `judge_jev__fix_steps` | `state.jsonl` lines (6) |
| judge-jev | `judge_jev__accepted` | `accepted.jsonl` lines (6) |
| plan-anchor | `plan_anchor__runs` | run file header and summary (7) |
| plan-anchor | `plan_anchor__anchor_results` | one row per anchor per run (7) |
| plan-anchor | `plan_anchor__oracles`, `plan_anchor__candidates` | `tune-oracle/1`, `tune-candidates/1` (8) |
| plan-anchor | `plan_anchor__rankings` | tournament ranking rows (8) |
| plan-anchor | `plan_anchor__change_reports`, `plan_anchor__drift_findings` | `change-report/1`, `drift-review/1` (9) |

### 3.4 Moving judge-jev and plan-anchor results

- judge-jev: the fix workspace's `state.jsonl` and `accepted.jsonl` move to their tables. The candidate `.md` copies and Raptor indexes are working files, not results; they stay in `.sc/judge-jev/fix/<ID>/`.
- plan-anchor: `anchor-runs/` stops being written. `report`, `coverage`, `change_report.py` and the tournament's `--results`/`--choose` read the database by `run_id`. `anchors.json` and `anchors.md` are the plan's source, not logs, and stay in the repo.
- The plan-anchor README line "Every run is stored in `anchor-runs/`" and the docs that send readers there are updated in the same change.

## Schema versioning

sc-agent's core schema carries the major version (1.0, 2.0, …). A tool's schema is `<core major>.<minor>`: plan-anchor 1.0 and 1.1 both run on sc-agent 1.0, and plan-anchor 2.0 and 2.1 on sc-agent 2.0. A minor version adds and never breaks, so each tool upgrades on its own.

### Rules

| Change | Allowed in |
| --- | --- |
| New table, new index, new view | tool minor |
| New nullable column, or a column with a constant default | tool minor |
| Data stored in a tool's `ext` JSON column | any version, no migration |
| Rename, drop, retype a column; change a key; change a core table | core major only, with every tool shipping `<new major>.0` |
| UPDATE or DELETE of existing rows | migration code only, never at run time |

### What happens when a tool opens the database

The tool declares its owner name, its `major.minor`, and its ordered migration files. Rust then checks the database's `schema_versions` rows:

1. Core major in the database ≠ the tool's major: refused with a typed error naming both versions and the upgrade to run. Nothing is written.
2. Tool minor in the database < the tool's minor: the missing migrations are applied in order, each in one transaction with its `schema_versions` row.
3. Tool minor in the database > the tool's minor (a newer copy of the tool already upgraded it): opened as is. The tool appends with the columns it knows; the newer columns stay null for its rows.
4. Equal: opened.

### The shared toolset

sc-agent ships one toolset that every tool uses, so the rules above are checked mechanically and not by review.

- **Migration layout:** `<owner>-<major>.<minor>.sql` files, one per version, in the tool's package data. A lint check enforces the naming and an unbroken sequence.
- **DDL lint:** a minor migration may contain only the statements the rules table allows. Every table it creates has the `<owner>__` prefix, the header columns (`run_id`, and `session_id` when the row is per query), an `ext` JSON column, and the immutability triggers.
- **Compatibility tests:** a reusable test, Rust and Python, that every tool runs in its CI:
  - Build a database at every released version of the tool and of core, then upgrade it to the current version and check the result.
  - Open the current database with the previous release and append a row, which must succeed. This is case 3 above.
  - Check that core-major mismatch is refused.
- **Fixture databases:** each release commits a small fixture database at that version, so the upgrade tests always run against what was actually shipped.

The toolset lives in sc-agent and reaches tools through the wheel and the client template, like the rest of the integration.

## HTML reports

Every report is an HTML page rendered by sc-compose from a Jinja template, reading only the audit database. This is the pattern plan-anchor's `change_report.py` already uses.

### The pattern

`change_report.py` (`render`, \~637) does three things:

1. Writes the template's variables to a JSON var file in a scratch folder.
2. Runs `sc-compose render --root <templates> --file <name>.html.j2 --var-file <vars> --strict --check-render --output <page>`, with sc-compose's own log kept in the scratch folder.
3. Writes a JSON sidecar of the same model beside the page.

Each report below follows that shape. Its variables come from read-only SQL views over the database, not from loose files. Every report keeps its JSON sidecar, so an agent reads the same data the page shows.

### Reports

| Report | Owner | Template | Reads | Today |
| --- | --- | --- | --- | --- |
| Runs and sessions: runs by tool and mode, failures by error code, model and token use, cache hits | sc-agent (wheel template) | `runs.html.j2` | `run_starts`, `run_ends`, `sessions`, `session_questions` | none |
| Quality run: per record, the tree walked, each wave's answers and verdict | judge-jev | `quality-report.html.j2` | `walks`, `walk_steps`, `step_sessions`, `session_answers`, `batch_results` | JSON on stdout |
| Compare and drift | judge-jev | `compare-report.html.j2`, `drift-report.html.j2` | `judge_jev__compares`, `judge_jev__drifts`, `judge_jev__drift_records` | JSON on stdout |
| Fix progress: rounds, candidates, accepted records | judge-jev | `fix-report.html.j2` | `judge_jev__fix_steps`, `judge_jev__accepted` | files in `.sc/judge-jev/` |
| Gate run and coverage | plan-anchor | `run-report.html.j2`, `coverage-report.html.j2` | `plan_anchor__runs`, `plan_anchor__anchor_results`, `batch_results` | `report` / `coverage --format md` |
| Tournament | plan-anchor | `tournament-report.html.j2` | `plan_anchor__oracles`, `plan_anchor__candidates`, `plan_anchor__rankings`, `batch_results` | JSON in the `--out` folder |
| Change report (mandatory for the owner) | plan-anchor | `change-report.html.j2` (exists) | `plan_anchor__change_reports`, `plan_anchor__drift_findings`, `plan_anchor__runs` | renders from `anchor-runs/` |

- Each tool's templates ship in its own templates folder; the runs report ships in the sc-decidr wheel's templates, next to the client template.
- A page is written where the command's `--out` says, the same as the change report today. The database stays the record; a page can always be rendered again from it.

## Records, sprints and open items

The work is 13 sprints in phase order. The REQ/ADR changes land on develop before the sprint that needs them, and the release rule still holds: nothing merges to develop or gets a full publish until sc-agent, judge-jev and plan-anchor are all complete.

### Record changes

Phase j runs the Phase 1 records as sprints j-1 (sc-agent), j-2 (client template), j-3 (plan-anchor) and j-4 (judge-jev). `docs/plans/phase-j/records-audit.md` lists each conflicting record line by line.

| Record | Change | Sprint | Phase j |
| --- | --- | --- | --- |
| ADR-SAG-0021 | Session rows go through an sc-observability Logger; always on; oversize or failed write gives no row and `log.session_write`; add run start/end and the ULID `run_id`; drop "appended directly" | S2, S4 | j-1 |
| REQ-SAG-0003 | The session log is written in Rust through sc-observability, by a sink that never rotates or prunes | S2 | j-1 |
| REQ-SAG-0004 | Every row carries `tool` and `tool_version` | S2 | j-1 |
| REQ-SAG-0005 | Session row fields: schema tag, model, provider, usage, `run_id`, typed answers; one row per query; 1 MiB rule; no row on a cache hit; later the database path | S2, S5, S8 | j-1 |
| REQ-SAG-0008 | Session rows written through sc-agent's own session Logger, not "appended directly" | S2 | j-1 |
| REQ-SAG-0202 | Caching points to REQ-SAG-0703; the digest stays content-only | S2 | j-1 |
| REQ-SAG-0701 | A cache hit returns the holding row's `session_id` | S2 | j-1 |
| new REQ-SAG-0703 | Result cache (1.4) | S2 | j-1 |
| REQ-SAG-0018 | Template: run context, `jev-batch-result/2` generic fields, dry-run rule, `cache` pass-through, session lookup helper, tree-walk records, `audit.insert` | S3, S4, S6, S9 | j-2 |
| plan-anchor REQ-010, REQ-013, REQ-017, REQ-038, REQ-043, REQ-076, ADR-010, ADR-011, skill references | Results reference `session_id`; repeats and confirmation pass `cache=False`; dry runs write no run file; stand-in queries go through sc-decidr | S3 | j-3 |
| judge-jev REQ-JEV-0002, 0008, 0101, 0208, 0211, ADR-JEV-0001, 0007, skill references | `jev-batch-result/2`; log result events with `session_id`, not requests or answers; dry run writes nothing; drop the 65536-byte rule | S1, S3 | j-4 |
| sc-agent `sc-sanity-jev` agent and `jev_client.py` | Call Jev through the client template; keep only `session_id` and the derived choice | S3 | none yet (closest j-2) |
| new ADR | Per-repo audit database: location, WAL, immutability, Rust as the one writer | S7 | later phase |
| new ADR | Schema versioning: core major, tool minor, open-time checks | S7 | later phase |
| new REQ | Each report is rendered by sc-compose from the database | S13 | later phase |
| plan-anchor README, references | `anchor-runs/` replaced by the database | S11 | later phase |

### Sprints

**Phase 1: minor fixes**

1. **S1** judge-jev stale text (1.1). judge-jev only.
2. **S2** Session log through sc-observability, plus model, provider and usage in the row (1.2, 1.3, 1.4). sc-agent; sc-observability first if its API needs the non-rotating sink.
3. **S3** ULID `run_id`, `jev-batch-result/2` with tool ids moved to tool records, dry-run rule, session lookup helper (1.5, 1.6). Template, then both tools.

**Phase 2: log clean-up**

4. **S4** `run_start` / `run_end` in Rust and the template's run context, called by both tools (2.2). Needs S3.
5. **S5** Typed answers in the session row (2.3). sc-agent and template.
6. **S6** Standard header, tree-walk records moved into the template, plan-anchor batch-result logging (2.1, 2.4, 2.5). Needs S4.

**Phase 3: SQLite**

7. **S7** Schema toolset (rules, lint, compatibility tests) and the database layer: open, WAL, triggers, `schema_versions`.
8. **S8** Core 1.0 sessions tables, dual-written with the JSONL session log. Needs S5, S7.
9. **S9** Core runs, `batch_results` and tree tables, written by the template. Needs S6, S8.
10. **S10** judge-jev extension 1.0 and its writers.
11. **S11** plan-anchor extension 1.0; `report`, `coverage`, the tournament and `change_report.py` read the database; `anchor-runs/` retired.
12. **S12** Importer for existing session JSONL, general-log app events and run files.
13. **S13** HTML reports, one per row of the reports table.

S10 and S11 can run in parallel after S9. Sprint N names sprint N−1's output as a required input through a child bead, per the handoff pattern.

### Open items for the owner

- After the dual-write period, does the JSONL session log stay as a second copy, or is SQLite the only session record?
- Should the run start record carry a session id? As designed, a run's result and step records carry its `session_id`s, because a run's sessions do not exist yet when it starts and a cache hit returns an older session.
- Does a purely additive core change (a new core table) stay a core major bump, as written, or may core also take minor versions?
- Should the importer (S12) load all history, or only what the reports need?

## Owner rulings, 2026-11-24 (after review)

- Both 1.1 items (the stale `~/.judge-jev/` path at `installation-and-troubleshooting.md.j2:49` and the `wave_log_events` docstring) are folded into phase j's judge-jev sprint.
- The session log is never rotated or pruned: it does not grow too large because `project` changes over time.
- The 2.1 standard header lands in `jev-batch-result/2` (one schema bump, not two).
- Result cache: a flag, default on. Before each model call, if the sha256 of the full text sent to Jev and the model already have a session row, that row's result is returned and no API call is made. It applies per item for batches and streamed input: if items 2 and 5 of 10 changed, only 2 API queries are made.
- No concurrent-writer tests: a test that SQLite is configured correctly (WAL, busy timeout, `BEGIN IMMEDIATE`) is enough.
- Result cache detail: the flag is a config key and a per-call `cache=` argument on `send` and `send_batch` (per job in a batch). The key is the full-request sha256 and the requested model, within the same session log. A hit returns `cached: true` and the existing row's `session_id`, and writes no row. Intentional resends pass `cache=False`.
- The shared result line holds generic fields only; tool ids live in tool records that reference `session_id`. No `subject_id`.
- `session_id` is known only after the send returns, and a cache hit returns an older one, so a run's result and step records hold the link from the run to its queries.
- A dry run checks that the input composes into a valid request and shows it; it writes nothing to the session log, the general log as results, or any run or result store.
- The 65536-byte figure in judge-jev is an sc-observability wheel limit, not a judge-jev rule.
