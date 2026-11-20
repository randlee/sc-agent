# sc-agent, pass 3: design for review

Status: draft for Rand's one gap pass. Replaces `sc-agent-rewrite-design.md`. Nothing is planned or coded from this until he has marked it.

## 1. Goal (Rand's words, 2026-11-19, unchanged)

You want sc-agent to do one small job well. That job is to replace the retry and logging code each of your Python tools currently carries, about 100 lines per tool, with one shared Rust core, and to add sitr.

What it does:
- Sends a prompt to a named model (Jev, decider, Haiku, Luna, Laya and the rest) through a fixed table per provider with model (string) and date (so most recent is used by default.
- Retries when the provider is busy, waiting for Retry-After.
- Gets the API key from the .env or environment or the OS keyring, and never shows it.
- Batches many requests in one run.
- Logs consistently through sc-observability: one Logger per process, files at fixed paths, and session rows filed by repo, agent and project.
- Extracts code with tree-sitter (sitr), usable from the CLI and from Python.
- Ships as two CLIs, decidr and sitr, plus Python wheel, so plan-anchor, judge-jev and future tools write almost no Python of their own.

How it should be built:
- About the size of the Python it replaces. ~ 2,000 lines of Rust in total, not 50,000.
- Libraries are called directly: no wrappers, no re-implemented parsers, and no re-testing what the libraries already test.
- Errors come back as values, not exceptions.
- The working Python is the spec, and nothing gets built that isn't in it or that you didn't ask for.

Added on Rand's yes (rulings from this week that the working Python does not contain):
1. Per-question sha256 as the cache key, with a public method to compute it without sending.
2. The request text in every session row.
3. A different typed answer per question type.
4. Layered config through sc-config.
5. Cancel.
6. `doctor`.

## 2. Sources

| Source | Where | Used for |
|---|---|---|
| The spec | `jev_judge.py.j2` on `main` of plan-anchor (`cfa67c7`) and judge-jev (`9065a5a`): `call_jev`, `http_error`, `parse_retry_after`, `judge_with_policy`, `validate_response`, `answer_problem`, `resolved_api_key`, `check_api_key`, `run_batch_job`, `stream_batch` | Every behavior in section 4 |
| Reference Rust | current sc-agent (pass 2) at `e77f131` | Only the two things the Python does not have: the Haiku/Luna path and sitr. Section 7 names the items |
| Client names | `decidr_client.py.j2` on `develop` of both consumers | The names the consumers' own code calls (section 8) |

Nothing else in pass 2 is a source.

## 3. What ships

| Product | Contents |
|---|---|
| `decidr` CLI | `send`, `batch`, `digest`, `models`, `doctor` |
| `sitr` CLI | `query`, `languages`, `queries` |
| One wheel, `sc-decidr` | `sc_agent` (decisions) and `sc_agent.sitr` (tree-sitter), plus the client template |
| Client template | Rendered into plan-anchor and judge-jev by sc-compose; holds the shared batch pipeline the two tools duplicate today |

One wheel means one copy of sc-observability and one Logger in a Python process.

## 4. Behavior: the working Python and the Rust that replaces it

| Behavior | Working Python | Rust | Library |
|---|---|---|---|
| Send | POST `https://api.typesafe.ai/v1/systemone`, bearer key, JSON body, 30 s default timeout | Same | ureq |
| Reply limits | Over 1 MiB, not JSON, or not an object is an invalid response | Same | serde_json |
| Request limit | 24,000 bytes default, 64,000 ceiling, because the Python logger refused events over 64 KiB | The Rust sc-observability logger accepts events up to 1 MiB, so the request limit is 1 MiB; a larger request returns `SizeExceeded` without sending | none |
| Status to error | 401 auth; 422 validation with the first 300 characters; 429 and 529 busy; 500 and up transient; a 4xx mentioning `max_tokens` or token-exceed is context-exceeded; anything else is a plain HTTP error. Bodies are withheld except the 422 text | Same, as `Category` plus `status` | none |
| Retry | Up to 3 sends. Retried: busy, 5xx, network, invalid response. Wait Retry-After (seconds, capped at 30) else 1 s, then 2 s | Same. `max_sends` default 3; the batch path passes 2, as the Python does | std |
| Not retried | 401, 422, context-exceeded | Same. Context-exceeded and request-size-exceeded are returned at once as errors; nothing halves or shortens a request, in Rust or in the client | none |
| Answer check | Every question answered, no unknown ids; choice is one of the options, confidence and probabilities in range and summing to 1 within rounding; noul in 0..1 | Same, producing typed answers (item 3) | serde |
| Key | Environment, then OS credential store; must be printable ASCII with no whitespace | Environment, then `.env`, then OS keyring (service = variable name, account = OS user) | dotenvy, keyring |
| Never show the key | Redaction pass over output | Key type whose debug and display print nothing; no reply body or header reaches a message | none |
| Batch | Thread pool of `max_workers` (default and cap 4), results yielded in request order, one retry per job, whole batch stops sending on an auth error | `send_batch` with scoped threads, same order, same stop rule | std |
| Result record | Per job: every attempt with its error, retry count, raw response, model that answered | `Outcome` carries the same (section 6) | none |
| Dry run and answer fixtures | In the script | Stay in the client template; Rust knows nothing about them | none |
| Item shortening on context-exceeded | In the script | Not built. Rust returns `ContextExceeded`; a request over the size limit returns `SizeExceeded` | none |

One deliberate difference: the Python honours Retry-After on the first retry only. The Rust honours it on every retry. Say so if you want the Python's rule exactly.

What the Python does not have and you asked for:

| Item | Design |
|---|---|
| Other models | A provider table (base URL, key variable, wire format) and a model table (name, provider, model id, release date). `--model haiku` picks the newest row named haiku; an exact id picks that row. Three wire formats: Jev, Anthropic Messages, Chat Completions. Rows: the ten models and six providers pass 2 has today |
| decider and Laya | Local runners speaking the Jev wire at `http://host:port`. Each is a config entry: name, host, port, model |
| Haiku/Luna answers | The question is turned into a prompt and an answer schema; the reply is parsed into the same typed answers. A reply that does not match the schema is resent once |
| sha256 (item 1) | `digests(request)` returns one sha256 per question id, through sc-sha, over the compact JSON of `{model, state, question}`. It never sends. `send` returns the same values |
| Session row (item 2) | One row per request: `session_id` (ULID), `repo`, `project`, `agent_role`, `model`, `position`, `sends`, `digests`, `message` (the request as sent), and `response` or `error`. File: `~/.sc-agent/sessions/<repo>/<agent_role>/<project>/sessions.jsonl` |
| Typed answers (item 3) | `Answer::Noul { noul }`, `Answer::Choice { choice, confidence, probabilities }` |
| Config (item 4) | sc-config's layered loader. Settings: `project` (required), `workers`, `max_sends`, `timeout_secs`, local runners, extra model rows |
| Cancel (item 5) | A token checked before each send and each wait; a cancelled request returns `Category::Canceled` |
| doctor (item 6) | For each provider: is a key found, and where. For each local runner: one reachability check. Config path in use. Log paths |
| General log | One sc-observability Logger per process, opened by the entry point (a CLI's `main`, or the client template) and passed to every call. File: `~/.sc-agent/logs/<name>.log.jsonl`. Root override for tests only: `DECIDR_LOG_ROOT`, `SITR_LOG_ROOT` |
| sitr | `run_query(language, query, source, captures)` over tree-sitter with the Python and Rust grammars and the predefined queries pass 2 ships |

## 5. Not built

Each of these exists in pass 2. None is in the working Python or in section 1.

- Images, assets and multipart requests.
- Score and Box questions. Neither consumer's own code uses them.
- Quota detection and the provider-wide pause.
- The 90-second send budget (the Python's three sends replace it).
- Local-runner discovery and daemon kinds. A runner is a host, a port and a model.
- LLM requests that are not decisions.
- C# and Go grammars.
- A log-root setting or flag.
- Any Python logging path, and the `sc_observability` Python package.
- A separate sitr wheel.
- Traits, transports and clocks kept for test doubles; the test-kit crate.
- An error type per module. There is one.

## 6. Public surface

This is the whole of it. A sprint that needs another public item stops.

```rust
// decidr (library)
pub fn send(request: &Request, options: &Options) -> Outcome;
pub fn send_batch(requests: &[Request], options: &Options, on_result: &mut dyn FnMut(Outcome));
pub fn digests(request: &Request) -> Result<BTreeMap<String, String>, Error>;
pub fn models(options: &Options) -> Result<Vec<Model>, Error>;
pub fn doctor(options: &Options) -> Doctor;
pub fn open_log(name: &str) -> Result<Logger, Error>;        // sc-observability's own Logger

pub struct Request  { pub model: String, pub state: serde_json::Value, pub questions: BTreeMap<String, Question> }
pub enum   Question { Noul { instructions: String, criteria: NoulCriteria }, Choice { instructions: String, criteria: BTreeMap<String, String> } }
pub struct Response { pub model: String, pub answers: BTreeMap<String, Answer>, pub usage: Option<Usage> }
pub enum   Answer   { Noul { noul: f64 }, Choice { choice: String, confidence: f64, probabilities: BTreeMap<String, f64> } }

pub struct Options  { pub project: String, pub client: String, pub log: Logger, pub workers: Option<usize>,
                      pub timeout_secs: Option<u32>, pub max_sends: Option<u32>, pub cancel: CancelToken }
pub struct Outcome  { pub session_id: Ulid, pub position: usize, pub model: String, pub digests: BTreeMap<String, String>,
                      pub attempts: Vec<Attempt>, pub result: Result<Response, Error> }
pub struct Attempt  { pub ok: bool, pub error: Option<Error> }
pub struct Error    { pub category: Category, pub message: String, pub status: Option<u16>, pub retry_after_secs: Option<f64> }
pub enum   Category { Busy, Auth, Network, ResponseInvalid, Validation, ContextExceeded, SizeExceeded, Http, Canceled, Panic }

// sitr (library)
pub fn run_query(language: &str, query: &str, source: &[u8], captures: Option<&[String]>) -> Result<Vec<Capture>, Error>;
pub fn languages() -> &'static [&'static str];
pub fn queries(language: Option<&str>) -> Vec<&'static QueryInfo>;
pub fn query_source(language: &str, name: &str) -> Result<&'static str, Error>;
```

Python, in the one wheel. Every function returns a value; none raises for a failed request.

```
sc_agent.open_log(name) -> Logger
sc_agent.send(request, *, project, client, log, timeout_secs=None, max_sends=None, cancel=None) -> dict
sc_agent.send_batch(requests, on_result, *, project, client, log, workers=None, timeout_secs=None, max_sends=None, cancel=None) -> None
sc_agent.digests(request) -> dict
sc_agent.models() -> list
sc_agent.doctor() -> dict
sc_agent.CancelToken
sc_agent.sitr.run_query(language, query, source, captures=None, *, log) -> list
sc_agent.sitr.languages() / queries(language=None) / query_source(language, name)
```

## 7. Files and sizes

Non-blank, non-comment lines. "Copied" is counted from the named pass-2 items; "new" is my estimate.

| Crate | File | Lines | Basis |
|---|---|---|---|
| `decidr` | `schema.rs` (request, question, response, answer) | 90 | new, from the Python's shapes |
| | `digest.rs` | 30 | new, sc-sha call |
| | `error.rs` | 60 | new, from `http_error` |
| | `keys.rs` | 45 | new, dotenvy and keyring calls |
| | `catalog.rs` (provider and model tables, newest-by-date) | 60 | new; rows copied from `sc-agent-types/src/model.rs` 123-189 |
| | `http.rs` (post, status mapping, retry) | 70 | new, from `call_jev` and `judge_with_policy` |
| | `jev.rs` (encode, answer check) | 90 | new, from `validate_response` and `answer_problem` |
| | `llm.rs` (prompt, answer schema, two wires, parse) | 440 | copied: `sc-decision-llm` `prompt.rs` 18-23, 75-114; `schema.rs` 11-100; `answers.rs` 77-136, 167-315. `sc-llm-provider` `encode.rs` 16-111; `decode.rs` 54-146; `endpoints.rs` 8-31. That is 516 lines; Score, Box and image arms come out |
| | `run.rs` (send, batch, cancel) | 90 | new, from `run_batch_job` and `stream_batch` |
| | `log.rs` (open log, call events, session row, repo name) | 120 | new, sc-observability calls |
| | `config.rs` | 50 | new, sc-config call |
| | `lib.rs` | 20 | exports |
| | `main.rs` | 110 | new, clap |
| `sitr` | `lib.rs` | 360 | copied: `sc-treesitter` `run.rs`, `grammar.rs`, `query.rs`, `source.rs` are 459 lines; the 267-line `error.rs` becomes one enum |
| | `main.rs` | 100 | new, clap |
| `sc-decidr` | `lib.rs` (PyO3 for both APIs) | 190 | new |
| **Rust total** | | **1,925** | |
| `sc-decidr` | `consumer.py.j2` | at most 480 | the client names in section 8, copied from pass 2's template |
| Tests | Rust, against a local socket server | at most 1,200 | one test per row of section 4 |

Three crates. Dependencies, and no others: sc-observability, sc-config, sc-sha, ureq, dotenvy, keyring, clap, serde, serde_json, ulid, tree-sitter, tree-sitter-python, tree-sitter-rust, pyo3.

The Haiku/Luna path is 23% of the Rust. It is the one large piece and the one piece the Python does not specify.

## 8. Drop-in for plan-anchor and judge-jev

The consumers' own code keeps calling the client by the names it uses on `develop` today:

- Both: `ClientError`, `ChoiceAnswer`, `call_batch_results`, `remote_preflight`, `BatchSettings`, `BatchJob`, `BatchInputError`, `StreamedResult`, `batch_settings`, `validate_request_lines`, `check_fixture_coverage`, `load_answers_fixture`, `stream_batch`, `pump_batch`, `streamed_result`, `_line`, `_fixture_attempt`, `_json_answers`.
- plan-anchor only: `call_result`, `from_tool_error`, `canceled_envelope`, `CANCELED_EXIT`, `REPO_ROOT_HELP`, `BATCH_ID_RE`, `BATCH_MAX_WORKERS`, `BATCH_RESULT_SCHEMA`, `batch_request_id`, `encode_result_line`, `write_stdout_line`, `questions_digest`, `_safe_id`, `_total_attempts`, `_typed_answer`.
- judge-jev only: `_aiter`, `_encode_request`.

Changes a consumer makes, and no others:
- The wheel pin, and judge-jev drops its `sitr` pin.
- Its sc-compose render variables.
- judge-jev's separate run log becomes the client log.
- Three test lines in judge-jev that name `sc_treesitter`.

Error codes the consumers print (`JEV.RATE_LIMITED`, `JEV.OVERLOADED`, `AUTH.CREDENTIALS` and the rest) are mapped in the client from `Category` and `status`.

Every client function returns a result value (`Result[T, E]`: ok with the value, or err with the typed error). The client raises nothing; `ClientError` and `BatchInputError` are error values, not exceptions.

One thing a user will notice: the working Python read the keychain entry `plan-anchor` / `typesafe-api-key`. The Rust reads `TYPESAFE_API_KEY` / the OS user, per your ruling. A key stored under the old name is not found.

## 9. Gates

Each is a script that passes or fails. No reviewer judgement is involved in any of them.

| Gate | Fails when |
|---|---|
| Files | A crate has a file not in section 7 |
| Size | A file is over its line count |
| Surface | The public items differ from section 6 |
| Dependencies | `Cargo.toml` names a crate not in section 7 |
| Constructs | Source defines a trait, or uses `unwrap`, `expect` or `panic!` outside tests |
| Panics | Every public Rust function and every wheel function catches a panic (`catch_unwind`) and returns it as an error with `Category::Panic`; a test forces a panic through each entry point and gets the error back |
| Behavior | A row of section 4 has no test named for it, or the test fails |
| Drop-in | Either consumer's own test suite fails against the wheel |

Review rule: a finding must cite a section of this design. A finding that asks for something not in the design is closed as invalid. QA runs once per sprint, not once per fix.

## 10. Sprints

| Sprint | Delivers | Lines |
|---|---|---|
| 0 | Records rewritten from this design; sc-config's layered loader released | 0 |
| 1 | One request to Jev from the CLI: schema, error, keys, http, jev, log, the `send` command | about 550 |
| 2 | Batch, cancel, digest, config, the provider and model tables, `models`, `doctor` | about 285 |
| 3 | Haiku/Luna path | about 440 |
| 4 | sitr library and CLI | about 460 |
| 5 | The wheel and the client template | 190 and up to 480 |
| 6 | plan-anchor on the wheel | consumer edits only |
| 7 | judge-jev on the wheel | consumer edits only |

Sprint 1 is the 100 lines of Python, in Rust, end to end. If it cannot be done in its size, the rest of the table is wrong and the work stops there.

Live calls to Jev need you present. Each sprint collects its live checks into one run.

## 11. Yours to rule

1. Where pass 3 is built. `sc-agent-ref` is already pass 1, so the current repo cannot take that name.
2. `sc_agent.sitr` as the import name.
3. What happens to the `sitr` package on PyPI.

## 12. Not verified

- The "new" line counts in section 7 are estimates. The two "copied" counts are measured, before the removals.
- sc-config's layered loader exists only as a draft pull request.
- I have not checked that dotenvy and keyring behave as section 4 says on Windows and Linux.
- The client's 480 lines is an upper bound from the names the consumers call; with batching in Rust it should be smaller.
