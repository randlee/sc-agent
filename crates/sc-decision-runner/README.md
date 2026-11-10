# sc-decision-runner

The decision runner of sc-agent: `DecisionRunner` implements
`sc_decision_schema::Decision` for any `Arc<dyn Backend>` over one
`sc_agent_http::HttpClient`.

- Worker threads (default 8, `AgentConfig.workers`) drain a bounded job queue;
  results come back fastest first, failures included (REQ-SAG-0701).
- A busy or timed-out send waits in the queue until its holdoff ends, not in a
  worker (REQ-SAG-0601); a reply that does not match the schema is resent
  unchanged up to `max_sends` (REQ-SAG-0901).
- Cancelling the caller's `CancelToken` ends every unfinished request at once
  with `decision.cancelled`, in-flight sends included.
- Exactly one session row per result (ADR-SAG-0002, NFR-SAG-0001).

Sync threads and channels only: no tokio, no sleep (REQ-SAG-0009, REQ-SAG-0010).
