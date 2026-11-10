#!/usr/bin/env bash
# apply.sh <anchor-id>: validate the tuner's sealed result and write it as the anchor's plan-questions override
set -euo pipefail
ID="$1"; ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"; cd "$ROOT"
K=.claude/skills/plan-anchoring; D=docs/plans/phase-a/anchor-tuning/tune-2026-11-09/$ID; A=docs/plans/phase-a/anchors.json
R=$(ls -t $(find $D -name result.json) 2>/dev/null | head -1); [ -n "$R" ] || { echo "$ID: no result.json"; exit 2; }
V=$(python3 $K/scripts/anchor_tuning_contract.py result "$R" | jq -r '.data.valid'); [ "$V" = true ] || { echo "$ID: result contract invalid"; exit 2; }
python3 - "$R" "$D/override.md" "$D/tuning_history_after.json" <<'PY'
import json,sys
r=json.load(open(sys.argv[1])); d=r.get("data",r)
s=d.get("sealed_question_set")
if not s or not s.get("questions"): print("NO_SEAL"); sys.exit(3)
open(sys.argv[2],"w").write("```json\n"+json.dumps({"questions":s["questions"]},indent=2,ensure_ascii=False)+"\n```\n")
json.dump(d.get("tuning_history_after"),open(sys.argv[3],"w"),sort_keys=True,separators=(",",":"),ensure_ascii=False)
print("sealed", s.get("digest","")[:12])
PY
PB=$(jq -r --arg i "$ID" '.anchors[]|select(.id==$i)|.beads.plan_questions' $A)
bd update "$PB" --body-file "$D/override.md" 2>&1 | grep -v permissions | tail -1
