# sitr

CLI for predefined tree-sitter queries. Business logic lives in the **`sc-treesitter`** crate (`sc_treesitter` library); PyPI bindings ship as the **`sitr`** project (`import sc_treesitter`).

```bash
cargo install sitr
sitr languages
sitr doctor
```

`sitr doctor` prints the resolved `project` and `log_root` settings and the existence of the
local and global `sitr.json` and `sc-agent.json` files. Like every command it
records one `sitr.command` event in its general log. Configuration uses the shared seven-layer lookup; `SITR_LOG_ROOT` sets sitr's log root,
`SC_PROJECT` shows the shared project/phase when configured, and `--log-root` overrides the
resolved root for one invocation.

Sitr's default general log is `<log_root>/logs/sitr.log.jsonl`, opened through `sc-agent-log`.
Sitr does not write session logs and does not require a project/phase to run.
