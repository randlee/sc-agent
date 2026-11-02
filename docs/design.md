# sc-agent design

**Status:** Draft  
**Owner:** sc-agent maintainers

## Purpose

**sc-agent** is the Synaptic Canvas repository for AI team assets: Claude/Codex/Cursor
agents, orchestration skills, beads integration, and plan anchoring for Rust
development workflows.

## Goals

- Version-controlled agent prompts and skills shared across harnesses
- ATM team **sc-ai** (`ai-lead`, `ai-qa`, `tai`, `lai`, `sai`) driving sprint work
- Plan anchoring and plan hardening compatible with sibling SC repos
- Rust QA via installed `sc-rust` reviewer assets under `.claude/assets/sc-rust/`

## Non-goals

- Replacing product runtime code in `sc-runtime`
- Embedding host-specific paths or secrets in committed agent text

## Principles

1. **Repo-local truth** — policy lives in `.claude/project/quality-policy.md` and `docs/`.
2. **Portable paths** — skills and agents reference paths relative to the repository root.
3. **Thin repo, rich agents** — orchestration and QA contracts live in `.claude/` and `.cursor/`.
