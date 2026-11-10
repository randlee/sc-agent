Fixes applied after the drift review (2026-11-09)

1. A-9 resend cap: busy, timeout and schema resends share one max_sends and one 90 s budget per request (REQ-SAG-0901). Gate re-run: pass.
2. A-7 wording: only sc-decision-llm imports Jev types; sc-llm-provider uses no decision types (ADR-SAG-0004). Gate re-run: pass.
3. ANC-004 logging scope: new anchor ANC-007, locked.
   Statement: "The existing sc-agent library code must be updated so that it writes log records."
   Quote: "You need to go through the code and add logging and inject the logger everywhere that should have had it from day-1."
   The statement drops "inject the logger" (REQ-SAG-0008 covers injection) and "everywhere"; about 70 wordings that kept both failed the rubric (faithful or single_constraint).
   Rubric 3/3 pass; fixtures pass; tuned questions 54/54 cells (default questions 46/54, 2 unstable cases).
   Sprint lines added: A-6 (availability-check records), A-7 (request, token usage, schema mismatch), A-9 (stream start/end, worker stop, session-log failure), A-10 (send, models, doctor; sc-agent-py accepts a logger).
   Excluded: A-1 (types only, below the log layer), A-2 (ANC-001 bars logging code), A-3 (sc-treesitter depends on no sc-agent crate; tsq logging is in A-10), A-4 (ADR-SAG-0004: config and secrets share sc-agent-log's layer and may depend only on lower layers, so their callers log key source and config events).
4. Stabilising A-6 and A-7: each description now says its crates are sc-agent library crates. Jev had doubted that, which made ANC-007 swing between 0.34 and 0.64. A-6 also states that both backends implement sc_decision_schema::Backend; before that, an unrelated edit swung ADR-SAG-0001 between 0.26 and 0.83, and after it the cell held 0.92-0.94 over 4 runs.
Final full gate: all ten sprints pass. Re-runs: A-6 passed 2/2 and A-7 3/3.
