# sc-agent architecture

**Status:** Draft  
**Owner:** sc-agent maintainers

## System shape

sc-agent is an **agent-orchestration repository**, not a runtime daemon.

| Layer | Location | Role |
| --- | --- | --- |
| Agents | `.claude/agents/`, `.cursor/agents/`, `.codex/agents/` | Role prompts for QA, Rust work, publishing, anchoring |
| Skills | `.claude/skills/`, `.cursor/skills/` | Orchestration, plan hardening, plan anchoring, publishing |
| Policy | `.claude/project/quality-policy.md` | Reviewer sets, validation, layout |
| Rust QA assets | `.claude/assets/sc-rust/` | Templates for rust-qa / RBP / RSH reviewers |
| Beads | `.beads/` | Dolt-backed issue tracking (`bd`) |
| Plan anchor | `.sc/plan-anchor/` | Tracker settings and install lock |
| Release surface | `release/`, `.github/scripts/` | Manifest-driven publishing when enabled |

## ADRs

No binding ADRs yet. Add `## ADR-SAG-nnnn:` sections here when decisions affect
multiple sprints.

## Related documents

- [`design.md`](design.md) — goals and non-goals
- [`framework-charter.md`](framework-charter.md) — multi-consumer posture for generated Rust
- [`requirements.md`](requirements.md) — REQ-SAG ids
