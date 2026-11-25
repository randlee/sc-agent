# Decision: sc-decidr is the one Python decision harness: error values with a stored category, per-call limits and a generated consumer module

**Status:** Approved  
**Version:** 1.0.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-22  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-14  

---

## ADR-SAG-0019: sc-decidr is the one Python decision harness

**Status:** Approved  

### Related Documents

New with this ADR: REQ-SAG-0016 (typed error fields), REQ-SAG-0017 (the category table), REQ-SAG-0702 (per-call limits), REQ-SAG-0018 (the generated consumer module), and NFR-SAG-0003 (one Python floor). REQ-SAG-0007 covers decision values. REQ-SAG-0601 and REQ-SAG-0602 cover resends. REQ-SAG-0701, REQ-SAG-0013 and REQ-SAG-0015 set the configured worker count and timeouts. ADR-SAG-0004 sets the crate layering and that errors are `Result<T, E>`.

**Requirements:**

- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- REQ-SAG-0018
- NFR-SAG-0003
- REQ-SAG-0007
- REQ-SAG-0002
- REQ-SAG-0601
- REQ-SAG-0602
- REQ-SAG-0701
- REQ-SAG-0013
- REQ-SAG-0015

**Architecture Decisions:**

- ADR-SAG-0004

### Context

sc-decidr is the one harness through which sc-agent's Python consumers reach decision models: Jev, decider, Laya, Haiku and Luna. It owns model selection, API-key lookup, session logging (one record per query and response), the injectable general logger, resends and errors.

The owner requires that judge-jev, plan-anchor and every later Python consumer have identical integration for errors, API keys, model selection, the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees.

The two existing consumers each rebuilt the wheel boundary, and the copies diverged:

- **Errors.** judge-jev pins the original id list and falls back to `JEV.UNMAPPED`. plan-anchor matches message text (`"http 401"`).
- **Model.** judge-jev passes the model selector unchanged. plan-anchor pins `provider="typesafe"`, which disables Haiku and Luna.
- **Order.** Both re-sort `send_batch` results into input order.
- **Timeout.** judge-jev refuses `--timeout`. plan-anchor calls the unreleased `timeout_secs`.

Two gaps remained: consumers had no category to branch on, so they pinned id lists; and there was no per-call worker count or CLI timeout flag.

### Decision

sc-decidr makes the wheel the one consumer boundary for Python decision consumers, through these decisions. Decision numbers are stable; there is no decision 3.

1. **The wheel is the consumer boundary.**

   Every consumer concern except prompt authoring and result interpretation belongs in sc-decidr.

   | Concern | Provided by sc-decidr | Consumer does |
   | --- | --- | --- |
   | Model and provider | `model=` takes a short name or a full model id. With `provider=None` the first provider table, in list order, that has the name is used; among rows that share a short name the most recent wins. | Pass the selector unchanged. Never set `provider`. |
   | API keys | The environment, then the repo-root `.env`, then the OS keyring. | Nothing. |
   | Session log | One record per query and response, always written to the wheel's default location, tagged with `project` and `agent_role`. | Pass `project` and `agent_role`. Sessions are appended to the default session files (REQ-SAG-0008, ADR-SAG-0021); no session Logger is injected. |
   | General log | The generated client obtains one Logger from the sc-decidr wheel, named after the client and writing `~/.sc-agent/logs/<client>.log.jsonl`, and injects it into every sc-decidr and sitr call (REQ-SAG-0022, ADR-SAG-0021). | Log its own records to the client's Logger, or pass its own Logger to the client. |
   | Errors | The returned error value: `id`, `message`, `action`, a stored `category` and the send count (decisions 2, 5 and 6). | Map a category to its own codes. Never parse message text. |
   | Limits | `timeout_secs`, `workers`, a fixed resend count and cancellation (decision 7). | Pass its own flags through. |
   | Batch order | `send_batch` returns one result per request, each with its input position (decision 8). | Nothing. |

   **The integration artifact is a generated module** (REQ-SAG-0018). The wheel ships an sc-compose template, `sc_agent/templates/consumer.py.j2`, as package data, and each consumer renders its integration module from it. To carry the file, the wheel moves to maturin's mixed layout; `import sc_agent` is unchanged.

   - The template covers the decision call, model pass-through, logging wiring, cancellation and the error-to-category-to-code mapping. Prompt authoring and result processing stay in the consumer.
   - Its variables are `client_name`, `sitr` (`true` or `false`) and `sc_decidr_version`; model selectors are call arguments. Session logging and the injected general logger are not optional (REQ-SAG-0008, REQ-SAG-0022).
   - sc-compose is an external render tool, not a dependency of the wheel. The template's front matter declares its sc-compose minimum (`requires.cli`); the minimum is declared, not enforced.
   - All Jev queries and responses go to the session log, so every consumer's Jev data takes the same path and can be filtered by repository and agent.

   **The lower-level API stays the contract.** `send`, `send_batch`, `digest`, `logger`, `log`, `flush`, `Cancel` and `sc_agent.sitr` are public. The generated code uses only them and the standard library.

2. **Typed fields: `attempts` and `category`.**

   An error value carries at least `id`, `message` and `action`, and the typed fields below when they apply. It may carry more fields. Unset is not written.

   | Field | Meaning |
   | --- | --- |
   | `attempts` | The sends made for this request, the first one included. |
   | `category` | The error's category (decision 6). |

   REQ-SAG-0016 holds the requirements.

   **Retry-After is read as a Duration** and used for the wait before a resend.

4. **Breaking minor release.**

   The Rust change breaks SemVer: the error type and the call API change. Below 1.0 a minor bump is the breaking bump. For Python the change only adds things.

   All workspace crate and wheel versions move together (`[workspace.package].version` in `Cargo.toml`); `verify-version-lockstep` enforces this. sc-decidr is the one wheel and holds the sitr API, so decidr and sitr always have the same version in Python.

5. **Python errors.**

   Every wheel call returns ok with a value or an error with its id and message; nothing raises.

6. **The category is stored when the error is built.**

   The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.

   - Every error has a stable id.
   - Every error stores its category.

   Python reads the `category` field of the returned error value.

   **Retryable means "worth resubmitting as a new call later, after the wheel's own resends have run out."** It is true for `busy` and `network` only. A consumer that resubmits waits first and does not wrap the wheel in an immediate retry loop.

7. **Per-call limits: `timeout_secs` (exists) and `workers` (new).**

   `send` and `send_batch` take an optional timeout (a Duration) and an optional worker count; each applies to that call only; a bad value is an error value.

   - Python: `send_batch(workers=, timeout_secs=)` and `send(timeout_secs=)`. CLI: `--workers N` and `--timeout-secs SECS`.
   - A limit applies to its own call only. `None` means the configured value. A `workers` value below or above the configured count is honored.
   - `timeout_secs` replaces the default 120 s send timeout for every send of the call.
   - Every send gets the full timeout, resends included. Busy and network errors are resent up to a fixed count.

   REQ-SAG-0702 holds the requirements.

8. **`send_batch` returns one result per request, each with its input position.**

   `on_result` still receives each line as it finishes.

9. **Verification is the contract tests and the consumers' suites; the integration gate is removed.**

   `decidr::integration_gate`, its re-exports `integration_cases` and `validate_integration_results`, the matching `sc_agent` functions and the surrounding report and gate tooling are removed before 0.2.0. `decidr.integration_evidence` is not an error id.

   Contract tests run at the product boundary against a local HTTP server in the test, with no network, no real home and no sleeps. The work is done when they pass and plan-anchor's and judge-jev's suites pass against the new wheel. REQ-SAG-0133 is superseded by this decision.

10. **Category assignments are not locked.**

    Rand Lee (2026-11-20): "schema isn't locked yet ... No hand built checks."

11. **sitr is an optional, separate section of the consumer template.**

    sitr's Python API is `sc_agent.sitr`, inside the sc-decidr wheel (ADR-SAG-0005, ADR-SAG-0021). The template's sitr section is kept separate from the decision call: extraction runs outside the decision path, and its output is passed into the consumer's prompt authoring. The section uses `sc_agent.sitr` only. In this phase one offline test shows extraction output reaching a dry-run request; sitr is not used beyond that yet.

12. **One Python floor (NFR-SAG-0003).**

    - `floors.toml` at the repository root writes down the Python floor (`[python] floor`) and the sc-compose minimum (`[tools] sc-compose`). It only records values.
    - The pyo3 abi3 feature in `sc-agent-py` moves to the abi3 feature of the Python floor, so the wheel tag matches the `requires-python` the wheel declares.
    - Until sc-compose can read `floors.toml`, the template carries these values as literals.

### Rationale

Two consumers rebuilt the wheel boundary and the copies diverged, so every concern except prompt authoring and result interpretation moves into sc-decidr, and the integration code is generated from one template. A stored category and the send count replace message-text parsing and pinned id lists. Storing the category when the error is built needs no lookup. Per-call limits and position-tagged batch results remove the consumer code that worked around their absence. Prerelease tooling leaves the product API so it does not freeze into the error contract.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "I would never create a requirement requiring subnanosecond rounding for a http call." "Use time structs for time." "always return descriminated unions w/ error info" "I do not require results in request order." "No hand built checks."

### Consequences

**Positive:**

- Every Python consumer renders the same integration module and contains only its own prompts and result handling.
- No consumer parses error text or pins id lists.
- All Jev data from every consumer takes one session-log path.
- Prerelease tooling no longer ships in the product crate or wheel.

**Negative:**

- This is a breaking release for Rust dependents: the error type and the call API change.
- The wheel moves to maturin's mixed layout.

### Alternatives Considered

#### Alternative 1: A separate `server` category for 500, 503 and 504

**Description:**

Add a `server` category instead of counting these final replies as `busy`.

**Why Rejected:**

It would extend the fixed category set for a distinction no consumer acts on; `busy` already means "resubmit later".

#### Alternative 2: A "Consumer integration" recipe in the README

**Description:**

Document the integration as a recipe for each consumer to copy.

**Why Rejected:**

A recipe is copied by hand, and two hand-written copies already diverged.

#### Alternative 3: An import-time version check in the consumer module

**Description:**

Raise on import when the versions recorded in the generated header differ from the installed sc-decidr or sitr.

**Why Rejected:**

A fail-fast check makes troubleshooting and development harder. Mixed versions are documented as unsupported, not enforced.

### Implementation

The work lands on one branch, `rewrite/v1`, and merges into `develop` in one PR. The tests are the acceptance criteria of REQ-SAG-0016, REQ-SAG-0017, REQ-SAG-0018, REQ-SAG-0702 and NFR-SAG-0003.

Deliverables beyond the code:

- `CHANGELOG.md` (new), with "Breaking" and "Consumer migration" sections. The migration section names what judge-jev and plan-anchor delete: pinned id lists, `JEV.UNMAPPED`, message-text status parsing, the `--timeout` refusal, the ignored `--max-workers`, the `provider="typesafe"` pin, result re-sorting, their own retry loops and their hand-written integration glue.
- The consumer template and the `sc_agent/__init__.pyi` stub.
- `floors.toml`.
- The sc-decidr wheel, holding the decidr and sitr APIs, released to PyPI.

Changes inside judge-jev, plan-anchor and other consumer repositories are out of scope.
