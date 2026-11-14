# sc-agent-types

Shared value types for sc-agent. This is the bottom layer of the workspace (ADR-SAG-0004): every other sc-agent crate builds on it, and it depends on none of them.

The crate does no I/O, reads no files and needs no runtime. Its only dependencies are `serde` and `ulid`.

## What it provides

- **Model catalog:** `Catalog`, `ModelId`, `ShortName`, `ReleaseDate`, `ModelDescriptor` and the built-in `MODELS`. `ModelChoice` is how a caller picks a model: `Latest(ShortName)` or `Version(ModelId)`.
- **Providers:** `Provider`, with a fixed priority ranking (`Provider::PRIORITY`, `Provider::ranking`). Providers are separate from model ids.
- **Identifiers:** `SessionId` (a ULID) and `VarName` (a checked environment-variable name).
- **Cancellation:** `CancelToken`, cooperative cancel with deadline waits and wake hooks (`on_cancel`, `CancelHook`, `Wait`).
- **Time:** `Clock` (`Clock::system()`, `now`, `wait_until`) and `SystemClock`. It is the one source of time and of timed waits.
- **Errors:** the `ErrorId` trait and `ErrorDetail` (id, message, optional recovery, HTTP status and cause), the one error convention of the workspace, and the crate's own `TypesError`.

## Usage

```rust,ignore
use sc_agent_types::{CancelToken, Catalog, ModelChoice, Provider, ShortName};

let catalog = Catalog::builtin();
let choice: ModelChoice = "haiku".parse()?;
let model = catalog.resolve(&choice); // the newest Haiku in the catalog

let order = Provider::ranking(Some(ShortName::Haiku));

let cancel = CancelToken::new();
cancel.cancel();
assert!(cancel.is_cancelled());
```

## Features

- `testing` (not default): the `testing` module with `ManualClock` and the `CancelEvent` trace points of `CancelToken`. It adds no dependency. A test that needs time to pass moves a `ManualClock` instead of waiting.

## Depends on

No other sc-agent crate. Layer rule: a crate depends only on lower layers (ADR-SAG-0004), so nothing may be added below this one.

Dev only: `sc-agent-testkit`.

## Requirements and decisions

- REQ-SAG-0101: model short names.
- REQ-SAG-0102: model catalog, built-in models plus global configuration.
- REQ-SAG-0103: provider and model are separate.
- REQ-SAG-0104: cancel token.
- REQ-SAG-0001: created ids are ULIDs.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- REQ-SAG-0006: no consumer-application types in shared crates.
- ADR-SAG-0004: crate layering and per-crate error unions.

License: MIT.
