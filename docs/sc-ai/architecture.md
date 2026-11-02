# sc-ai — architecture

**Boundary:** `sc-ai-cli`  
**Depends on:** `sc-jev`, `sc-agent-types`

Binary crate. Subcommand router → library calls → stdout JSON / stderr errors.

Follow Synaptic Canvas **sc-ai-cli** conventions (version field, operation
names, readback). No HTTP in this crate.
