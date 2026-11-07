# Phase A.1 plan review round 5 — dispositions (waves 3–5 ADR pre-screen)

Input: arch-qa pre-screen of `integrate/phase-a` @ `97db748`,
`.cursor/phase-a-orchestration/wave9-dev/wave34-adr-prescreen.json`. It
screens r-2b (wave 3), r-3b (wave 4) and r-5 (wave 5, blocked) before
dispatch, for the pattern in plan "ADR ownership" ("No shape on another
sprint's Proposed text") and for `adrs` lists that leave out ADRs governing
owned paths.

As in round 4, `docs/architecture.md` is not edited here (wave-2 PRs touch
it). Each ADR text change is a deliverable, with its wording, in the sprint
doc that owns the subsection; that sprint applies it at flip time.
Abbreviations: `plan` = `plan.md`, `r-N` = that sprint doc.

## Dispositions

| id | Disposition | Where |
| --- | --- | --- |
| ARCH-R2B-001 (BLOCKING, ADR-SAG-0020 missing) | Applied. ADR-SAG-0020 added with a comment. New acceptance row: digest recovery adds no `ResolvedApiKey::as_str` call and no `disallowed_methods` expect under r-2b's owned paths, and a recovery error carries no body or key bytes (`test_digest_recovery_error_carries_no_body_bytes` in both `wire` targets) | r-2b `adrs`; acceptance `adr:ADR-SAG-0020`; validation |
| ARCH-R2B-002 (BLOCKING, ADR-SAG-0013 missing) | Applied. ADR-SAG-0013 added. Acceptance row names the log-event expectation: every `judge` call emits one `LogEvent`, a `Failed` backend included, `NotSent` emits none (`test_failed_backend_emits_one_log_event`); a session-save failure keeps emitted events and adds none (`test_session_save_failure_log_events_unchanged`) | r-2b `adrs`; acceptance `adr:ADR-SAG-0013`; validation |
| ARCH-R2B advisory (MINOR, ADR-SAG-0003 / 0023) | Applied. Both added, each annotated "untouched". An acceptance row checks it: no `.env` or repo-root code outside r-2's `discover_repo_root`, and `check-exact-pins.sh` green | r-2b `adrs`; acceptance `adr:ADR-SAG-0003, 0023`; validation |
| ARCH-R2B-003 (IMPORTANT, r-1 interim supersession scope) | Applied. r-2b now owns the ADR-SAG-0008 and ADR-SAG-0019 r-1 interim subsections (their Status lines and superseded sentences) and the ADR-SAG-0019 body's italic pointer line. D7 lists, sentence by sentence, what each one supersedes: 0008 bullet 1 except its last sentence, and the 0019 bullet-1 "Until r-2b … `equal = false`" sentences plus the bullet-2 "r-1 records `ChainContext`" sentence. It also lists what stays in force: 0008 bullet 2 (session v2) and 0019 bullet 2 (`Arc<PayloadBundle>` + `CancelToken`). D7 gives the new Status lines verbatim. ADR-SAG-0014: r-2's ADR text list (`sprint-r-2` "ADR text") supersedes the interim as a whole and does not drop "null until r-2b". r-2 is in flight, so r-2b adds an r-2b-owned pointer line to ADR-SAG-0025 bullet 3 and does not edit 0014 | r-2b `owned_paths`; D7 "What the r-2b amendments supersede"; acceptance `adr:ADR-SAG-0008`, `adr:ADR-SAG-0019` |
| ARCH-R2B-004 (IMPORTANT, markers r-2b must not touch) | Applied. D7 lists what r-2b must leave alone: the ADR-SAG-0026 row and text, the r-3b markers in rows 0010, 0011, 0013, 0016, 0017, 0020, 0023 and 0028, the r-1 "A.1 subsection" markers, and the r-2 markers | r-2b D7 "Markers r-2b must not touch" |
| ARCH-R3B-001 (BLOCKING, ADR-SAG-0008 missing) | Applied. ADR-SAG-0008 added, with an acceptance row: Independent is the default in compose and both consumers, and `ChainContext` runs only on request | r-3b `adrs`; acceptance `adr:ADR-SAG-0008` |
| ARCH-R3B-002 (BLOCKING, ADR-SAG-0019 missing; 0005 optional) | Applied. This reverses round 4's decline: the clap port of `--llm-system-prompt-file` is a consumer surface that the ADR-SAG-0019 precedence names. ADR-SAG-0019 added, with a new `test_cli_system_prompt_file_overrides_check_toml` (D12.1) and the `run::prompt::` regression. ADR-SAG-0005 added, with the PayloadBuilder prohibition row (`! rg -n 'PayloadBuilder'` over compose, `sc-ai` and `sc-jev-py` src) | r-3b `adrs`; D12.1; acceptance `adr:ADR-SAG-0019`, `adr:ADR-SAG-0005`; validation |
| ARCH-R3B-003 (IMPORTANT, ADR-SAG-0028 conditional and "ignored" branch) | Applied. D0 is outcome 4. The "ignored" alternatives are deleted from D7.0, D12.2 and the Manifests paragraph. D7.0 also states `facade = "workspace_tests"`, per the recorded spike. A new ADR text deliverable replaces the conditional in the ADR-SAG-0028 "Dev-only edges" bullet with the recorded outcome: dev edges counted, `+workspace-tests` in `allowed_dependents` of `sc-agent-types`, `sc-jev-check` and `sc-agent-compose`, and no gate-script dev-set row. The REQ-SAG-0109 and ADR-SAG-0028 acceptance rows check it | r-3b D7.0, D12.2, Manifests, "ADR text", acceptance |
| ARCH-R3B-004 (MINOR, ADR-SAG-0023 / 0028 Status lines) | Applied. Both Status lines added to `owned_paths` and to the flips list, wording given, with an ADR-SAG-0023 acceptance row | r-3b `owned_paths`; "ADR text"; markers paragraph; acceptance `adr:ADR-SAG-0023`, `adr:ADR-SAG-0028` |
| r-3b check 3 (ADR-SAG-0020 r-3b subsection from r-3) | Applied as a wave-4 pre-flight line: confirm r-3 landed the ADR-SAG-0020 "Amendment (Phase A.1, r-3b)" Proposed subsection and its index marker; if not, r-3b inserts them verbatim before flipping | r-3b "ADR text" (Wave-4 pre-flight) |
| ARCH-R5-001 (BLOCKING, ADR-SAG-0001/0020/0026/0027 missing) | Applied. All four added with comments. Deliverable 5 states the error and transport rules, and an acceptance row checks them (`test_error_response_stderr_golden`, `source_scans`, `jev_api`). ADR-SAG-0008/0014 not added: the envelope changes neither text | r-5 `adrs`; deliverable 5; acceptance `adr:ADR-SAG-0001, 0020, 0026, 0027` |
| ARCH-R5-002 (BLOCKING, Branch B has no ADR-SAG-0002 text) | Applied. Branch B and "both" insert and flip an ADR-SAG-0002 r-5 amendment, wording given (optional `criteria`/`CriteriaSpec`, exactly-one validation, byte-identical array payloads, one type), and add the ADR-SAG-0002 index marker. Both are in `owned_paths`. The marker is recorded as a third exception in plan "ADR ownership". Branch A's ADR-SAG-0005 text states that the object-shape path is deleted and that ADR-SAG-0002 is unchanged | r-5 `owned_paths`; Branch A, Branch B; acceptance `adr:ADR-SAG-0002`, `adr:ADR-SAG-0005`; plan ADR ownership row and exception note |
| ARCH-R5-003 (IMPORTANT, ADR-SAG-0019 wording) | Applied, option 1. The ADR-SAG-0005 r-5 bullet gains a sentence saying that ADR-SAG-0019 is unchanged and recovery is unchanged in method. No ADR-SAG-0019 owned path is added | r-5 deliverable 4; acceptance `adr:ADR-SAG-0005` |
| ARCH-R5-004 (IMPORTANT, `--features test-util` on `wire`) | Applied. `--features test-util` added to the three `sc-jev --test wire` lines, to a new `sc-jev-backends --features test-util --test wire` line and to the REQ-SAG-0500/0501 regression cell | r-5 validation; acceptance `boundary:jev` |
| Earlier QA: ADR ownership table lacks the r-1 interim subsections | Applied. Rows added for the ADR-SAG-0008, 0014 and 0019 r-1 interim subsections (landing sprint r-1), with what supersedes each: ADR-SAG-0025 (r-2b, sentence-level) for 0008/0019 and ADR-SAG-0024 (r-2) for 0014. An ADR-SAG-0002 r-5 row was added too | plan "ADR ownership" table and exceptions paragraph |

## Not changed

- plan "New boundary manifests" and the `cargo tree` gate table still
  describe the "dev edges ignored" alternative as a conditional, along with
  the `workspace-tests` `facade = ""` text that the spike's `workspace_tests`
  fallback replaces. This revision is limited to cross-references and
  ownership rows in plan. r-3b's text is now unconditional and takes
  precedence for r-3b. A later plan edit can remove the conditional.

## Verification follow-ups (arch-qa, PR 24)

| id | disposition | where |
| --- | --- | --- |
| ARCH-V-001 | applied — plan.md manifest TOML `facade = "workspace_tests"`; D0 conditional collapsed to outcome 4 in the manifests section, gate table and boundary map | plan.md |
| ARCH-V-002 | applied — D5 defines the recovery path: before send; parse failure = INPUT_INVALID, backend Failed, nothing sent, no body/key bytes | r-2b D5 |
| ARCH-V-003 | applied — sentence-precise supersession wording for the ADR-0019 interim | r-2b D7 |
| ARCH-V-004 | applied — pointer line names Unattested | r-2b D7 |
| ARCH-V-005 | applied — test named and validation line added (Branch B / both) | r-5 acceptance, validation |
| ARCH-V-006 | applied — row limited to the CLI path (Python side has no override kwarg today) | r-3b acceptance |

