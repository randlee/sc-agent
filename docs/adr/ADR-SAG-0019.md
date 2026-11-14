# Decision: Structured error fields, an exported error-id catalog, and per-call workers and timeout (sc-decidr 0.2.0)

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  
**Decision Date:** pending owner approval

---

## ADR-SAG-0019: Structured error fields, an exported error-id catalog, and per-call workers and timeout

**Status:** Proposed  

### Related Documents

**Requirements:**

- REQ-SAG-0016 Error details carry the HTTP status, send count and Retry-After as typed fields (new)
- REQ-SAG-0017 Every error id is published in a catalog with one category (new)
- REQ-SAG-0702 A batch or send call may override workers and the send timeout for that call only (new)
- NFR-SAG-0002 Error detail JSON written by 0.1.0 still reads in 0.2.0 (new)
- REQ-SAG-0002 Errors are a discriminated union carried from bottom to top (unchanged; this ADR adds detail only)
- REQ-SAG-0007 Numbers are never rounded (governs decision values; `retry_after_ms` is a transport hint: decision 1)
- REQ-SAG-0601, REQ-SAG-0602 Busy resend and quota (where the status, send count and Retry-After come from)
- REQ-SAG-0701 Batches run in parallel with a configured worker count (unchanged default)
- REQ-SAG-0013, REQ-SAG-0015 Worker cap of 64; local runner timeouts

**ADRs:** ADR-SAG-0004 (the Python binding is one function per decidr operation)

### Context

sc-decidr 0.1.0 has three gaps that its first consumer (judge-jev) works around with code of its own:

1. `ErrorDetail` (`crates/sc-agent-types/src/error_id.rs`) has only `id`, `message`, `recovery` and `cause`. The
   typed data the errors hold is lost when they become `ErrorDetail`. This affects `HttpError::ProviderBusy { attempts,
   retry_after_seconds, last }`, `QuotaExhausted { attempts, reply }`, `ResponseTooLarge { status }`,
   `Network { attempts }`, `DecisionError::{ApiStatus, ResponseParse} { status }`, `ApiBadRequest` (400),
   `ApiValidation` (422), `LlmError::ApiStatus { reply }` (for example 401) and `LocalError::ProbeStatus { status }`.
   That data is printed inside `message` and nowhere else. `ErrorDetail` is the only error that crosses into Python
   (`SagError` detail JSON) and the batch result lines. A consumer that needs to know "was this a 401?" therefore
   parses English.
2. No list of error ids exists. A consumer that branches on ids pins its own copy of the list and needs a fallback
   (judge-jev's `JEV.UNMAPPED`) for ids it has not seen.
3. `sc_agent.send`/`send_batch` (`crates/sc-agent-py/src/lib.rs`) take neither `workers` nor `timeout`. The worker
   count comes only from `AgentConfig.workers` (default 8, REQ-SAG-0701). The per-send HTTP timeout is fixed: 120 s
   (`Timeouts::default()`), or a local runner's `request_timeout_secs`. judge-jev refuses its own `--timeout` and
   ignores its `--max-workers`.

### Decision

#### 1. Three typed fields on `ErrorDetail`

`ErrorDetail` gains three fields. Each is `#[serde(default, skip_serializing_if = "Option::is_none")]`:

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `Option<u16>` | The HTTP status of the reply this error is about. |
| `attempts` | `Option<u32>` | Sends made for this request, the first included. |
| `retry_after_ms` | `Option<u64>` | The last reply's valid `Retry-After`, in whole milliseconds, rounded **up**. |

The struct keeps `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`. No floating-point field is added, so `Eq`
stays.

**Retry-After and REQ-SAG-0007.** REQ-SAG-0007's ban on rounding protects values a decision model returns,
such as Jev probabilities and confidences, and values computed from them (owner, 2026-11-14). `Retry-After` is a
transport timing hint, not a decision value, so `retry_after_ms` is the ceiling of the value in milliseconds. A
caller that waits that long never retries early. The exact value stays available in two places: the Rust field
`HttpError::ProviderBusy::retry_after_seconds: Option<f64>`, and the `Retry-After {seconds} s` text of `message`.
Rejected: `retry_after_seconds: Option<f64>`. It drops `Eq` from `ErrorDetail` and from every type that holds it.

#### 2. One provided `ErrorId::fields()` method, not an extension map

`sc-agent-types` adds `ErrorFields { status, attempts, retry_after_ms }`, a `Copy`, `Default`, `#[non_exhaustive]`
struct with `with_status`/`with_attempts`/`with_retry_after_ms` builders. `ErrorId` gains one provided method:

```rust
fn fields(&self) -> ErrorFields { ErrorFields::default() }
```

`detail()` stays the only builder. It now copies `fields()` into the detail. Implementors still must not override
it. Each crate's existing "detail equals the value built from the trait methods" test now also compares `fields()`.

Each variant sets the fields that hold for it:

| Id | status | attempts | retry_after_ms |
| --- | --- | --- | --- |
| `http.provider_busy` | `last.status` | `attempts` | from `retry_after_seconds` |
| `http.quota_exhausted` | `reply.status` | `attempts` | from `reply.retry_after_seconds` |
| `http.network` | — | `attempts` | — |
| `http.response_too_large` | `status`, but `None` when 0 (the body was refused before the status was read) | — | — |
| `llm.api_status` | `reply.status` (401, 403, 404, 5xx ...) | — | from `reply.retry_after_seconds` |
| `decision.api_bad_request` | 400 | — | — |
| `decision.api_validation` | 422 | — | — |
| `decision.api_status`, `decision.response_parse` | `status` | — | — |
| `local.probe_status` | `status` | — | — |
| `decision.cancelled` | — | `sends` | — |

A variant that wraps a whole lower error returns that error's fields. This covers `decision.http`,
`decision.backend`, `llm.http`, `decision_llm.llm`, `local.http`, `local.decision` and the `decidr.*` wrappers.
The variants keep the lower error as their cause; copying its fields up only adds detail (REQ-SAG-0002). A
consumer therefore reads the top level (`line["error"]["status"]`, `SagError.status`) and does not walk the
chain. Every lower level still carries its own fields.

Rejected: a typed extension map (`BTreeMap<String, Value>`) on `ErrorDetail`. It is untyped, has no field names
for Python or rustdoc to show, and would make every new datum a convention instead of a reviewed field. The
`#[non_exhaustive]` `ErrorFields` already lets a later field be added without a breaking change.

#### 3. `#[non_exhaustive]` on `ErrorDetail`

Yes. Adding the three fields already breaks every struct literal and exhaustive destructure outside
`sc-agent-types`. With `#[non_exhaustive]`, this is the last field addition that breaks anything. Outside the crate,
a value is built with `ErrorDetail::new`, `with_recovery`, `with_fields(ErrorFields)` (new) and
`ErrorDetail::foreign`. Fields stay `pub` for reading. Inside the workspace, the struct literals in tests become
builder calls.

#### 4. Version 0.2.0 for every crate and the `sc-decidr` wheel

The Rust change is SemVer-breaking: public fields are added to an exhaustive public struct, and `#[non_exhaustive]`
is added. The crates are published on crates.io. Below 1.0, Cargo treats a minor bump as breaking, so `^0.1`
dependents are not moved onto it silently. A 0.1.1 would break them.

The workspace has one version (`[workspace.package]`), so every crate and the wheel move to 0.2.0 together. For
Python the change is additive: the `SagError` args, the result-line keys and the existing kwargs are unchanged.

#### 5. Python surface

- `SagError.args` stays `(id, message, detail_json)`. The exception also gets read-only typed attributes, each
  taken from the top-level detail:
  - `id: str`
  - `message: str`
  - `detail: str`
  - `status: int | None`
  - `attempts: int | None`
  - `retry_after_ms: int | None`
- Batch and single result lines are unchanged in shape. `error` is the `ErrorDetail` JSON, so each level gains
  `status`, `attempts` and `retry_after_ms` when they are set. A failed decision (busy, quota, 401) stays data in its
  line, as today. `SagError` is for command errors.

#### 6. The error-id catalog, generated from the enums

- **Rust.** `sc-agent-types` gets a declarative macro, `error_catalog!`. Each crate lists, once per variant, the
  variant, its id and its category. The macro generates the `id()` body and a category match, both exhaustive
  matches inside the defining crate, so a new variant without a row fails to compile. It also generates
  `impl ErrorCatalog for E { const IDS: &'static [(&'static str, ErrorCategory)] }`. `decidr::error_catalog()`
  concatenates every workspace crate's `IDS`, plus `sc_agent_py.worker_panicked`.
- **Python.**
  - `sc_agent.ERROR_IDS: frozenset[str]` holds every id in the catalog.
  - `sc_agent.category(id: str, status: int | None = None) -> str` returns one of `busy`, `quota`, `auth`,
    `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log`, `other`.
- **Status refinement.** An id maps to one base category, so `category()` alone cannot tell a 401 `*.api_status`
  from a 500. When `status` is given and the id is `llm.api_status`, `decision.api_status` or `local.probe_status`,
  the status refines the base category:
  - 401 or 403 is `auth`.
  - 429 is `busy`.
  - Any other 4xx is `validation`.
  - Anything else keeps the base category.
- **Ids outside the catalog.** Foreign ids such as `io.ConnectionRefused` and `foreign.source` are open-ended and
  appear only as causes. They are not in `ERROR_IDS`, and `category()` returns `other` for them. Raising instead
  would crash a consumer that walks a chain. `id in ERROR_IDS` is the membership test.
- **Tests.**
  - A Rust test builds `error_catalog()` and checks that no id appears twice.
  - A Python test checks that `ERROR_IDS` equals the Rust catalog, which the module also exposes as
    `sc_agent._error_catalog()`, and that every id has a category.

#### 7. Per-call `workers` and `timeout`

The new options are carried by `decidr::SendOptions` (`workers: Option<NonZeroUsize>`,
`timeout: Option<Duration>`), so the Python binding and the CLI share one implementation and one validation:

| Surface | `workers` | `timeout` |
| --- | --- | --- |
| `sc_agent.send_batch` | `workers: int \| None = None` | `timeout: float \| None = None` |
| `sc_agent.send` | — (one request uses one worker) | `timeout: float \| None = None` |
| `decidr send` | `--workers N` | `--timeout SECS` |

The CLI flags keep the one-function-per-flag parity of ADR-SAG-0004.

- **Scope.** An override applies to that call only. It never writes to the configuration and never changes another
  call. `None` means the configured value.
- **`workers`.**
  - Must be an `int` (a `bool` is refused) in `1..=MAX_WORKERS` (64).
  - A value below the configured `workers` is honored, and so is any value above it, up to 64. Throttling a single
    call is a legitimate use, and the configured value is a default, not a floor.
  - The runner's queue capacity follows the effective worker count, as it does today.
- **`timeout`.**
  - It is the per-send HTTP request timeout (`Timeouts::request`): the whole send, including connect, headers and
    body. It must be finite and in `(0, 3600]` seconds (`Timeouts::MAX`).
  - It replaces 120 s, or a local runner's `request_timeout_secs`.
  - For a load-on-first-request runner (Laya), it replaces the request part of the per-request deadline, so the
    deadline becomes `load_timeout + timeout`.
  - The 10 s connect timeout is unchanged.
- **Timeout and the resend budget.**
  - Each send gets the full `timeout`, resends included.
  - A send that times out is `http.network` of kind `timeout`. It is resent under `max_sends` and the 90 s start
    budget (REQ-SAG-0601), as today.
  - The 90 s budget limits when a send may **start**, measured from the first send. It never cuts a send in flight.
  - The worst-case wall time of one request is therefore the start of its last allowed send (under 90 s plus that
    send's holdoff) plus `timeout`.
  - A `timeout` above 90 s is valid. It makes a timed-out send the last one, because the budget has run out by the
    time it ends.
- **Typed errors, no clamping.** An out-of-range or ill-typed value raises `SagError` with a new id,
  `decidr.invalid_option`, before any logger opens or anything is sent. The message names the option, the value and
  the allowed range. The CLI exits through the same error. Values are never clamped.
- **Injected transports.** A test context's injected transport has no request timeout to replace. The Rust tests
  check the effective `Timeouts` that decidr builds. The Python tests use the real loopback transport.

### Tests (to be written with the implementation)

- **Per variant.** For every status-, attempts- or retry-after-carrying variant above, a test builds the Rust error,
  takes its `detail()`, serializes it, and reads it back in Python through `SagError` or the result line. Each test
  asserts the exact field values.
- **Compatibility (NFR-SAG-0002).** A checked-in 0.1.0-shaped detail JSON, which has no new fields, deserializes
  into the 0.2.0 `ErrorDetail` with the fields `None`. A detail with no fields set serializes byte-identical to
  0.1.0.
- **`workers`.**
  - The loopback runner holds each request until the test releases it, and records the most requests ever in flight.
  - With `workers=2`, a batch of 6 must show a maximum of exactly 2 in flight. The bound cannot be exceeded under
    any interleaving, and reaching it is forced by holding the first request until the second arrives.
  - `workers=0`, `65` and `True` each raise `decidr.invalid_option`.
  - This uses no sleeps and no timing assertions.
- **`timeout`.**
  - With the loopback runner in `hang` mode and `max_sends = 1`, `timeout=0.2` returns a result line whose error
    chain ends in `http.network` of kind `timeout` with `attempts == 1`. The server never answers, so the outcome
    does not depend on timing.
  - `0`, `-1`, `nan`, `inf` and `3601` each raise `decidr.invalid_option`.
- **`ERROR_IDS` and `category`.**
  - `ERROR_IDS` equals the Rust catalog, and every id has a category.
  - The status refinements (401, 403, 429, 422, 500) return the categories listed in decision 6.
  - An unknown id returns `other`.

### Consequences

**Positive:**

- Consumers branch on `status`, `attempts`, `retry_after_ms` and `category()` instead of parsing messages or
  pinning id lists. judge-jev can delete its pinned id list, its `JEV.UNMAPPED` fallback, its `--timeout` refusal
  and its ignored `--max-workers`. That work is out of scope here; the CHANGELOG has a "Consumer migration"
  section for it.
- A missing catalog row is a compile error, not a runtime surprise.
- `#[non_exhaustive]` on `ErrorDetail` and `ErrorFields` makes future fields non-breaking.

**Negative:**

- This is a breaking 0.2.0 for Rust dependents. A struct literal of `ErrorDetail` must become builder calls.
- `retry_after_ms` is rounded up to whole milliseconds. The exact value remains in Rust and in the message.
- `id()` moves into a macro in every crate, a one-time mechanical diff across the 11 error enums.
