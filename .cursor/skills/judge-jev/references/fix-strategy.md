---
name: fix-strategy
version: 0.2.0
description: How to fix a REQ, NFR or ADR record that fails Jev or passes below the confidence gate - ask Jev why with the diagnostic tree, pick strategies from the table, run one proposer per strategy in parallel, measure, accept and freeze. Rules, stop conditions, commit and report format.
---

# Fix strategy

Jev's answers say what is wrong. Ask Jev why before any edit; never reason about why. A low-confidence answer is unclear wording, not noise.

- **Gate.** Judge the record three times on identical text and take the middle (median) confidence of each gate answer. A record passes when its verdict is `yes` in at least two runs and every gate answer's median is at or above `T`. `T` is 0.6 unless the repository states another value. One run is not a pass: a score near `T` moves a few points between identical runs.
- **Unit of work.** One record. Jev judges each statement with the whole record as context, so an edit to one statement changes the input for all the others. Never give two agents the same record. Records are independent: run as many in parallel as you like.
- **State.** `$W/state.jsonl` is your memory. Read its last line before every step and append one line after it. Do not track rounds, candidates or results in your head.

## 0. Set up, once per record

```bash
ID=REQ-ABC-0001; KIND=req; T=0.6   # KIND is req, nfr or adr
F=docs/requirements/abc.md         # the file that holds the record
W=.sc/judge-jev/fix/$ID; mkdir -p "$W"
cp "$F" "$W/original.md"
D="<this skill's directory>/assets/questions/diag-$KIND.json"
G="<this skill's directory>/scripts/jev_gate.py"
printf 'J=%s\nRAPTOR=%s\nID=%s\nKIND=%s\nF=%s\nW=%s\nT=%s\nD=%s\nG=%s\n' "$J" "$RAPTOR" "$ID" "$KIND" "$F" "$W" "$T" "$D" "$G" > "$W/env"
```

Start every later command with `. .sc/judge-jev/fix/<ID>/env`.

Two commands do all the measuring. Each is one step: paste it whole.

`verify <v>`: snapshot the file as variant `<v>`, judge it three times with the shipped tree, print the gate.

```bash
cp "$F" "$W/<v>.md"
"$RAPTOR/.venv/bin/python" "$RAPTOR/scripts/extract.py" --project-root . --output "$W/index-<v>.json" | jq '.issues'   # must print 0
for r in 1 2 3; do uv run -q --script "$J" quality --$KIND "$ID" --index "$W/index-<v>.json" --run-name "$ID-<v>-r$r" > "$W/<v>-r$r.json" & done; wait
python3 "$G" --threshold "$T" "$W"/<v>-r1.json "$W"/<v>-r2.json "$W"/<v>-r3.json | jq -r 'if .success | not then "ERROR \(.error.code): \(.error.message)" else .data.records[] | "\(.id) pass=\(.pass) verdicts=\(.verdicts | join(",")) lowest=\(.lowest.median // "n/a")", (.low[] | "  s\(.statement // "-") \(.question)=\(.choice) median \(.median) \(.confidences)" + (if .runner_up then "\n      nearly: \(.runner_up.choice) (\(.runner_up.probability)) \(.runner_up.criterion)" else "" end)) end'
```

It prints one line for the record and, for each gate answer whose median is below `T`, the answer and the option Jev nearly chose instead (`nearly:`), with that option's probability and its criterion. Read `nearly:` first: a low confidence is Jev torn between its choice and that option, so it says what the statement is being read as, and its criterion often names the fix (`obligations=1` nearly `2`: the statement reads as two obligations; `check=inspection or grep` nearly `CLI run`: it does not say whether a file or a run decides it). `pass=true` is the only pass. `ERROR` is a tool error, not a verdict: rerun once, then report the code and message verbatim.

`diagnose <v>`: ask Jev why, once, for the variant `verify` just snapshotted.

```bash
uv run -q --script "$J" quality --$KIND "$ID" --index "$W/index-<v>.json" --questions "$D" --run-name "$ID-<v>-diag" > "$W/<v>-diag.json"
jq -r 'if .success | not then "ERROR \(.error.code): \(.error.message)" else .data.results[] | .waves[] | .reason[] | if has("statement") then "s\(.statement.index): \(.statement.text)", (.answers[] | "    \(.id)=\(.choice) @\(.confidence)") else "  \(.id)=\(.choice) @\(.confidence)" end end' "$W/<v>-diag.json"
```

`diag-<kind>.json` is the shipped gate tree with `d_*` diagnostic questions appended. Its gate questions and verdict rules are the shipped ones. Run it when `nearly:` does not settle the cause, and read the `d_*` answers of the statements `verify` listed. Every answer in a `quality` result also carries `probabilities` (every option) and `runner_up`. The gate itself is always `verify`, never this run.

## 1. Verify, then diagnose

`verify base`. If it prints `pass=true`, go to step 5. Otherwise read each `nearly:` line, `diagnose base` where the cause is still open, and append `{"round": n, "step": "diagnose", "low": [...]}` to `$W/state.jsonl`.

If a cause is still unclear, copy `$D`, add up to 10 choice questions of your own under `.nodes["wave-1"].questions` (same shape as the `d_*` ones), and run the `diagnose` command with `--questions <copy>`. Ask them all in one call. A question costs a fraction of a cent; a minute of your reasoning costs more than a thousand of them. Never edit a gate question, a criterion or a verdict rule.

## 2. Pick strategies

For each weak statement, take every row whose trigger matches, three at most.

| Strategy | Trigger: gate answer weak or failing, with these `d_*` answers | The one edit |
| --- | --- | --- |
| `name-referent` | `term`; `d_term_where` or `d_ambiguous` = `reference`, or `d_term_fix` = `replace reference` | Replace the back-reference ("that type", "as follows", "it", "the same") with the full noun phrase or the named table. |
| `define-term` | `term` = `project term defined nowhere` or `undefined acronym`; `d_term_fix` = `define in record` | One sentence in the record's `Definitions:` paragraph (first paragraph under `### Requirement Statement`; create it if absent), built from facts already in the record or the repository. Only the term Jev blames. Jev sees only this record. |
| `plain-words` | `term`; `d_term_fix` = `plain words`, or `d_term_where` = `qualifier` | Replace the jargon or qualifier with plain words that say the same thing. A vague qualifier takes the number the code or tests use; no number in the repository means unresolved. |
| `observable` | `check`; `d_check_kind` = `runtime` or `unclear`; `d_check_why` = `observable` or `trigger` | State what can be observed in place of an internal property: "use the base URL" becomes "each request goes to the URL ...". |
| `inspect-target` | `check`; `d_check_kind` = `static`; `d_check_why` = `location` or `test kind` | Name the file or command to inspect and what it shows. Do not recast the statement as behaviour: that adds a condition and reads as two obligations. Never write "checked by ...". |
| `split` | `obligations` 2+ with `and-list`; `d_obligations_why` = `list` | One statement per item, same subject and modal, every item and gloss kept. |
| `collapse-to-term` | Same as `split`, when the items are the parts of one named thing (a call chain, a field set) | Define the thing once, listing every part, and require the thing. |
| `one-choice` | `joined_by` = `alternatives` | Define the set and require exactly one member. |
| `move-rationale` | `joined_by` = `obligation+rationale`; `d_obligations_why` = `purpose` | Move the reason to `### Rationale`. |
| `lift-condition` | `d_obligations_why` = `condition`, `ordering` or `default` | Lead with the condition ("When ...,"), or state the default and its exception as two statements. |
| `siblings` | Two or more near-identical statements in the record are weak on the same answer | One candidate that edits all of them the same way. Fixing one alone makes the others fail. |
| `adr-context` | ADR `context`; `d_ctx_gap` | One sentence starting "The problem is ...", from the record's own Decision and Rationale. |
| `adr-alternatives` | ADR `alternatives`; `d_alt_gap` | `#### Alternative <n>: <name>` with `**Description:**` and `**Why Rejected:**`, taken from the record or other records, citing the id that rules it out. |
| `adr-consequences` | ADR `consequences`; `d_cons_gap`, `d_cons_weak` | The cost the record or the code already shows. |

Fix `term` answers first and re-diagnose before the others: structure answers often settle once the terms are clear.

Do not edit for these known misreads; list them: a lowercase or hyphenated product name read as `undefined acronym` once it is defined; `undefined acronym` on a statement with no acronym; `project term defined nowhere` near 0.3 for a term this record defines.

## 3. Propose, in parallel

Launch one `jev-fix-proposer` per statement and strategy, all in one message, with no shared context. Each gets one fenced JSON object:

```json
{
  "record_id": "REQ-ABC-0001",
  "kind": "req",
  "file": "docs/requirements/abc.md",
  "strategy": "name-referent",
  "record_text": "<the record as it is now>",
  "original_text": "<the record as its author wrote it>",
  "statement": {"index": 3, "text": "<statement text>"},
  "answers": "term=none median 0.41, nearly project term defined nowhere (0.38); check=CLI run @0.94; d_term_where=reference @0.83; d_term_fix=replace reference @0.80",
  "blocked": ["<an edit rejected in an earlier round, with the reason>"],
  "sources": ["docs/", "src/"]
}
```

Each returns one candidate. If no agent can be launched, write the candidates yourself, one per strategy. Append each candidate to `$W/state.jsonl` as `{"round", "step": "candidate", "id", "statement", "strategy", "old", "new"}`.

## 4. Measure

Reject before measuring: a candidate whose `old` does not occur exactly once in `$F`, or whose edited text fails this guard against the original.

```bash
python3 - "$W/original.md" "<edited file>" <<'PY'
import re, sys
from collections import Counter
pats = {"code span": r"`[^`\n]+`", "number": r"(?<![\w.])\d+(?:\.\d+)?(?![\w.])",
        "record id": r"\b[A-Z]+-[A-Z0-9]+-\d+\b", "MUST": r"\bMUST(?! NOT)\b", "MUST NOT": r"\bMUST NOT\b"}
a, b = (open(p, encoding="utf-8").read() for p in sys.argv[1:3])
lost = {k: sorted((Counter(re.findall(p, a)) - Counter(re.findall(p, b))).elements()) for k, p in pats.items()}
lost = {k: v for k, v in lost.items() if v}
print(f"REJECT, lost: {lost}" if lost else "guard ok")
PY
```

For each remaining candidate `c<n>`: apply it to `$F`, `verify c<n>`, then `cp "$W/base.md" "$F"`.

For each statement keep the candidate that lifts its lowest median highest without pushing any other answer in the record below `T`. Apply the kept candidates together and `verify comb`. If `comb` is worse than its parts, drop the weakest candidate and verify again. Append `{"round", "step": "measure", "kept": [...], "lowest": x}`. If the record improved, `cp "$W/comb.md" "$W/base.md"`: the next round starts from it.

## 5. Accept and freeze

A record is accepted when `verify` prints `pass=true`. Otherwise start the next round at step 1 on the current text, with the rejected edits in `blocked`.

When a record is accepted, run the guard on `$W/original.md` against `$F` and read `diff -u "$W/original.md" "$F"` against rule 1. Then append `{"id", "sha256": "<of the record text>", "runs": [...], "lowest": x}` to `.sc/judge-jev/accepted.jsonl`. An accepted record is frozen: do not judge it again unless its text changes. Re-judging unchanged text only finds chance failures.

Three rounds at most. A statement still below `T` after round 3 is unresolved: keep the best text reached and list it.

## Rules

1. **Obligations do not change.** No obligation is added, dropped, narrowed, widened or made stricter, and no rationale becomes a MUST. A fix that needs any of these is not applied: list it for the owner with both texts.
2. **Nothing is invented.** Every number, definition and alternative comes from the record or the repository. Cite the file and line in your report, never in the record.
3. **A statement never names how it is checked.**
4. **The gate is not edited.** Diagnostic questions are appended; gate questions, criteria, verdict rules and `T` stay as shipped.
5. **No record is deleted, and no statement is removed, to make a record pass.**

Do not ask the owner how to word something; ask Jev. Ask the owner only for a fact the repository does not hold.

## Many records

One agent per record, each in its own git worktree, each running this page for that record only. Give each the record id, the worktree path and this page; nothing else. An agent commits in its worktree and returns the commit id; it does not push. The lead cherry-picks the returned commits and pushes once. Give one lead 10 to 15 records at a time, then start a fresh lead.

## Which model does what

Split the work by whether it rewrites text.

| Work | Model |
| --- | --- |
| Mechanical, no rewriting: running `verify` and `diagnose`, building proposer inputs, applying a candidate, measuring, the guard, committing, the report table | The fast model (Claude Code: `haiku`; Codex: `luna`). |
| Writing a candidate edit (`jev-fix-proposer`) | The model the agent definition names. |
| The lead, and any review of meaning against a PRD | The frontier model. |

An agent on the fast model runs the loop for a record and launches proposers for every edit; it does not write candidates itself. Keep each agent's context small: give it the record, Jev's answers and this page, and nothing else. A long-running lead re-reads its whole history on every turn; start a fresh one for each batch.

## Commit message

```text
<ID>: <strategy> on s<index>

Jev failure: <question>=<choice> @<confidence>
Cause: <the d_* answer or your own question, with Jev's answer>
Strategy: <name>
Tried and failed: <strategy> (<lowest answer it reached>), ...
Result: lowest <before> to <after>
```

## Report

One table, only measured results: record, accepted or unresolved, lowest gate answer before and after, strategies that worked, statements added or removed. Then, for each unresolved statement: its text, Jev's gate and `d_*` answers verbatim, what was tried with its measured result, and the one fact needed from the owner. Then every change that rule 1 sent to the owner, with both texts.

Leave `$W` and `.sc/judge-jev/accepted.jsonl` in place. Every Jev request and answer is logged under `~/.judge-jev/<repository name>/<UTC date>/logs/`, outside the repository; never delete it.
