# sc-agent crates

Rust workspace members land here. Phase A adds the Jev-check stack and the
`sc-ai` CLI. **First release:** Rust crates on crates.io, **`sc-jev-py`** on PyPI, **`sc-ai`** CLI.

After Phase A, add **API-direct** Claude and Codex agent crates
(Jev comparison). Do **not** add subprocess wrappers for claude-code, codex-cli,
or cursor-cli.

**`sc-agent-types`** temporarily includes `observability::v2` (removed when upstream v2 ships).
**`sc-jev`** holds all TypeSafe Jev HTTP (100% API over time). See
`docs/plans/phase-a/plan.md` for boundaries and sprint order.
