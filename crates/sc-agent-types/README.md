# sc-agent-types

Shared error, judge, and `sc/subjects/1` types for the sc-agent workspace.

Every other sc-agent crate depends on these types: `JevError` and `ErrorCode`, the static backend catalog, the judge contract, canonical JSON bytes and digests, and the `sc/subjects/1` schema. Default features: `judge`, `secret`, `obs`; optional: `git`, `test-util`.

Part of the [sc-agent](https://github.com/randlee/sc-agent) workspace. See the
repository `docs/` for architecture and requirements. Licensed under MIT.
