# decidr

The `decidr` command line and its library. This is the top layer for decision models. The library target is the composition root: the one place where a backend is chosen (a configured local runner, TypeSafe Jev, or an LLM provider ranked by the keys found) and wired into `sc-decision-runner` over one `HttpClient`. The binary is the `decidr` CLI. `sc-agent-py` is a thin layer over this library and adds no wiring of its own.

## Command line

```bash
decidr send --model jev --project my-project --agent-role judge request.json
```

```bash
decidr send --batch --model haiku --project my-project --agent-role judge requests.jsonl
```

```bash
decidr models
decidr doctor
```

- `send`: one JSON request, or a JSONL batch with `--batch`. The input is a file, or `-` for stdin (the default). One result line is printed per request.
  Each result object contains `session_id`, `position`, `sends` (the number of provider requests started, including retries), and `digests`, plus either `response` or `error`. `sends` is zero when no request was sent.
  - `--model` takes a [short name](../sc-agent-types/src/short_name.rs), a model id, or the name of a configured local runner.
  - `--project` is an optional global project/phase override; absent, `SC_PROJECT` or a configuration file supplies it. A send without a project is refused before any request or session row. `--agent-role` is required. Both use path-safe lowercase slugs.
  - `--provider` overrides the provider. Without it, the provider is chosen by ranking the keys that are found.
  - `--image NAME=FILE` (repeatable) and `--long-context` apply to a single request, not to `--batch`.
  - `--timeout-secs SECONDS` sets the per-request timeout (finite values from 10 through 3600 seconds; fractional seconds are accepted).
  - `--workers COUNT` sets batch concurrency from 1 through 64 and requires `--batch`.
- `models [--runner NAME]`: lists the models of TypeSafe, or of a configured local runner.
- `doctor`: lists the keys found, in use order, and warns about provider models missing from the catalog.
- Configuration resolves flags, CLI environment variables (`DECIDR_`, plus `SC_PROJECT`), local `.sc/sc-agent/decidr.json` and `sc-agent.json`, then global `~/.sc-agent/decidr.json` and `sc-agent.json`. Outside a git worktree, local files are skipped. `doctor` reports settings, sources and file existence; unreadable keyrings appear as typed rows. It opens no file logger.

Exit codes: 0 when the command ran and every result succeeded, 1 otherwise, 130 after an interrupt. Ctrl-C or SIGTERM cancels the run: every unfinished request ends as `decision.cancelled` and the logs are flushed. A third interrupt exits at once.

## What the library provides

- `send`, `send_batch`, `models` and `doctor`, with `send_on` and `send_batch_on` over any `Decision`.
- `SendOptions`, `ModelsOptions`, `DoctorOptions`, `ModelSelector` and `Context`. The context holds the seams (loaded layered config, CLI name, explicit home, environment, keyring, transport, injected destinations and clock); every field `None` is the real default. `Context.keyring_override` optionally supplies a checked `KeyringLocation`; its service and account override the defaults only for the selected provider.
- `CallLimits`, `RequestTimeout` and `Workers` for checked per-call limits. `send_with` and `send_batch_with` accept `&CallLimits`; the existing `send` and `send_batch` use default limits. A timeout applies to the default decision transport and request deadline; worker count controls batch concurrency.
- `load_context`, `load_config`, `general_logger`, `session_logger` and `session_identity`.
- `DoctorReport`, `DetectedKey`, `MissingModel` and `Tie`.
- Modules `input` (`build_request`, `build_batch`, `read_batch`, limits), `output` (`result_line`, `result_text`) and `signal`.
- `DecidrError`. A failed decision is data in its `DecisionResult`, not an error.

## Library usage

```rust,ignore
use decidr::input::build_request;
use decidr::output::result_text;
use decidr::{Context, ModelSelector, SendOptions, load_config, send};
use sc_agent_log::Slug;
use sc_agent_types::CancelToken;

let json = br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;
let request = build_request(json, &[], false, None)?;

let context = Context::new(sc_agent_config::CliName::Decidr);
let config = load_config(&context)?;
let options = SendOptions {
    model: ModelSelector::parse("jev", &config)?,
    provider: None,
    project: Slug::new("project", "my-project")?,
    agent_role: Slug::new("agent_role", "judge")?,
    context,
    session: None,
};
send(&options, request, &CancelToken::new(), &mut |result| {
    println!("{}", result_text(&result)?);
    Ok(())
})?;
```

## Depends on

- `sc-agent-types`, `sc-agent-config`, `sc-agent-secrets`, `sc-agent-log`, `sc-agent-http`, `sc-decision-schema`, `sc-decision-systemone`, `sc-decision-local`, `sc-llm-provider`, `sc-decision-llm` and `sc-decision-runner`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004), and `decidr` is the top. Nothing above it except `sc-agent-py`. Dev only: `sc-agent-testkit`, and the `testing` features of `sc-agent-http` and `sc-agent-log`.

## Requirements and decisions

- REQ-SAG-0501: TypeSafe API as decidr subcommands.
- REQ-SAG-0503: doctor warns about models missing from the catalog.
- REQ-SAG-0504: invalid batch input fails fast.
- REQ-SAG-0014: input size limits.
- REQ-SAG-0011: interrupt cancels cleanly.
- REQ-SAG-0103: provider and model are separate.
- REQ-SAG-0004: session log identity.
- REQ-SAG-0008: optional logger injection with default loggers.
- ADR-SAG-0001 and ADR-SAG-0004: one backend trait; crate layering, with `decidr` as the composition root.

General logs use `<log_root>/logs/decidr.log.jsonl`; sessions use
`<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`, kept indefinitely.
The default log root is `<home>/.sc-agent`. `DECIDR_LOG_ROOT` overrides it;
`DECIDR_WORKERS` and nested variables such as `DECIDR_LOCAL_RUNNERS__LAYA__PORT`
override decidr settings. See [sc-agent-config](../sc-agent-config/README.md) for scope and precedence.
