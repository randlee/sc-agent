# sc-ai — architecture

**Boundary:** `sc-ai-cli`  
**Depends on:** `sc-jev-check`, `sc-agent-types`, `sc-observability` (v2 logger only)

Binary crate. Subcommand router → **`sc-jev-check`** → stdout JSON / stderr errors.
No HTTP crates as **direct** dependencies (`cargo tree -p sc-ai --depth 1` must not
list `reqwest` / `hyper` / `ureq`).

Follow Synaptic Canvas **sc-ai-cli** conventions: success envelope
`{ "version", "operation", "data" }` where `data` is Jev answer JSON + `session_id`.
Stderr errors may include optional `docs` URL per sc-ai-cli (RBP-F001); Phase A
minimum remains `error` + `message` from `JevError`.
