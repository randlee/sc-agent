---
name: write-records
version: 0.7.0
description: How to get a REQ, NFR or ADR record written from a list of what it requires, in place of editing its text - when to use it, the two prompt templates, who fills them, how the candidates are measured and accepted.
---

# Write records

Editing a failing statement keeps the writer close to the wording that failed. This page replaces the text instead: one writer lists what the record requires (its tenets), other writers write new text from that list alone, and Jev measures every candidate. Nobody who writes a candidate sees the old text.

Two sc-compose templates do the asking. Both are in `assets/templates/` of this skill:

| Template | Sent to | Returns |
| --- | --- | --- |
| `record-tenets.md.j2` | One writer who knows the PRD, once per record | JSON: `tenets` (one per obligation), `rationale`, `terms`, `must_not_say`, `conflicts` |
| `record-write.md.j2` | Every record writer, the same text to each | JSON: `candidates` (complete texts with a `covers` map), `cannot` |

## When to use it

Judge the record first (`references/fix-strategy.md`, `verify base`), then choose by what `verify` printed:

| What `verify` shows | Do |
| --- | --- |
| One or two weak answers, all on one statement | `references/fix-strategy.md`: edit that statement. |
| Any statement answers `obligations` `2`, `3` or `4+` with `joined_by` `and-list`, `alternatives` or `obligation+rationale` | This page. Listing the tenets is the split. |
| Weak answers on three or more statements | This page. Statements share the record as context, so edits to one move the others. |
| Two consecutive fix rounds kept no candidate | This page. |
| New record | Save a draft with its standard heading and sections in the target file; start at step 0. |

Never use it on an accepted record: an accepted record is frozen.

## Who does what

- **You, the coordinator.** Render, send, save the replies, measure. You fill template variables only from files. You never write or reword a tenet, and you never choose a candidate by reading it. The fast model is enough for this.
- **The tenets writer.** One frontier-model agent that knows the PRD. It decides what the record requires; everything after trusts its list.
- **The record writers.** One or more frontier-model agents, of different models where you have them. Each gets the same rendered text and no other context. They do not call Jev and do not edit files.

## 0. Set up, once per record

Do step 0 of `references/fix-strategy.md` first; it writes `.sc/judge-jev/fix/<ID>/env`. Then:

```bash
which sc-compose && sc-compose --version   # the templates are tested with 1.8.0
. .sc/judge-jev/fix/<ID>/env
A="<this skill's directory>/assets/templates"
X=.sc/judge-jev/write/$ID; mkdir -p "$X"
printf 'A=%s\nX=%s\n' "$A" "$X" >> "$W/env"
```

Put the record's lines, from its first `###` heading to its last line, in `$X/record.md` (`sed -n '<first>,<last>p' "$F"`). Put the PRD clauses that govern it in `$X/prd.json` as `[{"id": "...", "text": "..."}]`, each `text` copied from the PRD file. Then:

```bash
jq -n --arg id "$ID" --arg kind "$KIND" --arg title "<the record's title>" --rawfile text "$X/record.md" --slurpfile prd "$X/prd.json" \
  '{record_id: $id, kind: $kind, title: $title, record_text: $text, prd_clauses: $prd[0]}' > "$X/record.json"
```

## 1. Tenets

```bash
sc-compose render --root "$A" --file "$A/record-tenets.md.j2" --var-file "$X/record.json" --strict > "$X/tenets-prompt.md"
```

Send `$X/tenets-prompt.md` to the tenets writer and save the JSON it returns as `$X/tenets.json`. Check it:

```bash
jq -r '"tenets \(.tenets | length), conflicts \(.conflicts | length), terms with no meaning \([.terms[] | select(.meaning == null)] | length)"' "$X/tenets.json"
```

- `conflicts` above 0: the record requires something a PRD clause forbids. Stop work on this record and list it for the owner with the conflict. Do not write around it.
- More than 8 tenets: go on, and list the record as one the owner may want divided into several records. Never create a record id.
- Do not edit `tenets.json`. A tenet you think is wrong goes back to the tenets writer, once, with the reason.

## 2. Write

```bash
jq -s '.[0] + .[1] + {n: 3} | del(.record_text)' "$X/record.json" "$X/tenets.json" > "$X/write.json"
sc-compose render --root "$A" --file "$A/record-write.md.j2" --var-file "$X/write.json" --strict > "$X/write-prompt.md"
```

`del(.record_text)` is what keeps the old text from the writers. `n` is the number of candidates each writer returns; 3 from each of two writers is the default.

Send `$X/write-prompt.md` to every record writer in one message. Save each reply as `$X/<writer>.json`.

## 3. Measure

Reject before measuring a candidate that leaves a tenet uncovered or, for a REQ or NFR, covers two tenets with one statement:

```bash
jq -r --arg kind "$KIND" --slurpfile t "$X/tenets.json" '(.cannot // [] | map(.tenet)) as $cannot | .candidates[] | . as $c
  | [$t[0].tenets[].id | select(. as $id | $cannot | index($id) | not) | select(($c.covers[.] // []) | length == 0)] as $missing
  | [$c.covers[]?[]] as $all
  | "c\($c.n): " + (if ($missing | length) > 0 then "REJECT uncovered \($missing | join(","))"
      elif $kind != "adr" and ($all | length) != ($all | unique | length) then "REJECT one statement covers two tenets" else "ok" end)' "$X/<writer>.json"
```

A tenet under `cannot` is not a rejection; list it in the report. For each candidate that printed `ok`, as `<v>` = `<writer>-c<n>`:

```bash
jq -r '.candidates[] | select(.n == <n>) | .text' "$X/<writer>.json" > "$X/<v>.md"
python3 - "$F" "$ID" "$X/<v>.md" <<'PY'
import re, sys
path, rid, cand = sys.argv[1:4]
lines = open(path, encoding="utf-8").read().split("\n")
new = open(cand, encoding="utf-8").read().strip("\n").split("\n")
level = lambda l: len(re.match(r"(#+) ", l).group(1)) if re.match(r"#+ ", l) else 0
def stop(why):
    print(f"REJECT: {why}"); raise SystemExit(2)
heads = [i for i, l in enumerate(lines) if level(l) and re.search(r"(?<![\w-])" + re.escape(rid) + r"(?![\w-])", l)]
if len(heads) != 1:
    stop(f"{len(heads)} headings name {rid} in {path}")
start, depth = heads[0], level(lines[heads[0]])
end = next((i for i in range(start + 1, len(lines)) if 0 < level(lines[i]) <= depth), len(lines))
def sections(block, d):
    marks = [i for i, l in enumerate(block) if level(l) == d] + [len(block)]
    return [(block[a][d + 1:].strip(), a, b) for a, b in zip(marks, marks[1:])]
record, fresh = lines[start:end], sections(new, 3)
if not fresh or level(new[0]) != 3:
    stop("the candidate does not start with a ### section heading")
have = dict((name, (a, b)) for name, a, b in sections(record, depth + 1))
missing = [name for name, _, _ in fresh if name not in have]
if missing:
    stop(f"the record has no section {missing}")
out, cursor = [], 0
for name, a, b in sorted(fresh, key=lambda s: have[s[0]][0]):
    body = new[a + 1:b]
    while body and not body[-1].strip():
        body.pop()
    out += record[cursor:have[name][0]] + ["#" * (depth + 1) + " " + name] + body + [""]
    cursor = have[name][1]
open(path, "w", encoding="utf-8").write("\n".join(lines[:start] + out + record[cursor:] + lines[end:]))
print(f"applied {len(fresh)} sections to {rid}")
PY
```

It replaces each section the candidate wrote and leaves the record's title, header lines and every other section as they were. Then `verify <v>` (`references/fix-strategy.md`), and `cp "$W/base.md" "$F"` before the next candidate.

## 4. Accept

Among the candidates whose `verify` printed `pass=true`, keep the one with the highest `lowest`. Apply it to `$F`, then:

1. Run the guard of `references/fix-strategy.md` step 4 on `$W/original.md` against `$F`. A lost number, record id or code span that a tenet carries rejects the candidate: take the next best.
2. Send the kept text and `$X/tenets.json` to the tenets writer once: does the text require every tenet and nothing else? `no` rejects the candidate: take the next best.
3. Accept and freeze as in `references/fix-strategy.md` step 5, and commit.

No candidate passes: start round 2 at step 2. Add each weak answer’s statement, question and runner-up from the best candidate’s run files to `write.json` as `avoid`. Two rounds at most; then the record is unresolved: commit nothing, keep `$X`, and list the record with its best candidate and that candidate's `verify` output.

## Rules

The rules of `references/fix-strategy.md` hold here, with one reading: "obligations do not change" means the kept text requires exactly the tenets, and the tenets are exactly what the old record required.

- Do not show a record writer the old text, another writer's candidate, or Jev's scores for a candidate. `avoid` lines are the only feedback.
- Do not merge two candidates by hand. A candidate is measured and kept whole.
- Do not ask the owner how to word something. Ask the owner only about a `conflicts` entry.

## Commit message

```text
<ID>: written from <count> tenets

Tenets: <count>, from <tenets writer>
Candidates: <measured> measured of <returned> returned, by <writers>
Kept: <writer>-c<n>, lowest <median>; <count> passed
Rejected: <candidate> (<reason>), ...
Cannot: <tenet id> (<why>), ...
```

## Report

Per record: id, tenets, candidates returned/passed, kept and lowest median, or `unresolved` with best median. Diagnose failures; report reasons per SKILL.md. List conflicts, over 8 tenets, and `cannot` tenets.
