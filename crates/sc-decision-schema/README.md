# sc-decision-schema

The decision-layer contract of sc-agent: the TypeSafe System One API types and the two traits that every backend and the runner use. It sits above `sc-agent-types` and `sc-agent-http`, and below every decision backend and the runner. The crate does no I/O, holds no HTTP client, retry, cache or log code, and starts no runtime.

## What it provides

- **The one request form:** `SystemOneRequest` (POST /v1/systemone) with its `Question`s, `Asset`s and `State`. `DecisionRequest` wraps it with its image parts (`ImagePart`) and is valid by construction (`DecisionRequest::new`, or `DecisionRequest::try_from` a `SystemOneRequest`). `MAX_IMAGES` and `MAX_IMAGE_BYTES` are its limits.
- **The one response form:** `SystemOneResponse` with its `Answer`s, `Usage` and `BoxCoordinates`. Also `ModelMetadataList` (GET /v1/models) and the documented 400 and 422 bodies (`HttpBadRequestError`, `HttpValidationError`).
- **`Backend`:** the sans-IO translation each backend implements. `encode` builds one `HttpRequest`; `decode` turns the final `HttpResponse` into a `SystemOneResponse`.
- **`Decision`:** the sync `decide` and `decide_stream` API that the runner implements. It returns `DecisionResult`s.
- **`QuestionDigest`:** the per-question sc-sha digest of everything sent.
- **`DecisionError`:** the one error type, with stable `decision.*` ids, and `LimitKind`.
- `OrderedMap`: key order is kept, so a document round-trips.

## Usage

```rust,ignore
use sc_agent_types::CancelToken;
use sc_decision_schema::{Decision, DecisionRequest, SystemOneRequest};

let body = SystemOneRequest::from_json(
    r#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#,
)?;
let request = DecisionRequest::try_from(body)?;
assert_eq!(request.digests().len(), 1); // one digest per question

// `decision` is any `Decision`, such as a sc_decision_runner::DecisionRunner.
let result = decision.decide(request, &CancelToken::new());
match result.outcome {
    Ok(response) => println!("{} answers", response.answers.len()),
    Err(error) => eprintln!("failed: {error}"),
}
```

Numbers keep their exact value (`float_roundtrip`), so `0.123456789` comes back unchanged.

## Depends on

- `sc-agent-types` and `sc-agent-http`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). Backends implement `Backend`; the runner implements `Decision`. Neither is in this crate.

## Requirements and decisions

- REQ-SAG-0201: Jev API types live in sc-decision-schema.
- REQ-SAG-0202: question digest computed by sc-sha.
- REQ-SAG-0007: return values are not rounded.
- REQ-SAG-0014: input size limits.
- REQ-SAG-0006: no consumer-application types in shared crates.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0001: decision backends implement one trait with one request form.
- ADR-SAG-0004: crate layering and per-crate error unions.
