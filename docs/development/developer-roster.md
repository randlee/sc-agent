# Developer roster (sc-ai)

ATM team **`sc-ai`** (see `.atm.toml`).

| Identity | Role | Model / harness |
| --- | --- | --- |
| `ai-lead` | Phase/sprint lead, orchestration | Claude Opus |
| `ai-qa` | Quality manager (QA gate) | Claude Sonnet |
| `tai` | Developer | Codex GPT-5.6 Terra |
| `lai` | Developer | Codex GPT-5.6 Luna |
| `sai` | Developer | Codex GPT-5.6 Sol |

Dispatch templates use `lead=ai-lead`. QA assignments target **`ai-qa`**, not
`quality-mgr`, while still loading `.claude/agents/quality-mgr.md`.

Sprint docs may name a preferred developer (`tai`, `lai`, or `sai`) in
`recommended_agent`; the lead substitutes only an idle peer of the same or
higher tier.
