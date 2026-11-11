# Intent - phase-a

Written 2026-11-09 by Rand Lee (owner), recorded by the planning session. One page at most. This is the "why" that frames every anchor beneath it; Jev questions are judged against anchors, not against this document, so keep it short and let the anchors carry the testable facts.

## What this plan is for

Define the low-level sc-agent libraries: `sc-agent-types`, `sc-decision-schema` with the decision trait, and the shared HTTP layer. Jev, Haiku, Luna, Strands Decider and Laya sit behind one decision trait. The goal is sc-agent with streaming and session logging, so the owner can analyse 10,000 to 20,000 or more decisions and tune consumer code quickly.

## What success looks like

- Every TypeSafe API request and response can be sent and received through sc-agent with the minimum code needed.
- A batch of decision requests runs in parallel behind the decision trait, and every send is session-logged.

## What is deliberately out of scope

- The decidr CLI design beyond what the low-level libraries must support.
- Decision-model caching.

## Anchors

Anchors for this plan live in `anchors.json` (record of truth) and are rendered to `anchors.md`. Requirements and ADRs that govern this plan are listed there too, under their own IDs.
