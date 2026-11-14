# sc-agent v0.1.0 — One Launch Interface for Decision Models, LLMs and (Later) Agents

**Released:** November 13, 2026 · **Install:** `brew install randlee/tap/decidr` (Homebrew — installs `decidr` and `sitr`), `pip install sc-decidr` (the `sc_agent` Python module), `pip install sitr` (the `sc_treesitter` module), `cargo install decidr` (crates.io), or the platform archives on the [release page](https://github.com/randlee/sc-agent/releases/tag/v0.1.0)

[README](https://github.com/randlee/sc-agent/blob/main/README.md) · [Requirements](https://github.com/randlee/sc-agent/tree/main/docs/requirements) · [ADRs](https://github.com/randlee/sc-agent/tree/main/docs/adr) · [Release notes](https://github.com/randlee/sc-agent/releases/tag/v0.1.0)

This is sc-agent's first stable release. It ships phase A: every AI backend an agent
workflow calls — a TypeSafe Jev decision model, a local Jev-compatible runner, or a
hosted LLM such as Anthropic, OpenAI, OpenRouter, Nous or Together — behind one request
form, one key and configuration story, one log layout and one error shape. Seventeen
crates, two command-line tools (`decidr`, `sitr`) and two Python modules
(`sc_agent`, `sc_treesitter`); fourteen crates are published to crates.io and both
Python projects to PyPI. The workspace is synchronous throughout — there is no async
runtime and no tokio.

---

## Agent-Run Author — one call shape, whichever backend answers

**As an agent-run author, I want to launch every AI backend through one interface, so that swapping a decision model for an LLM is a flag change and not a rewrite.**

`decidr send` takes one JSON request, or a JSONL batch with `--batch`, and prints one
result line per request. `--model` accepts a short name (`jev`, `haiku`, `luna`, ...), a
model id, or the name of a configured local runner; `--provider` overrides the provider,
and without it the provider is chosen by ranking the keys that are actually present. So
the same command works on a machine that has only a TypeSafe key and on one that has only
an Anthropic key, and moving a gating step from Jev to Haiku in the place where a cheap
answer is good enough is one argument.

The point of the release is that the answer is shaped the same way regardless of who
produced it. Haiku and Luna can answer *in Jev's place*, and a local runner serving the
Jev API is interchangeable with the hosted one behind the same `Decision` trait and the
same `DecisionRequest`. A caller does not branch on which backend ran, and it does not
learn a second error vocabulary: request failures, backend failures and configuration
failures arrive as one error shape.

Everything `decidr` does is also one Python function away. `sc_agent.send`,
`send_batch`, `models` and `doctor` mirror the CLI and also accept `images=`,
`long_context=`, `cancel=`, `config_dir=` and injectable `general_log=` /
`session_log=` callables, so a Python orchestrator can route and capture logs without
reimplementing the client. `KeyboardInterrupt` cancels the run and is re-raised.

```bash
decidr send --model jev --project my-project --agent-role judge request.json
decidr send --batch --model haiku --project my-project --agent-role judge requests.jsonl
```

```python
import sc_agent

line = sc_agent.send(request_json, model="jev", project="my-project", agent_role="judge")
lines = sc_agent.send_batch(requests, model="haiku", project="my-project", agent_role="judge")
```

## Pipeline Orchestrator — cancellable runs and a per-request audit trail

**As a pipeline orchestrator, I want every request recorded in a session log and every run cancellable, so that a gating loop is reviewable after the fact and a stuck fleet can be stopped cleanly.**

Each run writes session rows to
`<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl`, one row per request, keyed
by the same project and agent role the caller passed. Those files are never rotated or
pruned, which is what makes them usable as evidence: an orchestrator can reconstruct what
a pipeline asked and what it got back for any past run. General logs are separate and are
rotated through sc-observability.

Cancellation is defined rather than incidental. `Ctrl-C` or `SIGTERM` cancels the run,
every unfinished request ends as `decision.cancelled`, the session log is flushed, and
`decidr` exits 130. A third interrupt exits at once, so an orchestrator that has already
been abandoned by its operator cannot wedge a slot. Multiple requests in flight are
served by worker threads in `sc-decision-runner`, which also owns holdoff after a
provider-wide failure — a backend that is failing does not turn into a burst of retries
from every worker.

## Ops Engineer — one resolution order for keys and config, plus `doctor`

**As an ops engineer, I want keys, configuration and logs resolved in one documented order, so that a broken credential or a catalog mismatch is diagnosable instead of guessed at.**

Configuration loads from `~/.sc-agent/default.json`, then `local.json`, then
`SC_AGENT__*` environment overrides, and every value is validated when the configuration
loads — not on the first request that happens to touch it. Keys are read from the
environment, from a repository-root `.env`, or from the OS keyring; an attached key is
redacted from all error text, so a failure log pasted into a ticket does not leak a
credential. `--config-dir` moves the whole directory for a sandboxed or shared host.

`decidr doctor` is the answer to "why is this not working": it lists the keys it found
*in the order they are used* and reports catalog mismatches, so a stale key in the
environment showing up ahead of the good one in the keyring is visible as an ordering
fact rather than inferred from behaviour. `decidr models` lists the models of TypeSafe or
of a configured local runner, which is how a provider catalog change is confirmed before
it is mistaken for an outage.

## Static-analysis Tool Builder — `sitr` ships alongside

**As a tool builder, I want to load a tree-sitter grammar and run any query against it from one command, so that a repository-wide structural check does not need a bespoke parser harness.**

`sitr` and the `sc_treesitter` Python module load tree-sitter grammars and run arbitrary
queries against them. They stand alone in the workspace — on no layer of the decision
stack — so they can be adopted by a repository that uses none of the rest of sc-agent.

---

## Upgrade Notes

- **Keys are never printed.** Expect redaction in error text; tooling that matched a
  full key in a diagnostic must match the redacted form.
- **Exit code 130 means cancelled.** An unfinished request is `decision.cancelled`, not
  an error, and the session log is flushed before exit.
- **Session logs grow without bound by design.** Point `<log_root>` at a filesystem you
  actually want to keep, or prune them outside sc-agent.
- **Crate layering is enforced.** Each crate depends only on the layers below it
  (ADR-SAG-0004); the runner is the top layer and no crate may depend on it. Consumers
  embedding the libraries should honour the same direction.
