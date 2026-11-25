# Requirement: Run lifecycle audit events

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0023: Run lifecycle audit events

**Status:** Draft  

### Requirement Statement

Definitions: a Run is the context of ADR-SAG-0019; mode is the command or flow name supplied at Run creation; an outcome is success, failure with an error code, or cancellation. Elapsed time is measured by Rust with `Instant` and held as `Duration`.

- Each successfully created, non-dry Run MUST write exactly one `run.start` event and exactly one `run.end` when finalized, through its existing general Logger and with the common header (REQ-SAG-0018).
- Start MUST carry the mode and the Run's one resolved model; it MUST NOT mint another run id or resolve that model again.
- End MUST carry `ok`, the error code for failure/cancellation and elapsed milliseconds. Cancellation and exceptions leaving a consumer with-block MUST still finalize the Run. Explicit typed failure return values MUST also produce failure outcomes.
- Explicit finalization and context exit MUST share one idempotent path; closing again MUST NOT write another end. Failed Run creation MUST write neither event; its Python identity properties remain null. Offline fixture replay and dry runs MUST create no Run and write neither event. A missing project MUST remain a per-call `config.missing_project` error through `project_required()`, with a nullable project in the valid Run header.
- A tool MUST supply its final command outcome on the Run, without inferring it from model/provider names. If a tool does not supply an outcome, the Run uses a recorded call failure or cancellation, otherwise success. A handled failure may be explicitly superseded by the tool's final outcome.
- Logging failures MUST remain typed error values with their original context and category `log`; finalization MUST expose them through the Run's result-returning finish/close methods. No exception-throwing logging API is added.
- New Python lifecycle arguments MUST preserve the wheel’s never-raise contract: unexpected positional, unknown keyword or mistyped input returns sc_agent_py.invalid_option.
- An invalid empty mode MUST return `audit.invalid_mode`, category `validation`, naming `mode` with an action to supply the command mode.
- No separate free run-start/run-end functions, second Logger, timer owner or Python clock MUST be introduced.

### Rationale

The same Run already owns identity and lifetime; another lifecycle would duplicate it.

### Success Criteria

**Acceptance Criteria:**

- [ ] Controlled success, typed failure, exception and cancellation cases each yield one matching start/end pair with one run id and the expected mode/outcome.
- [ ] Repeated close or finish emits no second end; an invalid Run or dry run emits neither.
- [ ] A pure serializer test supplies a fixed Duration and checks elapsed units; tests do not sleep or assert wall-clock timing.
- [ ] A controlled logger failure returns its original log error; borrowed loggers remain usable after finalization.

### Dependencies

**Related:**

- REQ-SAG-0018
- REQ-SAG-0005
- NFR-SAG-0004
- NFR-SAG-0005
- ADR-SAG-0019
- ADR-SAG-0021
