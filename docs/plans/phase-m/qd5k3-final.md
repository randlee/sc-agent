# agt-qd5k.3 final: phase-m planning gate

11 of 12 sprint beads pass. agt-m-7 fails on ANC-008 only (relevant no 0.30, addressed yes 0.80); the anchor-tuning agent (tune-ANC-008-planning-2) found no false result in the sealed questions, so it goes to the owner as it is. The tuner notes that neither oracle has a case where a method takes an existing grouping struct.

agt-m-3 and agt-m-4: fresh attested re-runs (110241Z, 110239Z) gave the same verdicts as 103304Z and 103234Z; the gate agents did not keep their original request envelopes. A second fresh m-3 run, plan-20261128T110224Z (17 s before 110241Z, no attestation found), failed ANC-002 as underspecified (relevant no 0.58); both are kept, and the owner decides whether m-3 stands. agt-m-1: answer_schema moved from sc-agent-schema to sc-agent-types (lai, 09:56Z, ANC-025; owner review pending).

Final runs:
- plan-20261128T085006Z-agt-m-0.json
- plan-20261128T095936Z-agt-m-1.json
- plan-20261128T094559Z-agt-m-2.json
- plan-20261128T110241Z-agt-m-3.json
- plan-20261128T110239Z-agt-m-4.json
- plan-20261128T094926Z-agt-m-5.json
- plan-20261128T094933Z-agt-m-6.json
- plan-20261128T095135Z-agt-m-7.json
- plan-20261128T095144Z-agt-m-8.json
- plan-20261128T095745Z-agt-m-9.json
- plan-20261128T101428Z-agt-m-10.json
- plan-20261128T101549Z-agt-m-11.json

ANC-007 is deterministic only (checked at sprint end). ANC-003 and ANC-004: no sprint touches them. ANC-010, 011, 015, 019, 031: no phase-m sprint carries them; listed for the owner.

## Anchor coverage (plan)

| Anchor | agt-m-0 | agt-m-1 | agt-m-10 | agt-m-11 | agt-m-2 | agt-m-3 | agt-m-4 | agt-m-5 | agt-m-6 | agt-m-7 | agt-m-8 | agt-m-9 | Covered |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ANC-001 | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | excluded | excluded | excluded | satisfied | not applicable | not applicable | yes |
| ANC-002 | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | yes |
| ANC-003 | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | no |
| ANC-004 | not applicable | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | no |
| ANC-005 | not applicable | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | yes |
| ANC-006 | not applicable | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | yes |
| ANC-007 | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | deterministic only | no |
| ANC-008 | not applicable | excluded | excluded | not applicable | not applicable | satisfied | satisfied | excluded | not applicable | underspecified | not applicable | satisfied | yes |
| ANC-009 | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | not applicable | yes |
| ANC-010 | not applicable | not applicable | not applicable | excluded | excluded | not applicable | not applicable | not applicable | not applicable | excluded | excluded | not applicable | no |
| ANC-011 | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | excluded | excluded | not applicable | not applicable | no |
| ANC-012 | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | satisfied | excluded | not applicable | excluded | not applicable | excluded | yes |
| ANC-013 | not applicable | excluded | satisfied | excluded | not applicable | not applicable | satisfied | excluded | not applicable | not applicable | not applicable | excluded | yes |
| ANC-014 | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | satisfied | excluded | not applicable | not applicable | not applicable | not applicable | yes |
| ANC-015 | excluded | excluded | excluded | excluded | excluded | excluded | excluded | excluded | excluded | excluded | excluded | excluded | no |
| ANC-016 | not applicable | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | yes |
| ANC-017 | excluded | satisfied | satisfied | excluded | excluded | satisfied | not applicable | satisfied | satisfied | satisfied | satisfied | satisfied | yes |
| ANC-018 | excluded | not applicable | excluded | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | yes |
| ANC-019 | excluded | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | no |
| ANC-020 | not applicable | not applicable | not applicable | excluded | not applicable | satisfied | not applicable | not applicable | satisfied | satisfied | excluded | not applicable | yes |
| ANC-021 | satisfied | satisfied | satisfied | excluded | satisfied | satisfied | satisfied | satisfied | satisfied | satisfied | satisfied | satisfied | yes |
| ANC-022 | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | excluded | excluded | not applicable | satisfied | not applicable | not applicable | yes |
| ANC-023 | not applicable | not applicable | excluded | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | yes |
| ANC-024 | not applicable | not applicable | excluded | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | yes |
| ANC-025 | satisfied | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | yes |
| ANC-026 | excluded | excluded | not applicable | not applicable | excluded | excluded | not applicable | not applicable | not applicable | excluded | satisfied | not applicable | yes |
| ANC-027 | not applicable | not applicable | excluded | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | yes |
| ANC-028 | not applicable | not applicable | excluded | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | satisfied | not applicable | yes |
| ANC-029 | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | yes |
| ANC-030 | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | satisfied | yes |
| ANC-031 | not applicable | not applicable | excluded | not applicable | not applicable | excluded | not applicable | not applicable | not applicable | not applicable | excluded | not applicable | no |
| ANC-032 | excluded | excluded | excluded | excluded | excluded | excluded | not applicable | not applicable | excluded | excluded | satisfied | not applicable | yes |
| ANC-033 | not applicable | not applicable | satisfied | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | not applicable | yes |

Uncovered anchors (no item satisfies them): ANC-003, ANC-004, ANC-007, ANC-010, ANC-011, ANC-015, ANC-019, ANC-031

## Exclusions (anchor, bead, reason, named carriers and their state)
- ANC-001 x agt-m-2: This sprint relocates sc-agent-config without changing model resolution; agt-m-7 owns the Jev model selection behavior.  [carriers: agt-m-7=satisfied]
- ANC-001 x agt-m-4: m-4 moves the API-shape implementations and changes no model table, model resolution or jev alias; agt-m-7 carries ANC-001.  [carriers: agt-m-7=satisfied]
- ANC-001 x agt-m-5: m-5 moves the runner unchanged and consumes the existing model table; it changes no jev default or model-resolution rule; agt-m-7 carries ANC-001.  [carriers: agt-m-7=satisfied]
- ANC-001 x agt-m-6: m-6 removes the old shell and rewires imports without changing the model table or jev resolution; agt-m-7 carries model-default configuration.  [carriers: agt-m-7=satisfied]
- ANC-004 x agt-m-11: This integration sprint does not change request input-size limits.  [carriers: none]
- ANC-005 x agt-m-8: m-8 changes session storage and creates no production default general logger; agt-m-3 carries default logger construction and paths.  [carriers: agt-m-3=satisfied]
- ANC-008 x agt-m-1: M-1 preserves existing parameter lists while moving code; signature grouping applies to the Settings API changes owned by agt-m-7.  [carriers: agt-m-7=underspecified]
- ANC-008 x agt-m-5: m-5 moves existing Run/RunArgs and their signatures unchanged rather than introducing repeated parameters; typed settings and parameter restructuring are carried by agt-m-7 and agt-m-9.  [carriers: agt-m-7=underspecified, agt-m-9=satisfied]
- ANC-008 x agt-m-10: m-10 has no related parameters repeated across several methods in its stated LlmRun signatures; RunArgs groups run setup.  [carriers: none]
- ANC-009 x agt-m-2: This sprint relocates the existing model table unchanged; configuration-supplied model rows are added by agt-m-7.  [carriers: agt-m-7=satisfied]
- ANC-010 x agt-m-2: This sprint changes sc-agent configuration code only and does not change judge-jev, plan-anchor, or their shared client template.  [carriers: none]
- ANC-010 x agt-m-7: m-7 changes Rust configuration rows and Settings, not code in judge-jev, plan-anchor or their client template; this consumer-code anchor has no carrying change in this sprint.  [carriers: none]
- ANC-010 x agt-m-8: m-8 changes sc-agent core session persistence, not code in judge-jev, plan-anchor or their client template; this consumer-code anchor has no carrying change in this sprint.  [carriers: none]
- ANC-010 x agt-m-11: no sprint in phase m carries this anchor; listed for the owner.  [carriers: none]
- ANC-011 x agt-m-6: no sprint in phase m carries this anchor; listed for the owner  [carriers: none]
- ANC-011 x agt-m-7: no sprint in phase m carries this anchor; listed for the owner  [carriers: none]
- ANC-012 x agt-m-5: m-5 moves the runner unchanged and delegates to existing Decision implementations; the wire-format implementation inventory is carried by agt-m-4.  [carriers: agt-m-4=satisfied]
- ANC-012 x agt-m-7: m-7 changes provider and model configuration rows, not the wire-format Decision implementations; agt-m-4 carries the single implementation per API shape.  [carriers: agt-m-4=satisfied]
- ANC-012 x agt-m-9: This HTTP transport sprint does not implement decision provider traits; agt-m-4 carries the decision wire-format implementations.  [carriers: agt-m-4=satisfied]
- ANC-013 x agt-m-1: This item moves the shared Llm trait and JSON-schema helper but changes no non-Jev LLM call; m-4 owns the wire-format implementations and call behavior.  [carriers: none]
- ANC-013 x agt-m-5: m-5 is the decision runner and adds no non-Jev LLM interface; agt-m-10 carries the shared Llm API.  [carriers: agt-m-10=satisfied]
- ANC-013 x agt-m-9: m-9 changes retry and holdoff parameters in sc-agent-http; it adds or changes no LLM interface. agt-m-10 carries the shared Llm API.  [carriers: agt-m-10=satisfied]
- ANC-013 x agt-m-11: This integration sprint does not change the LLM trait or provider wire formats; agt-m-4 carries the shared implementations.  [carriers: agt-m-4=satisfied]
- ANC-014 x agt-m-5: m-5 moves the existing prompt lookup unchanged; prompt defaults and overrides remain in the API-shape move agt-m-4 and configured settings agt-m-7.  [carriers: agt-m-4=satisfied, agt-m-7=not_applicable]
- ANC-015 x agt-m-0: m-0 adds or changes no REQ, NFR or ADR record of sc-agent, judge-jev, plan-anchor or any other tool; it only cites existing records.  [carriers: none]
- ANC-015 x agt-m-1: This sprint changes crate code and a CLI test import, not requirement, NFR, or ADR records for derivative tools.  [carriers: none]
- ANC-015 x agt-m-2: This sprint moves configuration code and adds or changes no REQ, NFR, or ADR records for derivative tools.  [carriers: none]
- ANC-015 x agt-m-3: agt-m-3 moves only sc-agent logging and digest code and changes no REQ, NFR, or ADR records for derivative tools; derivative-record edits are not assigned to agt-m-3.  [carriers: none]
- ANC-015 x agt-m-4: agt-m-4 moves sc-agent transport, types, and provider implementations but changes no REQ, NFR, or ADR records for derivative tools; derivative-record edits are not assigned to agt-m-4.  [carriers: none]
- ANC-015 x agt-m-5: m-5 changes Rust runner code, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this move.  [carriers: none]
- ANC-015 x agt-m-6: m-6 changes crate imports, workspace membership and release inputs, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this move.  [carriers: none]
- ANC-015 x agt-m-7: m-7 changes configuration code, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this change.  [carriers: none]
- ANC-015 x agt-m-8: m-8 changes core session persistence, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this change.  [carriers: none]
- ANC-015 x agt-m-9: This transport sprint changes no cross-tool requirements or derivative-tool records.  [carriers: none]
- ANC-015 x agt-m-10: This LLM runtime sprint changes no cross-tool requirements or derivative-tool records.  [carriers: none]
- ANC-015 x agt-m-11: This integration sprint changes no cross-tool requirements or derivative-tool records.  [carriers: none]
- ANC-017 x agt-m-0: m-0 adds empty crates, boundary records and a CI lint step; it adds or changes no test.  [carriers: none]
- ANC-017 x agt-m-2: This sprint relocates the existing tests unchanged; deterministic test changes for the model configuration work are owned by agt-m-7.  [carriers: agt-m-7=satisfied]
- ANC-017 x agt-m-11: m-11 adds or changes no test; it runs the existing tests against live agents, which ANC-018 covers.  [carriers: none]
- ANC-018 x agt-m-0: m-0's only CI change is a static sc-lint boundary job; it adds no live-provider test, live-run selection or workflow_dispatch job.  [carriers: none]
- ANC-018 x agt-m-10: m-10 adds LLM and resume tests but changes no live-agent integration run; agt-m-11 carries that run.  [carriers: agt-m-11=satisfied]
- ANC-019 x agt-m-0: m-0 does not edit requires-python, Python version checks, CI Python matrices or interpreter guards; its python3 commands only run the sc-lint bootstrap and the gate script.  [carriers: none]
- ANC-019 x agt-m-11: requires-python is already >=3.11 (crates/sc-agent-py/pyproject.toml:11); m-11 changes no Python version, check or matrix.  [carriers: none]
- ANC-020 x agt-m-8: m-8 changes session storage assertions but no injected general Logger construction path; agt-m-6 carries unchanged wheel logger wiring.  [carriers: agt-m-6=satisfied]
- ANC-020 x agt-m-11: m-11 adds or changes no code in any crate and no injected-logger path; agt-m-6 carries the wheel logger wiring.  [carriers: agt-m-6=satisfied]
- ANC-021 x agt-m-11: m-11 adds or changes no code in any crate under crates/; it runs integration tests, lands the phase and moves the size limits to sc-lint.  [carriers: none]
- ANC-022 x agt-m-2: This sprint relocates configuration code without changing local-model resolution; agt-m-7 owns the local-model configuration behavior.  [carriers: agt-m-7=satisfied]
- ANC-022 x agt-m-4: m-4 moves the API-shape implementations and adds no model row, local-model URL or model id; agt-m-7 carries local-model configuration.  [carriers: agt-m-7=satisfied]
- ANC-022 x agt-m-5: m-5 consumes model resolution unchanged and defines no local-model row, URL or id; agt-m-7 carries local-model configuration.  [carriers: agt-m-7=satisfied]
- ANC-023 x agt-m-3: agt-m-3 only extracts the existing JSONL session-row writer unchanged; agt-m-8 owns replacing that writer with SQLite persistence and removing sessions.jsonl.  [carriers: agt-m-8=satisfied]
- ANC-023 x agt-m-10: This sprint consumes sessions and does not change session persistence; agt-m-8 carries SQLite session writes.  [carriers: agt-m-8=satisfied]
- ANC-024 x agt-m-8: m-8 changes session persistence and no general-log rotation or retention rule; agt-m-3 carries general-log writing through sc-observability.  [carriers: agt-m-3=satisfied]
- ANC-024 x agt-m-10: m-10 changes no general log rotation or deletion policy; the existing sc-observability policy remains in force.  [carriers: none]
- ANC-026 x agt-m-0: m-0 adds no SQL table definition anywhere; sc-agent-schema is an empty crate in this sprint.  [carriers: none]
- ANC-026 x agt-m-1: This sprint moves request/answer types and defines no SQL table; table SQL is owned by agt-m-8.  [carriers: agt-m-8=satisfied]
- ANC-026 x agt-m-2: This sprint adds or changes no SQL table definitions; the table-definition work is owned by agt-m-8.  [carriers: agt-m-8=satisfied]
- ANC-026 x agt-m-3: agt-m-3 moves logging and digest code but adds or changes no table SQL; agt-m-8 owns the schema definitions.  [carriers: agt-m-8=satisfied]
- ANC-026 x agt-m-7: m-7 defines provider and model configuration rows, not SQL tables; agt-m-8 carries core SQL ownership in sc-agent-schema.  [carriers: agt-m-8=satisfied]
- ANC-027 x agt-m-3: agt-m-3 only extracts the existing JSONL session-row writer unchanged; agt-m-8 owns the messages and responses schema and persistence.  [carriers: agt-m-8=satisfied]
- ANC-027 x agt-m-10: This sprint consumes sessions and does not define their tables; agt-m-8 carries the message and response schema.  [carriers: agt-m-8=satisfied]
- ANC-028 x agt-m-3: agt-m-3 extracts the existing session-row writer and does not change cache behavior; agt-m-8 owns the cache key and lookup.  [carriers: agt-m-8=satisfied]
- ANC-028 x agt-m-10: This sprint does not change cache-key definition; agt-m-8 carries the session cache.  [carriers: agt-m-8=satisfied]
- ANC-030 x agt-m-4: agt-m-4 moves the existing resend and holdoff behavior unchanged; agt-m-9 owns parameterized waits, limits, and busy/quota rules.  [carriers: agt-m-9=satisfied]
- ANC-031 x agt-m-3: agt-m-3 only extracts the existing session JSONL writer; no sprint in phase m carries ANC-031 derivative-tool result storage, so this anchor remains listed for the owner.  [carriers: none]
- ANC-031 x agt-m-8: m-8 moves only sc-agent's own session log to SQLite and adds Db::execute; it changes no judge-jev, plan-anchor or other tool's storage. No phase-m sprint carries ANC-031 (coverage gap, waiting: owner).  [carriers: none]
- ANC-031 x agt-m-10: no sprint in phase m carries this anchor; listed for the owner.  [carriers: none]
- ANC-032 x agt-m-0: m-0 adds no table definition and no schema version, and does not touch crates/sc-agent-py.  [carriers: none]
- ANC-032 x agt-m-1: This sprint moves request/answer types and changes no table definitions or schema versions; sc-agent-py table/schema work belongs to agt-m-8.  [carriers: agt-m-8=satisfied]
- ANC-032 x agt-m-2: This sprint adds or changes no table definitions, schema versions, or wheel code; those changes are owned by agt-m-8.  [carriers: agt-m-8=satisfied]
- ANC-032 x agt-m-3: m-3 defines no table and no schema version and does not touch crates/sc-agent-py; agt-m-8 carries tool schema ownership and versions.  [carriers: agt-m-8=satisfied]
- ANC-032 x agt-m-6: m-6 changes wheel imports only and defines no SQL tables or schema versions; agt-m-8 carries core-versus-tool schema ownership and versions.  [carriers: agt-m-8=satisfied]
- ANC-032 x agt-m-7: m-7 repoints provider_keys and defines no wheel SQL tables or tool schema versions; agt-m-8 carries core-versus-tool schema ownership and versions.  [carriers: agt-m-8=satisfied]
- ANC-032 x agt-m-10: m-10 defines no SQL table and no schema version and does not touch crates/sc-agent-py; agt-m-8 carries tool schema ownership and versions.  [carriers: agt-m-8=satisfied]
- ANC-032 x agt-m-11: This integration sprint changes no table or schema definition; agt-m-8 carries session schema work.  [carriers: agt-m-8=satisfied]
