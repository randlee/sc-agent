# phase-h starting code

The sc-agent rework's starting point, split into the existing crate names.
Built from the session's minimal-path scratch. At this commit:
`cargo clippy --workspace --all-targets -- -D warnings` is clean and the 21 tests
pass on each of 5 runs.

h-0 moves the repo's `crates/` to `crates-ref/` and copies `crates/*` from here.
Each phase-h sprint then ports its crate's behaviour tests from `crates-ref/` and closes
the crate's record gaps.

Not for merging: this branch shares no history with develop.

Known gaps:
- `sc-config` is a local path dependency. The `layers` module is not published yet (needed by h-6 and decidr).
- `clippy.toml` drops the `ApiKey::as_str` entry, since there is no ApiKey newtype.
