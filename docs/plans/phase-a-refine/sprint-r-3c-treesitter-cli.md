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
D2 `6911372`, D3 `0f8ffa8`, D4 `1b1f1cf`). Every "Required validation" line,
the workspace gates and `cargo test -p sc-agent-types --features git,test-util`
passed at `1b1f1cf` (developer run; CI evidence is added by QA). r-3c owns no ADR text and flips
no `(Proposed, r-3c)` marker (none exists). Governing ADRs, all Active:
ADR-SAG-0001, 0015, 0017, 0023, 0027, 0029. The `argv` and `cli` validation lines
carry `--features cli` (their `[[test]] required-features`; `cli` is also a
default feature, so the bare commands run the same targets, and both forms
passed).

**Deviations from the text above**, with the requirement or ADR each touches:

| Deviation | Touches | Why |
| --- | --- | --- |
| The `WORKTREE` read keeps a local `git hash-object -t blob --stdin --no-filters` call in `git_io.rs` (D3 names `hash_object` among the replaced paths) | RBQA-F006, D3; r-3b `Command::new` clippy entry | `sc_agent_types::git` has no hashing helper and `crates/sc-agent-types/**` is outside this sprint's `owned_paths`. The call uses the shared module's environment policy (`LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/`GIT_OBJECT_DIRECTORY`/`GIT_ALTERNATE_OBJECT_DIRECTORIES` removed). Follow-up below |
| `extract --language python --help` now prints help and exits 0 (a-10 test `cli_help_only_in_the_first_two_slots` expected `INPUT_INVALID`) | ARCHP-011, CR-016 | clap's `DisplayHelp` is exit 0 by D2. The a-10 intent, that a value such as `-h` is not swallowed as help, is kept by `allow_hyphen_values` (`test_hyphen_values_are_values_not_flags`) |
| No arguments is `INPUT_INVALID` exit 2 (was help, exit 0) | ARCHP-011, ADR-SAG-0029 (c) | D2: only `DisplayHelp`/`DisplayVersion` exit 0 |
| A value parser's own `JevError` (`Rev::parse`, `--captures`) is returned unwrapped rather than re-wrapped by clap's message; both are `INPUT_INVALID` | ADR-SAG-0029 (c) | one recovery text per error (`fix_hint`) |
| `cli_version_prints_name_and_version` and the help test moved from `tests/cli.rs` to `tests/argv.rs` (`test_version_flag_exits_zero`, `test_help_prints_usage_and_exits_zero`); the 0029 (c) code assertions (missing positional, injection, unresolvable revision, tree, oversize, outside paths) stay in `tests/cli.rs` | REQ-SAG-0310, ADR-SAG-0029 | D2 ports argv edge cases to `argv.rs` |
| Path validation uses `RepoRelativePath::try_from` plus a component normalizer; the local `validate_relative` and `object_id` copies are deleted (`BlobSha` validates ids) | RBP-F003, REQ-SAG-0108 | r-1 note: "`sc-treesitter`'s CLI path handling moves in r-3c" |
| `cargo clippy -p sc-treesitter` alone prints clippy's config warning that `sc_agent_types::ResolvedApiKey::as_str` is unreachable (exit 0) | ADR-SAG-0029, `clippy.toml` | with `default-features = false` the `secret` module is not compiled for this package alone; workspace clippy (`--workspace`) is clean. `clippy.toml` is outside `owned_paths` |
| The tree-sitter pins keep the inline-table form | NFR-SAG-0004, ADR-SAG-0023 | `src/extract.rs` (its asserting test) is outside this sprint's `owned_paths` |

**Deferral closed.** r-4's CPR-M212 clap/mimalloc boundary-manifest
deferral: `boundaries/sc-treesitter/treesitter.toml` now lists `clap` and
`mimalloc` (optional). r-3b owns the `sc-ai` half.

**Not done here (outside `owned_paths`).** The a-10 "RBP obs: ExtractInput
required-field constructor" item deferred to r-3c touches
`crates/sc-treesitter/src/extract.rs`, which this sprint does not own;
`ExtractInput::new(language, query, source)` already takes the required
fields. Recommended follow-ups: a `sc_agent_types::git` blob-hash helper
(owner of `crates/sc-agent-types/src/git.rs`) so the last `git` spawn in
`sc-treesitter` moves before r-3b's `Command::new` clippy entry, or r-3b's
item-level expect on `git_io::git_command`; `allow-invalid = true` on the
`clippy.toml` `as_str` entry if per-package clippy is gated.

## This sprint does not close

REQ-SAG-0712 and REQ-SAG-0135 as `req:` roots (r-3b closes both after the
`sc-ai` parser lands); the e2e run of this binary through `check.toml`
(r-3b `e2e_treesitter_producer`); the CI script clap rows (r-3b).
