# Decision: Crate layering and error values

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-28  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0004: Crate layering and error values

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0002
- REQ-SAG-0009
- REQ-SAG-0301

### Context

sc-agent will launch decision models, LLMs, CLI agents and SDK agents. Each needs shared types, configuration, secrets, HTTP and logging, but not each other's code.

**Problem Statement:**

Crates must be layered so each launcher depends only on what it uses, and errors must keep every lower-layer id and message.

### Decision

Crates are in four layers. A crate depends only on crates in lower layers, except within layer 1: schema, then types, then config and sqlite, then log; a layer-1 crate depends only on layer-1 crates earlier in that order. Layer 1, core types and traits: `sc-agent-schema`, `sc-agent-sqlite`, `sc-agent-types`, `sc-agent-config`, `sc-agent-log`. Layer 2, one crate per API shape: `sc-decision-systemone`, `sc-anthropic`, `sc-openai`. Layer 3, shared behaviour and the API: `sc-decision-runner`, `sc-llm`. Layer 4, thin public surface: the `decidr` CLI and the Python wheel. `sitr` on `sc-treesitter` is a separate stack. Behaviour shared by crates in one layer is placed in a lower layer. A difference between them stays in the higher layer. A provider, a model and a URL are data: adding one changes configuration and no Rust code. CLI agents and SDK agents get their own crates later. Errors are discriminated unions: every call returns `Result<T, E>`, and `E` carries the error info: at least an id and a message. A layer that changes an error keeps the lower error's id and message (REQ-SAG-0002). An error may carry more fields. Whether a crate has its own error type or shares one is not specified.

**Key Principles:**

- A crate depends only on lower layers.
- Shared behaviour moves down; differences move up.
- A provider is a configuration row, not code.

### Rationale

Layers let behaviour that several crates share be written once in a lower layer, and let a new provider or model be added without a code change.

Owner ruling (2026-11-27), Rand Lee: "You need to be able to push shared methods down and move differences up"

Owner ruling (2026-11-27), Rand Lee: "How do you extent to an 'unknown provider' or model' if you need to change rust when a model/provider is added, the architecture = fail. provider/model/url is just a string-tuple."

Owner ruling (2026-11-21), Rand Lee: "Errors are discriminated unions Result<t,e> I don't care about per crate"; "Extra fields are fine, make sure adr doesn't limit too much. I don't want findings for returning more."

Owner ruling (2026-11-21), Rand Lee: "always return descriminated unions w/ error info"

### Consequences

**Positive:**

- A new provider or model is a configuration row. A new API shape is one new layer 2 crate.

**Negative:**

- More crates to version and publish than one core crate.

### Alternatives Considered

#### Alternative 1: One core crate

**Description:**

Five crates, with `sc-agent` as one core crate holding the provider tables, sending, resends, keys, configuration, errors, logging paths and session rows.

**Pros:**

- Fewer crates and fewer lines.

**Cons:**

- Adding a provider or a model needs a Rust change.
- Every concern is in one file, so no boundary can be checked.

**Why Rejected:**

Owner ruling (2026-11-27), Rand Lee: "if you need to change rust when a model/provider is added, the architecture = fail."
