# sc-agent requirements

**ID range:** REQ-SAG-0001+  
**Status:** Draft  
**Owner:** sc-agent maintainers

## Product purpose

Provide a maintained home for Synaptic Canvas agent teams, orchestration skills,
and supporting scripts used to build and ship Rust products (including future
crates hosted in this repository).

## REQ-SAG-0001 — Agent assets in repo

The repository MUST commit agent definitions under `.claude/agents/`, Cursor
agents under `.cursor/agents/`, and orchestration skills under
`.claude/skills/` and `.cursor/skills/`.

## REQ-SAG-0002 — Quality policy

The repository MUST maintain `.claude/project/quality-policy.md` as the
single source of reviewer selection, validation commands, and layout
expectations.

## REQ-SAG-0003 — ATM team configuration

The repository MUST declare the sc-ai team in `.atm.toml` with identities
documented in `docs/development/developer-roster.md`.

## REQ-SAG-0004 — Plan anchoring

The repository MUST ship plan-anchor settings under `.sc/plan-anchor/` and
plan-anchoring skills for configured harnesses.

## REQ-SAG-0005 — Rust workspace (future)

When Rust crates are added, they MUST follow `docs/architecture.md` and gain
crate-level requirements under `docs/<crate>/requirements.md`.
