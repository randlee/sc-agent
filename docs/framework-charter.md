# sc-agent framework charter

**Status:** Active for agent-hosted Rust work  
**Audience:** Plan authors, QA reviewers, Rust developers using this repo's teams

sc-agent orchestrates **human and agent teams** that implement Rust software
using Synaptic Canvas conventions. When this repository (or a worktree) hosts
application crates, the same multi-consumer rules as other SC repos apply:

- Prefer **JSON-friendly CLI**, actionable errors, and thin boundaries
- **Framework instantiates; the project wires** — shared crates stay small;
  product logic lives in project-owned modules
- **Agent-first** — prompts, skills, and ATM assignments are first-class
  interfaces

For product-runtime framework details, see the **sc-runtime** charter in that
repository. sc-agent imports Rust QA and orchestration patterns from SC repos
but does not duplicate sc-runtime crate architecture here.
