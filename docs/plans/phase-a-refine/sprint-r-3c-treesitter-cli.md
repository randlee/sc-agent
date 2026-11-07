---
id: r-3c
phase: r
status: in-review # QA pending; every acceptance row green at the developer run (see Implementation notes)
closure_type: boundary
target_boundary: treesitter
track: producer
wave: 2
size: M
branch: sprint/r-3c-treesitter-cli
worktree: sprint/r-3c-treesitter-cli
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-1, r-4]
parallel_safe: [r-2, r-3]
requirements:
  - REQ-SAG-0712
  - REQ-SAG-0135
  - REQ-SAG-0104
  - REQ-SAG-0134
  - REQ-SAG-0305
  - REQ-SAG-0310
  - NFR-SAG-0001
  - NFR-SAG-0004
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0015
  - ADR-SAG-0017
  - ADR-SAG-0023
  - ADR-SAG-0027
  - ADR-SAG-0029
owned_paths:
  - "crates/sc-treesitter/Cargo.toml"
  - "crates/sc-treesitter/src/main.rs"
  - "crates/sc-treesitter/src/cli/**"
  - "crates/sc-treesitter/src/git_io.rs"
  - "crates/sc-treesitter/tests/**"
  - "boundaries/sc-treesitter/**"
  - "docs/sc-treesitter/**"
  # QA-1 fix round, parent-authorized cross-crate edits (RBQA-F006 close; no
  # other wave-2 sprint owns these files or items):
  - "crates/sc-agent-types/src/git.rs#hash_blob, MAX_INPUT_BYTES"
  - "crates/sc-agent-types/src/ids.rs#Rev::is_worktree"
  - "crates/sc-agent-types/tests/git.rs#hash_blob tests"
  - ".github/scripts/cargo-tree-gates.sh#sc-treesitter no-default-features feature row (r-4 merged)"
  - "tests/support/deadline.rs#unit-test wall-clock asserts (FTQ-002; written by r-1, no running owner)"
  - "clippy.toml#allow-invalid on the ResolvedApiKey::as_str entry (QA-001; the path is unreachable in sc-treesitter's default-features = false build, the all-features workspace clippy still enforces it)"
---

# Sprint r-3c — sc-treesitter clap and shared git read (D12 producer, RBQA-F006 caller)

Wave-2 ownership: r-1 (wave 1) migrated every `JevError` literal in
`main.rs`, `git_io.rs`, `grammar.rs` and `query.rs` to constructors; r-3c
starts from that tree and owns `main.rs`, `cli/**` and `git_io.rs` in wave 2.
`grammar.rs` and `query.rs` are not touched here. r-4 left `main.rs`
unchanged (`[[bin]] required-features = ["cli"]` in `Cargo.toml`).

## Outcome

The `sc-treesitter` binary parses argv with clap derive behind the `cli`
feature, reads git blobs through `sc_agent_types::git`, and its library links
only the `sc-agent-types` modules it uses (`default-features = false`). The
library stays I/O-free and clap-free. Split out of r-3b so it runs in wave 2
on files no other wave-2 sprint touches.

## Closes findings

ARCHP-011 (parser); contributes ARCHE-018/RBP-F012/EXT-010 (r-3b closes),
RBQA-F006 (producer caller), RBQA-F009 (`default-features = false`).

## Deliverables

1. **Cargo.toml.** `sc-agent-types = { ..., default-features = false }`;
   `cli = ["dep:mimalloc", "dep:clap", "sc-agent-types/git"]` (extends r-4's
   feature; `git` is non-default in `sc-agent-types` since r-1, so only
   `cli` turns it on here). The library target needs only `error`, `ids`,
   `envelope`, `schema`. `[lints] workspace = true` stays.
2. **clap.** `crates/sc-treesitter/src/cli/mod.rs`:
   `#[derive(Parser)] struct Cli` with the existing flag set, `--version`,
   values starting with `-` accepted via `allow_hyphen_values` on value flags.
   `try_parse_from`: `DisplayHelp`/`DisplayVersion` print and exit 0; every
   other error prints one `ScAgentErrorResponse` line on stderr with the code
   current tests expect and exits 2. Delete the hand-rolled parser and its
   flag tables. The a-10 argv edge cases are ported to
   `crates/sc-treesitter/tests/argv.rs`, declared in
   `crates/sc-treesitter/Cargo.toml` as
   `[[test]] name = "argv" required-features = ["cli"]` beside r-4's
   `[[test]] name = "cli"` entry, so `cargo hack --each-feature test` skips
   it when `cli` is off (CPR-207).
3. **git read.** `git_io.rs` `repo_root`, `hash_object` and the `git show`
   path are replaced by `sc_agent_types::git::{discover_toplevel, read_at_rev}`
   (compiled only with `cli`); producer error variants are the
   producer-neutral ones from r-1 (`producer = "sc-treesitter"`).
4. **Manifest.** `boundaries/sc-treesitter/treesitter.toml`:
   `allowed_dependencies += ["clap", "mimalloc"]` (optional), notes record
   `default-features = false` and that `git` is enabled only by `cli`.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:treesitter | No hand-rolled argv tables (deleted); `tests/argv.rs` edge cases green; `test_version_flag_exits_zero`; `test_rev_option_injection_rejected` (`--rev=--output=FILE` creates no file); `cargo build -p sc-treesitter --lib --no-default-features` green and its tree has no `clap`, `mimalloc`, `sc-observability`; `cargo hack` feature powerset green. Regression: REQ-SAG-0310 `cargo test -p sc-treesitter --test cli`; REQ-SAG-0134 the mimalloc `cargo tree` assertion below; NFR-SAG-0004 `bash .github/scripts/check-exact-pins.sh` (tree-sitter pins unchanged) |
| adr:ADR-SAG-0015 | Library still links no judge module: the asserted row below finds no `judge`, `secret`, `obs` or `git` feature of `sc-agent-types` in `cargo tree -p sc-treesitter --no-default-features -e normal,features -i sc-agent-types` (`-e normal,features` so a future dev-dependency cannot trip it, CPR-M208; r-3b adds the same row to `cargo-tree-gates.sh`) |
| adr:ADR-SAG-0029 | The clap parser and the shared git reads keep the 0029 (c) error-code dispositions: missing positional, leading-`-` revision, bad path or revision, and non-blob object are `INPUT_INVALID`; a blob or worktree file over 8 MiB is `INPUT_TOO_LARGE`; `io_failed`/`encode_failed` mapping unchanged. `cargo test -p sc-treesitter --test cli` keeps the existing code assertions green. |

## Required validation

```bash
just lint
just test
cargo test -p sc-treesitter --test argv --features cli
cargo test -p sc-treesitter --test argv --features cli -- test_version_flag_exits_zero --exact
cargo test -p sc-treesitter --test argv --features cli -- test_rev_option_injection_rejected --exact
cargo test -p sc-treesitter --test cli --features cli
cargo build -p sc-treesitter --lib --no-default-features
test -z "$(cargo tree -p sc-treesitter --no-default-features -e normal | grep -E '(^|[^[:alnum:]_-])(clap|mimalloc|sc-observability) v[0-9]')"
test -z "$(cargo tree -p sc-treesitter --no-default-features -e normal,features -i sc-agent-types | grep -E 'sc-agent-types feature "(judge|secret|obs|git|default)"')"
cargo hack check -p sc-treesitter --feature-powerset --no-dev-deps
cargo hack --each-feature test -p sc-treesitter
bash .github/scripts/check-exact-pins.sh
! rg -n '#!?\[allow\(' crates/
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## Implementation notes

Branch `sprint/r-3c-treesitter-cli`, one commit per deliverable (D1 `da64ff0`,
D2 `6911372`, D3 `0f8ffa8`, D4 `1b1f1cf`), then one commit per QA-1 item
(PR 25, reviewed at `39661a6`; listed below). Every "Required validation"
line, the workspace gates and `cargo test -p sc-agent-types --features
git,test-util` passed at the developer run on the fix-round head. PR 25 CI
evidence at `92b3d05` (fix-round code head): sc-agent CI run 42386842485
(test on ubuntu-24.04, macos-15 and windows-2022, python ×3, rustdoc,
dependency gates, sc-treesitter feature matrix, coverage) and sc-publish
dry-run run 42386842465, all green; the parent confirms the final head.
A local `cargo publish --dry-run` in a reused `target/` can compile a stale
cached `sc-agent-types 0.1.0` (cargo treats registry-sourced builds as
immutable); a fresh `CARGO_TARGET_DIR` passes, as CI does. r-3c owns no ADR text and flips no `(Proposed,
r-3c)` marker (none exists). Governing ADRs, all Active: ADR-SAG-0001, 0015,
0017, 0023, 0027, 0029. The `argv` and `cli` validation lines carry
`--features cli` (their `[[test]] required-features`; `cli` is also a
default feature, so the bare commands run the same targets, and both forms
passed).

**Deviations from the text above**, with the requirement or ADR each touches:

| Deviation | Touches | Why |
| --- | --- | --- |
| D3's shared git reads go further than the `owned_paths` above: `sc_agent_types::git` gains `hash_blob` and `MAX_INPUT_BYTES`, and `Rev` gains `is_worktree` (QA-1 additions to `owned_paths`) | RBQA-F006, ADR-SAG-0027 | parent-authorized in the QA-1 fix round (REQ-QA-R3C-001, RBP-F001, RBQA-F001, RBQA-F003). With them `git_io.rs` spawns no process: its `git_command` / `hash_bytes` copy and duplicated environment policy are deleted, so r-3b's `Command::new` ban needs no `git_io.rs` expect and no r-3b routing remains for `sc-treesitter` (REQ-QA-R3C-002). `sc-jev-check` keeps its own `MAX_INPUT_BYTES` until r-2 migrates it |
| `extract --language python --help` prints help and exits 0 (a-10 test `cli_help_only_in_the_first_two_slots` expected `INPUT_INVALID`) | REQ-SAG-0310, REQ-SAG-0701, NFR-SAG-0001; ARCHP-011, CR-016 | clap's `DisplayHelp` is exit 0 by D2. The a-10 intent, that a value such as `-h` is not swallowed as help, is kept by `allow_hyphen_values` (`test_hyphen_values_are_values_not_flags`) |
| No arguments is `INPUT_INVALID`, exit 2, one `ScAgentErrorResponse` line (was help, exit 0) | REQ-SAG-0701, NFR-SAG-0001, REQ-SAG-0310; ADR-SAG-0029 (c) (no row of its own; recorded in `docs/sc-treesitter/architecture.md`) | D2: only `DisplayHelp`/`DisplayVersion` exit 0 |
| A value parser's own `JevError` (`Rev::parse`, `--captures`) is returned unwrapped rather than re-wrapped by clap's message; both are `INPUT_INVALID` | ADR-SAG-0029 (c), NFR-SAG-0001 | one recovery text per error (`fix_hint`) |
| `cli_version_prints_name_and_version` and the help test moved from `tests/cli.rs` to `tests/argv.rs` (`test_version_flag_exits_zero`, `test_help_prints_usage_and_exits_zero`); the 0029 (c) code assertions (missing positional, injection, unresolvable revision, tree, oversize, outside paths) stay in `tests/cli.rs` | REQ-SAG-0310, REQ-SAG-0701, NFR-SAG-0001, ADR-SAG-0029 | D2 ports argv edge cases to `argv.rs` |
| Path validation uses `RepoRelativePath::try_from` plus a component normalizer; the local `validate_relative` and `object_id` copies are deleted (`BlobSha` validates ids) | RBP-F003, REQ-SAG-0108 | r-1 note: "`sc-treesitter`'s CLI path handling moves in r-3c" |
| `clippy.toml`'s `ResolvedApiKey::as_str` entry gets `allow-invalid = true` | ADR-SAG-0020, ADR-SAG-0029 | QA-001 (parent-authorized): with `default-features = false` the path is unreachable in a `-p sc-treesitter` clippy run, which warned; the `--all-features` workspace clippy still enforces it (its two `#[expect]` sites stay fulfilled) |
| The tree-sitter pins keep the inline-table form | NFR-SAG-0004, ADR-SAG-0023 | `src/extract.rs` (its asserting test) is outside this sprint's `owned_paths` |

**QA-1 fix round (PR 25).** CI-R3C-001: the Windows `test` job failed
because clap built usage from `argv[0]` (`sc-treesitter.exe extract`);
the `Cli` sets `bin_name`, and `usage_ignores_argv0` plus the argv help test
pin `Usage: sc-treesitter `. RBQA-F002: `.github/scripts/cargo-tree-gates.sh`
gains feature rows (parent-authorized edit of the r-4 script): the
`--no-default-features` tree of `sc-treesitter` must enable no `judge`,
`secret`, `obs`, `git` or `default` feature of `sc-agent-types`, and the
default tree must enable `git` (positive control); removing
`default-features = false` was checked to fail the row. FTQ-001: the test
helper `isolate_git` removes the hook-inherited git variables and sets
`GIT_CONFIG_SYSTEM` to the null device. FTQ-002: the shared
`tests/support/deadline.rs` unit tests assert outcomes instead of elapsed
time (parent-authorized).

**Deferral closed.** r-4's CPR-M212 clap/mimalloc boundary-manifest
deferral: `boundaries/sc-treesitter/treesitter.toml` now lists `clap` and
`mimalloc` (optional). r-3b owns the `sc-ai` half.

**a-10 item closed.** "RBP obs: ExtractInput required-field constructor"
(`docs/plans/phase-a/a-10-hardening-dispositions.md`, QA-1 E, deferred to
r-3c) is satisfied as is: `ExtractInput::new(language, query, source)`
(`crates/sc-treesitter/src/extract.rs:56`) takes the three required
fields, and `extract` rejects an empty `path`, `rev` or `blob_sha`
(REQ-QA-R3C-005). No code change.

## This sprint does not close

REQ-SAG-0712 and REQ-SAG-0135 as `req:` roots (r-3b closes both after the
`sc-ai` parser lands); the e2e run of this binary through `check.toml`
(r-3b `e2e_treesitter_producer`); the CI script clap rows (r-3b).
