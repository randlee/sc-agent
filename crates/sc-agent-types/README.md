# sc-agent-types

Shared value types for sc-agent, the bottom layer of the workspace (ADR-SAG-0004):

- the model catalog (`Catalog`, `ModelId`, `ShortName`, `ReleaseDate`, `MODELS`) and `ModelChoice`;
- `Provider` and its fixed ranking, separate from model ids (REQ-SAG-0103);
- `SessionId` (a ULID) and `VarName` (an environment-variable name);
- `CancelToken`, cooperative cancellation with deadline waits and wake hooks;
- `ErrorId` and `ErrorDetail`, the one error convention of the workspace (REQ-SAG-0002).

The crate does no I/O and depends only on `serde` and `ulid`.

License: MIT.
