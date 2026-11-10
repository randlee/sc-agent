# Independent anchor-question calibration

Tune for accurate detection of approved INTENT and locked ANC requirements.
Do not optimize for the current plan to pass. The optional diagnostic plan is
non-normative and may be changed only in disposable copies.

```json
{
  "schema_version": "2.0.0",
  "operation": "tune",
  "template": {
    "id": "anchor-tuning-tune-request",
    "version": "0.4.1",
    "sha256": "d8d6796121bb8155af53277053b25fe9cf99ead86ba55f51b19df8962b441ed3"
  },
  "correlation_id": "tune-plan-ADR-SAG-0004",
  "runtime": {"agent":"anchor-tuning","contract_script":".claude/skills/plan-anchoring/scripts/anchor_tuning_contract.py","contract_sha256":"357b58b6de3b438d8945b5e087ea13eb265af22871e2b6032394786a66930e62","harness":"claude","model":"opus","script":".claude/skills/plan-anchoring/scripts/jev_judge.py","sha256":"906335de770b6393c6b0922aff177e579d1c216ed20d13b80f198cab7ed8ed0a","version":"0.10.1","workspace":"/Volumes/Extreme Pro/github/sc-agent"},
  "authority": {
    "intent": {"content":"# Intent - phase-a\n\nWritten 2026-11-09 by Rand Lee (owner), recorded by the planning session. One page at most. This is the \"why\" that frames every anchor beneath it; Jev questions are judged against anchors, not against this document, so keep it short and let the anchors carry the testable facts.\n\n## What this plan is for\n\nDefine the low-level sc-agent libraries: `sc-agent-types`, `sc-decision-schema` with the decision trait, and the shared HTTP layer. Jev, Haiku, Luna, Strands Decider and Laya sit behind one decision trait. The goal is sc-agent with streaming and session logging, so the owner can analyse 10,000 to 20,000 or more decisions and tune consumer code quickly.\n\n## What success looks like\n\n- Every TypeSafe API request and response can be sent and received through sc-agent with the minimum code needed.\n- A batch of decision requests runs in parallel behind the decision trait, and every send is session-logged.\n\n## What is deliberately out of scope\n\n- The decide CLI design beyond what the low-level libraries must support.\n- Decision-model caching.\n\n## Anchors\n\nAnchors for this plan live in `anchors.json` (record of truth) and are rendered to `anchors.md`. Requirements and ADRs that govern this plan are listed there too, under their own IDs.\n","id":"phase-a-intent","sha256":"5908af69c31e7ca46243005ac725420e801cad90b65d6c250335d3632511be8c","version":1},
    "anchors": {"records":[{"check":"","fixtures":{"fail":["sc-agent-types adds sc-decision-schema to its [dependencies] so it can name DecisionRequest."],"pass":["sc-agent-http depends only on sc-agent-types, sc-agent-secrets and sc-agent-log; no crate depends on sc-decision-runner."]},"id":"ADR-SAG-0004","kind":"prohibition","quote":"Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only.","scope":"adds or changes a crate dependency in a Cargo.toml","state":"locked","statement":"No sc-agent crate\u0027s Cargo.toml must depend on a crate in a higher layer, where the layers from the bottom are: types; config, secrets and log; http; decision schema; decision backends; runner.","summary":"No crate depends on a crate above it in the layering","version":2}],"sha256":"cd9363635bc71f68f297c602ff63dc0b123d143e38908f07b53aa5739bbe16aa"}
  },
  "gate": "planning",
  "current_questions": {"addressed":{"criteria":{"no":"The item text is silent about the directive: it neither includes the directive\u0027s specific content nor references it.","unclear":"The item text alludes to the directive\u0027s topic but it is not possible to tell whether the directive itself is included.","yes":"The item text contains the directive\u0027s specific content, in its own words or the user\u0027s, as a step, constraint, or stated starting point."},"instructions":"Does `item.text` explicitly carry out, preserve, or plan for the directive given in `anchor.statement`? Look for the directive\u0027s specific content being present in the item: the same starting point, the same method, the same constraint, or an explicit step that does what the statement asks. A generic mention of the topic without the directive\u0027s specific content does not count. Judge presence of the directive only, not whether the item conflicts with it.","type":"choice"},"contradicted":{"criteria":{"no":"Nothing in the item text conflicts with the directive, whether or not the directive is mentioned.","unclear":"The item text is ambiguous enough that it might or might not conflict with the directive.","yes":"The item text states a plan, approach, or decision that cannot be reconciled with the directive in the anchor statement."},"instructions":"Does `item.text` propose doing something that `anchor.statement` rules out, or propose a different approach than the one `anchor.statement` requires? Examples of contradiction: the anchor says to start from an existing implementation and the item plans a fresh implementation; the anchor forbids a dependency and the item adds it; the anchor fixes an order of work and the item reverses it. Silence about the directive is not a contradiction. Judge conflict only.","type":"choice"},"relevant":{"criteria":{"no":"The item is about a different part of the system or a different activity, and completing it would neither follow nor break the anchor.","unclear":"The item text is too vague to tell what part of the system it touches.","yes":"The item plans, changes, or depends on the thing the anchor statement is about, directly or as a clear prerequisite."},"instructions":"The state holds one planning item (`item.title`, `item.text`) and one anchor: a directive the user stated verbatim in `anchor.statement`, with the area it governs in `anchor.scope`. Does the work described in `item.text` touch the subject that `anchor.statement` talks about? Judge overlap of subject matter only. Do not judge whether the item obeys the anchor.","type":"choice"}},
  "candidate_minimum": 3,
  "repeat_count": 2,
  "tournament_protocol_version": "plan-anchor/question-tournament/2.0.0",
  "production_confidence_threshold": 0.6,
  "confidence_reference_threshold": 0.8,
  "tuning_history": {"events":[],"history_id":"phase-a-ADR-SAG-0004-planning","schema":"plan-anchor/tuning-history/2"},
  "diagnostic_plan": null
}
```

Freeze the oracle before the first Jev response. Evaluate a larger candidate
pool in parallel, retain the evidence, and down-select the smallest accurate
role-complete question set. Tournament acceptance uses exact semantic labels,
threshold-free derived verdicts, repeat stability, and zero regressions.
Confidence remains diagnostic here; the production threshold remains fail-closed.