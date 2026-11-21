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
- `sc-config` is a git dependency pinned to sc-runtime `eb293db` (feature/sc-config-layers), because the `layers` module is not published yet. Replace it with the crates.io version once that release exists.
- `clippy.toml` drops the `ApiKey::as_str` entry, since there is no ApiKey newtype.
