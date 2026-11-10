# Phase-a drift review (read-only), 2026-11-09

Compared: anchors.json a447d01 -> HEAD (015839a); gate-items a-1..a-10 (ecd83ef -> 015839a); beads agt-4p2.1..10 (acceptance_criteria, design, description). The bead acceptance_criteria match the gate-item texts exactly in all 10 sprints.

## 1. Material drift

### M1. A-9 (agt-4p2.9 design): schema-mismatch resends get a fresh send count and a fresh 90 s budget (medium; owner call)
- Where: A-9 design: "decoded replies are counted apart from the busy and timeout sends of each reply, assistant decision", and `attempts: Attempts, // fresh after each schema resend`. AC: "decision.schema_mismatch ... after max_sends decoded replies".
- What changed: one request can now be sent max_sends x max_sends times (25 by default), and the 90 s attempt budget restarts after each schema resend.
- Contradicts: REQ-SAG-0901, "resent unchanged, up to the configured maximum number of sends". owner-busy-resend, "Max sends configurable, default 5 ... 90 s attempt budget". owner-llm-decider-mode leaves the mechanism to the assistant, but not the cap.
- Fix: keep one send counter and one budget per request, shared by busy, timeout and schema-mismatch resends. Or get the owner to approve the per-reply count explicitly.

### M2. A-7 AC (REQ-SAG-0201 line) makes sc-llm-provider import from sc-decision-schema (low; wording fix)
- Where: a-7.txt / agt-4p2.7 AC: "sc-decision-llm and sc-llm-provider import every Jev-mapped type (SystemOneRequest, SystemOneResponse and the answer types) from sc-decision-schema".
- What changed: this line says sc-llm-provider depends on the decision schema. The same sprint's ADR-SAG-0004 AC says sc-llm-provider's only sc-agent dependencies are types, secrets, log and http. A-7 design line 232 says "sc-decision-schema does not depend on either".
- Contradicts: ADR-SAG-0004 rationale, "let a non-decision program use HTTP, providers and logging without decision types". owner-architecture, "sc-llm-provider -> sc-decision-llm".
- Fix: "sc-decision-llm imports every Jev-mapped type from sc-decision-schema and defines none of its own; sc-llm-provider defines and uses no Jev-mapped type."

### M3. ANC-004 retirement reason only half true (low)
- Where: anchors.json ANC-004, retired "covered by REQ-SAG-0008 (optional logger injection with default loggers)".
- What changed: REQ-SAG-0008 covers injection and defaults only. The other half of the owner's words now has no anchor or requirement: "go through the code and add logging and inject the logger everywhere that should have had it from day-1". The sprints that log are A-5 (http), A-8, A-9 and A-10. A-4 and A-6 have no general-log events of their own. A-6 discovery is logged only through HttpClient.
- Mitigation: ANC-004 was only proposed, never locked, so it is not among the 11 approved anchors.
- Fix: re-propose a narrowed anchor, e.g. "every sc-agent library crate that performs I/O logs through the injected or default general logger". Or record that the owner accepts REQ-SAG-0008 as sufficient.

No other material drift found.

## 2. Acceptable implementation choices the owner should know (one line each)
- A-1: on an equal release date the config model wins (owner said he does not care). A-10 doctor warns on equal dates, as REQ-SAG-0503 requires.
- A-1: Provider::PRIORITY = TypeSafe, Anthropic, OpenAi, OpenRouter, Together, Nous (direct providers before aggregators; assistant decision).
- A-1: jev-1.13.0's release date is a TODO for the owner. docs.typesafe.ai gives none, and the sprint cannot close without it. Owner said "look it up"; it was looked up and not found.
- A-3: new crate crates/sc-treesitter-py; 19 error variants (the owner's 17 plus Write and Log, added for tsq).
- A-3/A-10: tsq lives in sc-treesitter behind a default `cli` feature (clap and sc-observability). The "library writes no log" check therefore runs with --no-default-features.
- A-4: deps keyring and whoami; config file ~/.sc-agent/default.json with SC_AGENT__ env overrides; a runner named like a short name is rejected (config.runner_name).
- A-5: Retry-After counts as valid only when it is 1-32 ASCII digits with an optional fraction; after 8 s the backoff keeps doubling, capped at 30 s.
- A-6: daemon kinds are strands, laya, mapika and generic (`decider` renamed `strands` to keep it apart from Mapika/decider); a 503 from a local daemon is final.
- A-7: sc-llm-provider "sits on the http layer". This is stricter than the anchor's "decision backends (sc-decision-llm over sc-llm-provider)" and matches the ADR rationale.
- A-7: the decider system prompt is drafted by the assistant for owner review (Mapika/decider uses none).
- A-8: invented test name log_error_keeps_lower_layer_cause (harmless).
- A-8/A-4: default log_root is ~/.sc-agent, which matches sc-observability's documented default `~/.<app>` (checked in its docs).
- A-9: queue capacity is a constant 4 x workers, not a config key.
- A-9: a single `decide` holds off on the caller's thread with CancelToken::wait_until rather than in the queue. Idle workers use Condvar timed waits. Neither is a sleep.
- A-10: new crates `decide` (lib + bin) and `sc-agent-py` (module sc_agent, publish=false, depends on the decide lib).
- A-10: this is not scope creep. REQ-SAG-0002, 0004, 0007 and REQ-SAG-0104 AC2 already require PyO3 bindings, and no earlier sprint delivered them.
- A-10: `SagError` is a Python exception carrying (id, message, detail chain), the same pattern as TreesitterError in A-3. It is consistent with the per-crate error unions.
- A-10: --project and --agent-role are required with no default; tsq logs to the fixed path ~/.sc-agent/logs/tsq.log.jsonl, because sc-treesitter may not read sc-agent-config.
- ANC-003 ("no REQ/NFR/ADR/bead cites plan-anchor or judge-jev as a source") is still only proposed and gated nowhere. The designs mention plan-anchor and judge-jev only as agent_role examples and in REQ-SAG-0006 text.
- Outside this review's scope: 16 REQ/NFR/ADR files have uncommitted modifications and 9 are new, all in the working tree. They match owner memories, but owner-rule-req-adr-changes requires owner approval before they are committed.

## 3. No-drift confirmations

Anchors
- The approved changes are applied exactly: ANC-001 pass fixture, REQ-SAG-0006 pass fixture, REQ-SAG-0010 fixtures (name sc-agent), and the ADR-SAG-0004 statement ("must" -> "may", with sc-llm-provider placed under sc-decision-llm).
- Since a447d01, no quote, scope, deterministic_check or kind changed on any pre-existing anchor. question_overrides are empty everywhere.
- The other new fixtures are consistent with their records and owner memories: ADR-SAG-0001, ADR-SAG-0004, REQ-SAG-0002, 0009, 0201, 0601, 0701 and ANC-006.
- ANC-002 -> ANC-006 is correct. "sc-ai CLI" became "decide" (owner-cli-requirement-1), and the quote is unchanged. tsq-vs-PyO3 parity stays covered by REQ-SAG-0303 ("tsq CLI can expose a subset of the PyO3 query surface").
- ANC-005 -> REQ-SAG-0701 is correct. REQ-SAG-0701 defines a batch as "one JSONL file, one stdin stream or one Python call", and requires workers with a default of 8.
- Every exclusion reason was checked against the current sprint text and is true:
  - ANC-001: A-5, A-6, A-7.
  - ADR-SAG-0001: A-5; A-10 (it only wires backends and calls Decision).
  - ADR-SAG-0004: A-3.
  - REQ-SAG-0006: A-5 to A-8.
  - REQ-SAG-0009: A-3.
  - REQ-SAG-0201: A-1; A-10 (sc-agent-py takes JSON strings, and its only pyclass is CancelToken).
  - REQ-SAG-0601: A-2, A-4, A-6, A-7, A-10.
  - REQ-SAG-0701: A-4.
  - ANC-006: A-1, A-3.
  - The retired ANC-002's exclusions are now moot.

Sprint criteria (changes from ecd83ef to HEAD move toward the owner's decisions)
- --provider is now optional and picked by ranking (owner-provider-priority); before, it was required.
- Timeouts are resent within max_sends; connect, DNS and TLS failures are not; 402 and quota errors are returned with the remote id and message (owner-busy-resend, owner-retry-transient).
- Key precedence is env, then repo .env, then keyring (owner-provider-priority).
- The global config is in ~/.sc-agent/ and each CLI writes its own general log (owner-config-log-root).
- There is one sessions.jsonl per repo/agent_role/project, and NFR-SAG-0001 now expects "one file" (owner-session-log-layout).
- There is no unsupported-input error variant (13 decision ids) (owner-llm-inputs).
- Structured output is used for Haiku and Luna (owner-llm-decider-mode).
- Mapika/decider is added (owner-system-one-backends, owner-local-decision-crate).
- A batch fails fast before any send, naming line and field (owner-input-fail-fast).
- doctor warns on ties and lists keys in order (owner-catalog-ties, owner-provider-priority).
- tsq is named (owner-treesitter-cli-name), and Python and Rust come first with csharp and Go second (owner-treesitter-grammars).
- No caching requirement appears anywhere in the sprint text (owner-decision-schema-scope).
- No rounding (REQ-SAG-0007), ULIDs for ids, no agent_name.

Layering and dependencies
- Every per-crate dependency list in A-1..A-10 points down the ADR-SAG-0004 order:
  - types: serde and ulid
  - config/secrets: types and sc-config
  - log: types
  - http: types, secrets, log
  - schema: types and http
  - backends: schema, plus sc-llm-provider for llm
  - runner: everything below except the backends
  - decide: all of them; sc-agent-py: decide
- sc-treesitter depends on no sc-agent crate.
- No tokio: sc-observability 1.5.0 has no tokio dependency (checked its Cargo.toml), ureq is sync, and each sprint has a cargo-tree check.
- No sleep in any sprint: there is a clippy disallowed-methods rule, and waits use the CancelToken or a Condvar.
- REQ-SAG-0002 chain-carrying tests exist in every sprint.

Not reviewed: the content of the sealed plan question sets (8b9e6cb), and the full bead design bodies (about 200 KB) beyond targeted searches.
