# Phase A.1 plan review round 4 — dispositions (wave-2 ADR pre-screen)

Input: arch-qa pre-screen of `integrate/phase-a` @ `34dfed0`,
`.cursor/phase-a-orchestration/wave8-dev/wave2-adr-prescreen.json`. It
screens r-2, r-3 and r-3c before wave 2 dispatch, with a frontmatter-only
look at r-2b and r-3b. It is meant to stop a repeat of the r-1 QA-1 pattern
(ARCH-R1-001..004 in
`.cursor/phase-a-orchestration/wave7-qa/r-1-qa1/arch-qa.json`): code shapes
that only another sprint's Proposed ADR text governs, and `adrs` lists that
leave out ADRs governing owned paths. Also applied: req-qa REQ-QA-R1-005
(D0 outcome 4 in plan "Test seams").

`docs/architecture.md` is not edited in this revision, because r-1 and r-4
have open PRs that touch it. Where the pre-screen asks for an ADR text
change, the exact replacement wording is a deliverable in the sprint doc
that owns the subsection. That sprint applies it when it flips the
subsection. Abbreviations: `plan` = `plan.md`, `r-N` = that sprint doc.

## Dispositions

| Item | Disposition | Where |
| --- | --- | --- |
| Global precondition: wave-1 flips (ADR-SAG-0027, 0023 r-4 part; r-1 subsections of 0009, 0016, 0020, 0022) not visible at `34dfed0` | Accepted as a dispatch gate, with no doc change. Wave 2 dispatches only after r-1 and r-4 merge with those flips. r-2 and r-3 mark ADR-SAG-0023 "binding once r-4 has made its r-4 part Active" | r-2 / r-3 `adrs` comments |
| Open r-1 blocker (ARCH-R1-001/002): answers, EqualInputRecord/Attestation/MultiBackendMode and session v2 shapes have no Active text | Applied on the r-2 side. r-2 `adrs` names "the r-1 interim amendment of ADR-SAG-0008/0014/0019 (Active, r-1)", which the r-1 fix round is adding. r-2 may not dispatch until it is Active | r-2 `adrs`; plan "No shape on another sprint's Proposed text" |
| r-2: `RunConfig::mode` is governed only by Proposed ADR-SAG-0025 | Applied, option 1 (remove). No r-2 deliverable reads a mode, so the setter moves to r-2b, which adds it with ADR-SAG-0025 | r-2 D2.1 (setter removed, note added); r-2b deliverable 1; plan must-follow row r-2b → r-2; plan ADR ownership rows 0024/0025 |
| r-2: `equal_input` / Attestation / `leak` in pinned samples are governed only by ADR-SAG-0025 | Applied. New ADR-SAG-0024 r-2 bullet, wording given verbatim: shape pin over the Active r-1 `EqualInputRecord`; dry-run gives every backend `NotSent` and `equal = false`; `leak`, the Attestation derivation and session-save runtime behaviour are ADR-SAG-0025 (r-2b); `jev_failed_haiku_ok.json` and `session_save_failed.json` are built with `test_builder` | r-2 D2.6 "shape pin" paragraph; r-2 "ADR text" (bullet); r-2 acceptance `adr:ADR-SAG-0024` |
| r-2: `session_save_failed.json` acceptable as a shape pin only | Applied. It is documented as a shape pin, with no r-2 runtime claim | r-2 D2.6; ADR-SAG-0024 r-2 bullet |
| r-2: `BackendStatus::Failed` / `BackendFailure` covered by ADR-SAG-0024 bullet 3 | No dependency, so no change | — |
| r-2 adrs missing ADR-SAG-0002, 0020, 0017, 0023, 0007 (verify) | Applied. All five added. ADR-SAG-0007 was verified: its Active body covers the API-direct backends and timeouts. r-2 owns no backend file, so 0007 is a regression check (`--test llm` green, no backend file changed). ADR-SAG-0023 is annotated as binding after r-4. A new acceptance row gives a check for each | r-2 `adrs`; r-2 acceptance `adr:ADR-SAG-0002, 0007, 0017, 0020, 0023` |
| r-2 adrs: add the r-1 Active amendment from ARCH-R1-002 | Applied, as a quoted list entry | r-2 `adrs` |
| r-2 flip scope (a): the acceptance row omits the Rust API patterns amendment and the 0029 marker | Applied | r-2 acceptance `adr:ADR-SAG-0005, 0009, 0014, 0019, 0022, 0003, 0029` |
| r-2 flip scope (b): the inline "Amended by ADR-SAG-0024 (Proposed, r-2)" line under ADR-SAG-0014 is not in owned_paths | Applied | r-2 `owned_paths`; r-2 "ADR text" markers paragraph; acceptance row |
| r-2 flip scope (c): rows 0019 and 0022 carry r-2b markers that r-2 must not touch | Applied. The r-2b markers in rows 0004, 0008, 0019 and 0022 are listed as untouched, and the acceptance row checks they still read Proposed | r-2 "ADR text" markers paragraph; acceptance row |
| r-3: the Active ADR-SAG-0020 r-3 bullet names r-3b's `source_scans` | Applied. Replacement wording for bullet 1 states only what r-3 enforces: clippy `disallowed-methods`, the one `#[expect]` site, the workspace lints, `[lints] workspace = true` checked locally, and the `rg` check. r-3 moves the cross-crate clause into a new ADR-SAG-0020 "Amendment (Phase A.1, r-3b)" subsection, inserted as Proposed, and adds its index marker. The bullet text is in r-3b | r-3 deliverable 4 and 7 (wording); r-3 `owned_paths`; r-3 acceptance `adr:ADR-SAG-0020`; r-3b "ADR text" (ADR-SAG-0020 r-3b bullet); plan ADR ownership row and exception note |
| r-3 adrs missing ADR-SAG-0009, 0019, 0016 | Applied, each with a regression acceptance check | r-3 `adrs`; r-3 acceptance `adr:ADR-SAG-0009, 0016, 0019` |
| r-3: mixed ADR-SAG-0026 status | Applied. The acceptance row requires "Active (HTTP half, r-3); compose half Proposed (r-3b)" verbatim and states that r-3 asserts nothing about the compose half | r-3 `adrs` comment; r-3 acceptance `adr:ADR-SAG-0026` |
| r-3: r-3b/r-4 markers r-3 must not flip (rows 0017, 0013/0016; 0026 compose half) | Applied. These are listed: the r-3b markers in rows 0010, 0011, 0013, 0016 and 0017, the compose half of row 0026, the new 0020 r-3b marker, the r-4 markers in rows 0016 and 0017, row 0023 and ADR-SAG-0028 | r-3 deliverable 7 markers paragraph; acceptance `adr:ADR-SAG-0006, 0017` |
| r-3: ADR-SAG-0023 and 0027 Proposed at `34dfed0` | Covered by the global precondition | r-3 `adrs` comment |
| r-3c | No fix required, so no change. It dispatches after the wave-1 flips are confirmed | — |
| r-2b: the "consumer output in r-3b" clause in ADR-SAG-0025 bullet 2 goes Active at r-2b | Applied. r-2b ends bullet 2 at "REQ-SAG-0604 (engine part)" and inserts a "(Proposed, r-3b)" consumer-output bullet, wording given verbatim. ADR-SAG-0025 Status and index cell read "Active (r-2b); consumer-output bullet Proposed (r-3b)", and r-3b flips them | r-2b deliverable 7; r-2b acceptance `adr:ADR-SAG-0025`; r-3b `owned_paths`, "ADR text", acceptance `adr:ADR-SAG-0025`; plan ADR ownership row and Status paragraph |
| r-2b adrs missing ADR-SAG-0014, 0009, 0006, 0026 (HTTP half), 0027 | Applied, with a regression acceptance row. ADR-SAG-0026 is annotated "HTTP half only" | r-2b `adrs`; r-2b acceptance `adr:ADR-SAG-0006, 0009, 0014, 0026, 0027` |
| r-3b adrs missing ADR-SAG-0003, 0027 | Applied, with a regression acceptance row | r-3b `adrs`; acceptance `adr:ADR-SAG-0003, 0009, 0027` |
| r-3b adrs: ADR-SAG-0019 and 0009 (verify against the body) | ADR-SAG-0009 applied: compose builds `BackendRegistry::from_env` unless `ComposeInputs::registry` is set. ADR-SAG-0019 declined: r-3b owns no wire builder, parser or prompt-resolution code, and `system_prompt_override` is passed through unchanged to `RunConfig` (ADR-SAG-0024) | r-3b `adrs` |
| r-3b: the ADR-SAG-0017 cell carries r-4 and r-3 markers that r-3b must not flip | Applied. r-3b flips only the last "(Proposed, r-3b)" in row 0017, and the same rule covers the r-4/r-1 markers in row 0016 | r-3b "ADR text" markers paragraph; acceptance `adr:ADR-SAG-0016, 0017, …` |
| r-3b: ADR-SAG-0020 `source_scans` bullet | Applied as an r-3b deliverable: r-3 inserts it as Proposed, and r-3b flips it and its marker | r-3b `owned_paths`, "ADR text", acceptance `adr:ADR-SAG-0020` |
| Convention: no shape on another sprint's Proposed text | Applied. The new paragraph sits in the ADR ownership section, because plan.md has no heading named "ADR index ownership" | plan "ADR ownership" |
| REQ-QA-R1-005: plan "Test seams" still says self dev-dependency after D0 outcome 4 | Applied. "Test seams" now states outcome 4, points at `boundaries/README.md`, and says `[[test]] required-features` plus `--features test-util`. It names the Proposed ADR-SAG-0022 r-2 "Correction" sentence (r-2 rewords it) and the ADR-SAG-0028 "Built-binary contract" sentence (r-3b rewords it). Both sprint docs carry the replacement wording. The self-dev-dependency lines in r-2, r-3, r-2b and r-3b and the plan boundary-map cell for r-2 now use `required-features`. Their validation lines gain `--features test-util` | plan "Test seams", boundary map `sc-jev-check` row; r-2 D2.6 and "ADR text"; r-3 deliverable 1; r-2b deliverables 5–6; r-3b D3.3, D7.2 and "ADR text"; validation blocks |

## Verification follow-ups (arch-qa, PR 23)

| id | disposition | where |
| --- | --- | --- |
| V-001 | applied — `--features test-util` on the three sc-jev-check plan validation lines | plan.md validation |
| V-002 | applied — cross-reference now "ADR ownership" | r-2 D2.1 |
| V-003 | applied — boundary-map sentence marked not taken (outcome 4) | plan.md boundary map |
| V-004 | applied — interim-amendment note is a YAML comment; post-flip label note added | r-2 frontmatter, Deliverables |
| V-005 | applied — note on feature-independence + a `--lib` parse-refusal test line | r-3 validation |

