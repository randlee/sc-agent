---
id: r-3c
phase: r
status: planned
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
   `crates/sc-treesitter/tests/argv.rs`.
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
| adr:ADR-SAG-0015 | Library still links no judge module: the asserted row below finds no `judge`, `secret`, `obs` or `git` feature of `sc-agent-types` in `cargo tree -p sc-treesitter --no-default-features -e features -i sc-agent-types` (r-3b adds the same row to `cargo-tree-gates.sh`) |

## Required validation

```bash
just lint
just test
cargo test -p sc-treesitter --test argv
cargo test -p sc-treesitter --test argv -- test_version_flag_exits_zero --exact
cargo test -p sc-treesitter --test argv -- test_rev_option_injection_rejected --exact
cargo test -p sc-treesitter --test cli
cargo build -p sc-treesitter --lib --no-default-features
test -z "$(cargo tree -p sc-treesitter --no-default-features -e normal | grep -E '(^|[^[:alnum:]_-])(clap|mimalloc|sc-observability) v[0-9]')"
test -z "$(cargo tree -p sc-treesitter --no-default-features -e features -i sc-agent-types | grep -E 'sc-agent-types feature "(judge|secret|obs|git|default)"')"
cargo hack check -p sc-treesitter --feature-powerset --no-dev-deps
bash .github/scripts/check-exact-pins.sh
! rg -n '#!?\[allow\(' crates/
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

REQ-SAG-0712 and REQ-SAG-0135 as `req:` roots (r-3b closes both after the
`sc-ai` parser lands); the e2e run of this binary through `check.toml`
(r-3b `e2e_treesitter_producer`); the CI script clap rows (r-3b).
