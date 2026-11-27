# j-1 signatures v2 (spec; items and counts in j-1-signatures.json)

## Deliverables
1. J1-S02: struct fields append, in this order, after `local`:
2. J1-S03: const rows append in order: "max_sends", "decidr.luna-system-prompt", "
3. J1-S07: struct field append as last field: pub open: bool,
4. J1-S10: type aliases pub type DecisionRequest = Value;
5. J1-S11: trait pub trait Decision {
6. J1-S12: const pub const DEFAULT_SYSTEM_PROMPT: &str = "Answer every questi
7. J1-S14: struct pub struct Route<'a> {
8. J1-S15: types pub struct Jev<'a>(pub &'a Route<'a>);
9. J1-S17: impl impl Decision for Jev<'_> {
10. J1-S18: impls impl Decision for OpenAiCompat<'_> {
11. J1-S21: fn     pub fn check(&self) -> Result<(), Error> {
12. J1-S22: fn     pub fn recv<T>(&self, receiver: &Receiver<T>, id: &'stat
13. J1-S23: match arm add arm: "decision.api_status" => "other",   // after "http.
14. J1-S24: fn pub fn load(
15. J1-S25: fn pub fn doctor(config_dir: Option<&Path>, project: Option<&st
16. J1-S30: fn DELETED DELETED
17. J1-B01: struct #[derive(Debug)]
18. J1-B02: trait pub trait Llm {
19. J1-B04: impl impl Llm for OpenAiCompat<'_> {
20. J1-B05: impl impl Llm for AnthropicCompat<'_> {
21. O45-01: Runner { host, port } (model removed)
22. O45-02: Settings.local replaces Settings.laya
23. O45-03: Local rows: laya, decider_4b, decider_2b
24. O45-04: KEYS: decider endpoint keys, laya.model removed
25. O45-05: setting(): missing local setting by table
26. O45-06: send(): local base from Settings.local, resolved id on the wire
27. O45-07: candidates(): every row for a selector, newest first
28. O45-08: model_with/model keep signatures over candidates
29. O45-09: resolve(): keyless first, else first held key
30. O45-10: Nous rows: haiku, luna
31. O45-11: integration export: any available provider key

Exact code for each: the dev bead agt-j-1.group-dev Signatures and design "Public surface".

## Does Not Close
- The Rust `Run`, `RunArgs`, `Run::new`/`from_options`/`send`/`send_batch`, PyO3 `Run`/`Logger`, decidr on the Run, and deleting `Options.provider` (`Options.model` stays as the selector): j-2. j-1 keeps the free functions `send` and `batch`; they return errors unreported and callers `report`.
- Session row v2, `runs.jsonl`, `run_id`, tool identity, cache, and the reply usage pointers: j-3.
- The template: j-4.
- `event()` (split of `log`): j-3, its only user (session sinks).
- REQ-SAG-0503's catalog comparison: not in phase j.

## Behaviour changes the owner should know (none is pinned by a test or record I could find)
1. attempts is set on every failure after at least one send (and on cancel); before only on http failures and cancel. REQ-SAG-0016 says "when they apply".
2. decision.api_status and quota actions now use error()'s generic text; api_status category stays 'other'.
3. Anthropic reply text is read at /content/0/text (before: first block with text).
4. decidr's cancelled message is "request cancelled" (was "interrupted").
5. Each Decision impl runs schema() (ask for the compat impls, the Jev impl for Jev), so Decision::decide returns a schema-checked reply or decision.schema_mismatch (REQ-SAG-0901); send() no longer calls schema().
   10. A cancel after a finished request overrides only a success; a lower-layer error (http.auth, ...) is kept (J-1-05).
   11. A failure reading the reply body is http.network and is resent like a failed send (J-1-14); a sender waiting on the host holdoff ends at once on cancel (J-1-15).
   12. decidr has no post-send cancel check: send() and the batch already return decision.cancelled for what a cancel interrupted and keep lower-layer errors (J-1-05); decidr prints every result and exits non-zero when cancelled (status 0 only if the value is ok and the Cancel is not cancelled, REQ-SAG-0011; J-1-16).
6. Network failures share the busy path but still take no host lock.
7. The compat Decision path now sends the prompt as a system message plus the bare request JSON as the user message (ANC-014); before it sent one user message "Answer every question in this decision request using its required schema, including probabilities and confidence: <request>" (old wire(), lib.rs:904).
8. Provider.open appears in decidr's providers JSON (json!(PROVIDERS), decidr main.rs:36).
9. doctor gains rows for max_sends and the two decidr.*-system-prompt keys (KEYS).

## Gap closures
base_url: Route.base = options.base_url.clone().unwrap_or(base) at construction (in send). Key function is key(), not resolve(). Loop form: `for attempt in 1..=max`. Prompt key: format!("{}-system-prompt", options.model) as typed, else the resolved row's short name (a haiku/luna chosen by full id finds its setting). Schema wrapper, user text, response_format name and the Anthropic header table are in the code below. Tests need no BTreeMap import (Default::default()). Route is item S14. Gate change applied (S29).

## Exact code
### Signature items (one copy)
Aliases, LlmRequest, Decision, Llm, DEFAULT_SYSTEM_PROMPT, Auth consts, Route, Jev/OpenAiCompat/AnthropicCompat, the Decision and Llm impls, Cancel::check/recv/lock, load and doctor are specified once, in the closed Signatures list of agt-j-1.group-dev (items J1-S02..S30, B01..B05, O45-01..11, S31). `const ANTHROPIC_MAX_TOKENS: u32 = 64000;` (the Messages API's required max_tokens; the smallest documented maximum output among the Messages-wire rows, so no row is sent more than its model allows; lower it if a row documents less; verified at implementation; not an sc-agent cap). This design keeps only the bodies that list does not carry: ask, Route::text, Route::post, send, the consumer diffs.

### ask (S16)
```rust
fn ask(llm: &impl Llm, route: &Route<'_>, req: &DecisionRequest) -> Result<DecisionResult, Error> {
    // The override is keyed by the name as typed, else by the resolved row's short name, so a haiku/luna chosen by full id finds its setting (only decidr.haiku-/luna-system-prompt exist).
    let decidr = &route.options.settings.decidr;
    let short = route.provider.models.iter().find(|row| row.1 == route.model).map_or(route.options.model.as_str(), |row| row.0);
    let prompt = decidr
        .get(&format!("{}-system-prompt", route.options.model))
        .or_else(|| decidr.get(&format!("{short}-system-prompt")));
    let questions = req
        .get("questions")
        .and_then(Value::as_object)
        .into_iter()
        .flatten();
    let answers = questions
        .map(|(id, question)| (id, answer_schema(question)))
        .collect::<BTreeMap<_, _>>();
    let reply = llm.call(&LlmRequest {
        system: Some(prompt.map_or(DEFAULT_SYSTEM_PROMPT, String::as_str).into()),
        messages: json!([{"role":"user","content":req.to_string()}]),
        schema: Some(json!({
            "type":"object", "required":["answers"], "additionalProperties":false,
            "properties":{"answers":{"type":"object","properties":answers,"required":answers.keys().collect::<Vec<_>>(),"additionalProperties":false}}
        })),
    })?;
    // REQ-SAG-0901: a reply that misses the schema is decision.schema_mismatch from the decision backend itself.
    schema(req, &reply).map(|()| reply)
}
```

### Route::text (B06)
```rust
    fn text(&self, path: &str, auth: Auth, mut body: Value, at: &str) -> Result<Value, Error> {
        if let Some(fields) = body.as_object_mut() {
            fields.retain(|_, field| !field.is_null());
        }
        let reply = self.post(path, auth, &body)?;
        let text = reply.pointer(at).and_then(Value::as_str);
        let text = text.ok_or_else(|| {
            error(
                "decision.response_parse",
                format!("provider reply has no text: {reply}"),
            )
        })?;
        serde_json::from_str(text)
            .map_err(|e| error("decision.response_parse", format!("{e}: {text}")))
    }
```

### Route::post (S19)
```rust
    fn post(&self, path: &str, auth: Auth, body: &Value) -> Result<Value, Error> {
        let (cancel, provider, max) = (self.cancel, self.provider, self.options.settings.max_sends);
        let url = format!("{}/{path}", self.base.trim_end_matches('/'));
        let uri = url
            .parse::<ureq::http::Uri>()
            .map_err(|e| error("http.network", e))?;
        let host = uri
            .host()
            .ok_or_else(|| error("http.network", "endpoint has no host"))?
            .to_ascii_lowercase();
        let loopback = host == "localhost"
            || (host.trim_matches(['[', ']']).parse::<std::net::IpAddr>())
                .is_ok_and(|ip| ip.is_loopback());
        if !provider.key.is_empty() && uri.scheme_str() != Some("https") && !loopback {
            return Err(error(
                "http.key_on_plain_http",
                format!(
                    "{} is never sent to the plain-http host {host}",
                    provider.key
                ),
            ));
        }
        let pause = host_holdoff(&host)?;
        let key = match provider.key {
            "" => String::new(),
            name => key(
                name,
                &self.options.environment,
                &self.options.root,
                self.keyring,
            )?,
        };
        let redact = |text: String| {
            if key.is_empty() {
                text
            } else {
                text.replace(&key, "[redacted]")
            }
        };
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(self.options.timeout))
            .build()
            .new_agent();
        for attempt in 1..=max {
            cancel.check()?;
            // A busy reply holds off senders to this host through its injected wait; a waiting sender ends at once on cancel (REQ-SAG-0601).
            drop(cancel.lock(&pause)?);
            let mut post = agent.post(&url).header("Content-Type", "application/json");
            for (name, value) in auth.iter().filter(|_| !key.is_empty()) {
                post = post.header(*name, value.replace("{}", &key));
            }
            self.sends.set(attempt);
            // A send in flight cannot be aborted, so on cancel it is abandoned and the request ends now.
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            let sending = body.clone();
            std::thread::spawn(move || {
                let _ = sender.send(post.send_json(&sending).map(|mut reply| {
                    let delay = reply
                        .headers()
                        .get("Retry-After")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|h| h.parse::<f64>().ok())
                        .and_then(|s| Duration::try_from_secs_f64(s).ok());
                    (
                        reply.status().as_u16(),
                        delay,
                        reply.body_mut().read_to_string(),
                    )
                }));
            });
            // A failure reading the reply body is a network failure like a failed send: status 0, resent (REQ-SAG-0601).
            let (status, delay, text) = match cancel.recv(&receiver, "http.network")? {
                Ok((status, delay, Ok(text))) => (status, delay, redact(text)),
                Ok((_, _, Err(e))) | Err(e) => (0, None, redact(e.to_string())),
            };
            if (200..300).contains(&status) {
                return serde_json::from_str(&text)
                    .map_err(|e| error("decision.response_parse", format!("{e}: {text}")));
            }
            let low = text.to_lowercase();
            let quota = status == 402
                || [
                    "insufficient_quota",
                    "quota_exhausted",
                    "credit balance",
                    "spend limit",
                ]
                .iter()
                .any(|needle| low.contains(needle));
            let busy = [429, 502, 529].contains(&status) && !quota;
            if (busy || status == 0) && attempt < max {
                let _hold = (status != 0).then(|| cancel.lock(&pause)).transpose()?;
                (self.wait)(delay.unwrap_or_else(|| Duration::from_secs(2u64.pow(attempt - 1))));
                continue;
            }
            let id = match status {
                _ if quota => "http.quota_exhausted",
                _ if busy => "http.provider_busy",
                0 => "http.network",
                401 => "http.auth",
                _ => "decision.api_status",
            };
            return Err(error(id, text));
        }
        Err(error("http.network", "send count exhausted"))
    }
```

### send (S20)
```rust
pub fn send(
    request: &Value,
    options: &Options,
    logger: &Logger,
    cancel: &Cancel,
    wait: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync),
) -> Result<Value, Error> {
    let session_id = ulid::Ulid::new().to_string();
    let digests = request
        .get("questions")
        .and_then(Value::as_object)
        .map(|questions| {
            questions
                .keys()
                .filter_map(|id| digest(request, id).ok().map(|hash| (id.clone(), hash)))
                .collect::<BTreeMap<_, _>>()
        });
    let sends = Cell::new(0);
    let mut outcome = (|| {
        cancel.check()?;
        let project = options.settings.project_required()?;
        slug(project)?;
        slug(&options.agent_role)?;
        let found = candidates(
            &options.model,
            options.provider.as_deref(),
            &options.settings.models,
        )?;
        let (provider, model) = resolve(found, options, keyring)?;
        let base = if provider.url.is_empty() {
            let runner = options
                .settings
                .local
                .get(provider.name)
                .ok_or_else(|| missing_setting(&format!("{}.host", provider.name)))?;
            format!("http://{}:{}/v1", runner.host, runner.port)
        } else {
            provider.url.into()
        };
        let route = Route {
            options,
            provider,
            model,
            base: options.base_url.clone().unwrap_or(base),
            cancel,
            wait,
            keyring,
            sends: &sends,
        };
        // Each Decision impl returns a schema-checked reply; the wire is a typed enum matched exhaustively (ANC-012).
        match provider.wire {
            Wire::Messages => AnthropicCompat(&route).decide(request),
            Wire::Chat => OpenAiCompat(&route).decide(request),
            Wire::Jev => Jev(&route).decide(request),
        }
    })();
    // A cancel after a finished send does not replace a lower-layer error (ADR-SAG-0004, REQ-SAG-0002): only a success is overridden.
    if outcome.is_ok()
        && let Err(e) = cancel.check()
    {
        outcome = Err(e);
    }
    if let Err(Error::Failure {
        session_id: id,
        attempts,
        ..
    }) = &mut outcome
    {
        *id = Some(session_id.clone());
        *attempts = (sends.get() > 0 || cancel.is_cancelled()).then(|| sends.get());
    }
    if let Ok(value) = &mut outcome
        && let Some(fields) = value.as_object_mut()
    {
        fields.insert("attempts".into(), json!(sends.get()));
        fields.insert("session_id".into(), json!(session_id));
        fields.insert("question_digests".into(), json!(digests));
    }
    if let Ok(project) = options.settings.project_required()
        && slug(project).is_ok()
        && slug(&options.agent_role).is_ok()
    {
        let row = json!({"session_id":session_id,"repo":options.repo,"agent_role":options.agent_role,"project":project,"request":request,"question_digests":digests,"result":match &outcome {Ok(v)=>json!({"ok":true,"value":v}),Err(e)=>json!({"ok":false,"error":e})}});
        let path = options
            .log_root
            .join("sessions")
            .join(&options.repo)
            .join(&options.agent_role)
            .join(project);
        let _append = SESSION_APPEND
            .lock()
            .map_err(|e| error("config.session", e))?;
        let written = (|| -> std::io::Result<()> {
            use std::io::Write;
            std::fs::create_dir_all(&path)?;
            writeln!(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path.join("sessions.jsonl"))?,
                "{row}"
            )
        })();
        if let Err(e) = written {
            log(logger, "error", json!(error("log.session_write", e)))?;
        }
    }
    if let Ok(value) = &outcome {
        log(logger, "decision.result", json!({"response":value}))?;
    }
    logger.flush().map_err(|e| error("log.flush", e))?;
    outcome
}
```

### Consumer and gate diffs (S26, S27, S29)
```diff
diff --git a/crates/decidr/src/main.rs b/crates/decidr/src/main.rs
index 29b6d080..063c5c95 100644
--- a/crates/decidr/src/main.rs
+++ b/crates/decidr/src/main.rs
@@ -39,13 +39,7 @@ fn main() {
                     return sc_agent::doctor(cli.config_dir.as_deref(), cli.project.as_deref());
                 }
-                let (root, repo) = sc_agent::repository()?;
-                let local = root.join(".sc/sc-agent");
-                let mut settings = sc_agent::config(
-                    root.join(".git").exists().then_some(local.as_path()),
-                    cli.config_dir.as_deref(),
-                    &std::env::vars_os().collect::<Vec<_>>(),
-                    cli.project.as_deref(),
-                )?
-                .config;
+                let (loaded, root, repo) =
+                    sc_agent::load(cli.config_dir.as_deref(), cli.project.as_deref())?;
+                let mut settings = loaded.config;
                 let Command::Send {
                     model,
@@ -63,7 +57,5 @@ fn main() {
                     settings.workers = sc_agent::workers(workers)?;
                 }
-                if cancelled.is_cancelled() {
-                    return Err(sc_agent::error("decision.cancelled", "interrupted"));
-                }
+                cancelled.check()?;
                 let text = match input {
                     Some(path) => std::fs::read_to_string(path)
@@ -76,17 +68,7 @@ fn main() {
                             let _ = sender.send(read);
                         });
-                        loop {
-                            if cancelled.is_cancelled() {
-                                return Err(sc_agent::error("decision.cancelled", "interrupted"));
-                            }
-                            match receiver.recv_timeout(Duration::from_millis(20)) {
-                                Ok(value) => {
-                                    break value
-                                        .map_err(|e| sc_agent::error("decidr.input_read", e))?;
-                                }
-                                Err(mpsc::RecvTimeoutError::Timeout) => {}
-                                Err(e) => return Err(sc_agent::error("decidr.input_read", e)),
-                            }
-                        }
+                        cancelled
+                            .recv(&receiver, "decidr.input_read")?
+                            .map_err(|e| sc_agent::error("decidr.input_read", e))?
                     }
                 };
@@ -139,7 +121,5 @@ fn main() {
                     )
                 };
-                if cancelled.is_cancelled() {
-                    return Err(sc_agent::error("decision.cancelled", "interrupted"));
-                }
                 result
@@ status @@
-    let status = if value.get("ok") == Some(&Value::Bool(true)) {
+    let status = if value.get("ok") == Some(&Value::Bool(true)) && !cancelled.is_cancelled() {
             })();
diff --git a/crates/sc-agent-py/src/lib.rs b/crates/sc-agent-py/src/lib.rs
index 250bbd24..47317039 100644
--- a/crates/sc-agent-py/src/lib.rs
+++ b/crates/sc-agent-py/src/lib.rs
@@ -156,17 +156,8 @@ fn options(
     let model = option::<String>(named, "model")?.unwrap_or_else(|| "jev".into());
     let provider = option::<String>(named, "provider")?;
-    let (root, repo) = sc_agent::repository()?;
     let global = option::<String>(named, "config_dir")?.map(PathBuf::from);
     let project = option::<String>(named, "project")?;
-    let mut settings = sc_agent::config(
-        root.join(".git")
-            .exists()
-            .then_some(root.join(".sc/sc-agent"))
-            .as_deref(),
-        global.as_deref(),
-        &std::env::vars_os().collect::<Vec<_>>(),
-        project.as_deref(),
-    )?
-    .config;
+    let (loaded, root, repo) = sc_agent::load(global.as_deref(), project.as_deref())?;
+    let mut settings = loaded.config;
     sc_agent::model_with(&model, provider.as_deref(), &settings.models)?;
     if let Some(workers) = workers {
diff --git a/scripts/gate/rewrite.py b/scripts/gate/rewrite.py
index bd724222..e5996805 100644
--- a/scripts/gate/rewrite.py
+++ b/scripts/gate/rewrite.py
@@ -88,5 +88,5 @@ check("forbidden architecture", (
     f"{p.relative_to(ROOT)}:{i}: {match.group()}"
     for p in rust for i, line in enumerate(p.read_text().splitlines(), 1)
-    for match in re.finditer(r"\b(?:Transport|Clock|CancelToken|Catalog)\b|\btrait\s+|\bpub\s+use\b", line)
+    for match in re.finditer(r"\b(?:Transport|Clock|CancelToken|Catalog)\b|\btrait\s+(?!(?:Decision|Llm)\b)|\bpub\s+use\b", line)
 ))
 # The only permitted builder use is in the default_logger function.
```

## Placement and order (no latitude)
- Rows: `open: <bool>,` is the last field of each Provider row, after `models:`. Struct: `pub open: bool,` after `models`.
- lib.rs order, after `answer_schema` (where wire/response were): the two aliases, LlmRequest (with #[derive(Debug)]), Decision, Llm, DEFAULT_SYSTEM_PROMPT, Auth, BEARER, ANTHROPIC, Route, Jev, OpenAiCompat, AnthropicCompat, ask, impl Decision for Jev, OpenAiCompat, AnthropicCompat, impl Llm for OpenAiCompat, AnthropicCompat, then `impl Route<'_> { fn text; fn post }`, then the existing HOLDOFFS. Cancel gets `check`, `recv`, then the existing `wait`. `load` goes directly before `doctor`.

## S28 docli edit in full (scripts/docli_from_clap.rs; every edit, nothing else changes)
1. Delete the whole `CliModelError` block: the doc comment "/// Failure while reading arguments or writing a model file.", the struct, `impl Display`, `impl Error`, and `impl CliModelError { fn new }` (old lines 13-34).
2. Delete both `#[cfg_attr(not(test), allow(dead_code))]` impl blocks, `impl CliModel` (name, subcommands, options) and `impl OptionSpec` (long, name), with their doc comments (old lines 88-114).
3. `output_path_from_args` returns `Result<PathBuf, String>`; the error arm is `Err("usage: gen-cli-model --output <path>".into())`.
4. `write_model` returns `Result<(), String>`; each `.map_err(|error| CliModelError::new(format!(..)))` becomes `.map_err(|error| format!(..))`.
5. In the test module use fields, not methods: `model.name`, `option.name`, `verbose.long.as_deref()`, `model.options.iter()`, `model.subcommands.iter().map(|c| c.name.as_str())`. No method on CliModel or OptionSpec remains, so no test may call `.name()`, `.long()`, `.options()` or `.subcommands()`.
Full resulting diff:
```diff
diff --git a/scripts/docli_from_clap.rs b/scripts/docli_from_clap.rs
index ff7573a2..aa55fc56 100644
--- a/scripts/docli_from_clap.rs
+++ b/scripts/docli_from_clap.rs
@@ -10,26 +10,4 @@ use clap::{Arg, Command};
 use serde::Serialize;
 
-/// Failure while reading arguments or writing a model file.
-#[derive(Debug)]
-pub struct CliModelError {
-    message: String,
-}
-
-impl std::fmt::Display for CliModelError {
-    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
-        formatter.write_str(&self.message)
-    }
-}
-
-impl std::error::Error for CliModelError {}
-
-impl CliModelError {
-    fn new(message: impl Into<String>) -> Self {
-        Self {
-            message: message.into(),
-        }
-    }
-}
-
 /// A command (or subcommand) in the neutral model.
 #[derive(Debug, Serialize, PartialEq, Eq)]
@@ -72,35 +50,4 @@ pub struct ArgumentSpec {
 }
 
-#[cfg_attr(not(test), allow(dead_code))]
-impl CliModel {
-    /// Command name.
-    pub fn name(&self) -> &str {
-        &self.name
-    }
-
-    /// Nested subcommands, sorted by name.
-    pub fn subcommands(&self) -> &[CliModel] {
-        &self.subcommands
-    }
-
-    /// Flags and options, sorted by name. Auto-injected help and version are omitted.
-    pub fn options(&self) -> &[OptionSpec] {
-        &self.options
-    }
-}
-
-#[cfg_attr(not(test), allow(dead_code))]
-impl OptionSpec {
-    /// Long form, including dashes.
-    pub fn long(&self) -> Option<&str> {
-        self.long.as_deref()
-    }
-
-    /// Canonical identifier.
-    pub fn name(&self) -> &str {
-        &self.name
-    }
-}
-
 /// Build a model from `command` (typically `Cli::command()`).
 pub fn from_clap(command: &Command) -> CliModel {
@@ -109,5 +56,5 @@ pub fn from_clap(command: &Command) -> CliModel {
 
 /// `--output <path>` from the process arguments.
-pub fn output_path_from_args() -> Result<PathBuf, CliModelError> {
+pub fn output_path_from_args() -> Result<PathBuf, String> {
     let mut args = std::env::args().skip(1);
     match (args.next().as_deref(), args.next()) {
@@ -115,21 +62,20 @@ pub fn output_path_from_args() -> Result<PathBuf, CliModelError> {
             Ok(PathBuf::from(path))
         }
-        _ => Err(CliModelError::new("usage: gen-cli-model --output <path>")),
+        _ => Err("usage: gen-cli-model --output <path>".into()),
     }
 }
 
 /// Write `model` as pretty JSON, creating parent directories when needed.
-pub fn write_model(path: &Path, model: &CliModel) -> Result<(), CliModelError> {
+pub fn write_model(path: &Path, model: &CliModel) -> Result<(), String> {
     if let Some(parent) = path.parent()
         && !parent.as_os_str().is_empty()
     {
         std::fs::create_dir_all(parent)
-            .map_err(|error| CliModelError::new(format!("create {}: {error}", parent.display())))?;
+            .map_err(|error| format!("create {}: {error}", parent.display()))?;
     }
-    let mut bytes = serde_json::to_vec_pretty(model)
-        .map_err(|error| CliModelError::new(format!("encode model: {error}")))?;
+    let mut bytes =
+        serde_json::to_vec_pretty(model).map_err(|error| format!("encode model: {error}"))?;
     bytes.push(b'\n');
-    std::fs::write(path, bytes)
-        .map_err(|error| CliModelError::new(format!("write {}: {error}", path.display())))
+    std::fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
 }
 
@@ -252,5 +198,5 @@ mod tests {
 
         let model = from_clap(&command);
-        assert_eq!(model.name(), "demo");
+        assert_eq!(model.name, "demo");
         assert_eq!(model.version.as_deref(), Some("1.2.3"));
         assert_eq!(model.description, "Demo tool");
@@ -266,18 +212,13 @@ mod tests {
 
         let verbose = model
-            .options()
+            .options
             .iter()
-            .find(|option| option.name() == "verbose")
+            .find(|option| option.name == "verbose")
             .expect("flag");
-        assert_eq!(verbose.long(), Some("--verbose"));
-        assert!(model.options().iter().all(|option| option.name() != "help"));
-        assert!(
-            model
-                .options()
-                .iter()
-                .all(|option| option.name() != "version")
-        );
+        assert_eq!(verbose.long.as_deref(), Some("--verbose"));
+        assert!(model.options.iter().all(|option| option.name != "help"));
+        assert!(model.options.iter().all(|option| option.name != "version"));
 
-        let names: Vec<&str> = model.subcommands().iter().map(CliModel::name).collect();
+        let names: Vec<&str> = model.subcommands.iter().map(|c| c.name.as_str()).collect();
         assert_eq!(names, ["run"]);
     }
```

## Unresolved owner decisions
1. The behaviour changes above (1-6). 2. Key name max_sends. 3. Prompt key by typed name, else the resolved row's short name. 4. Whether to keep the docli dead-code deletion (outside sc-agent). 5. Cache hash definition (O3) and Run vs ADR-0001 (O1). 6. ANC-009 CLI half is j-2.

## Units O4/O5 (owner rulings 2026-11-24; placement: lead decision)
Both are new capability, counted separately from the -77 restructure. Code lands on sprint/j-1 @b4be55e6 and is updated in place. Items are in scratchpad/phase-j/j-1-o45-beads.json (one per public-surface item).

Goal (Rand's words): O4 "restore decider (Strands Decider) as a local model exactly like laya ... --model decider" (REQ-SAG-0801: decider resolves to the 4b row; each runner's model has a fixed id in the table; address from configuration). O5 "haiku/luna required for all tools with no TYPESAFE_API_KEY ... send works keyless for haiku/luna via provider keys" (REQ-SAG-0103: prefer a keyless provider; else the first whose key is found). Measure: the named tests below pass; no provider or model name is compared outside PROVIDERS.

Layout: crates/sc-agent/src/lib.rs (all of it, no providers.rs); scripts/integration.py and tests/contract/test_integration.py (export); crates/sc-agent/tests/{config_and_values,sending,records,wires,support}.rs; tests/contract/test_contract.py and crates/sc-agent-py/tests/loopback.py (laya fixtures lose "model").

### Public surface
O4-1..O4-6 and O5-1..O5-5 are the closed items O45-01..O45-11 of agt-j-1.group-dev (one copy), plus O45-12 (rows newest first, REQ-SAG-0102, restructure net 0).
O45-11 (integration export) reads the keyed provider variables by regex over crates/sc-agent/src/lib.rs, with a literal subset in Python: an interim j-1 shortcut (NFR-SAG-0004). It is deferred explicitly to j-2, whose `sc_agent.provider_keys()` and `decidr models` replace the regex and the literal list; j-1 adds no provider-key accessor.

### External contracts (examples)
Config (global or local decidr.json / sc-agent.json, or env):
`{"decider_4b":{"host":"127.0.0.1","port":18001},"decider_2b":{"host":"127.0.0.1","port":18000},"laya":{"host":"127.0.0.1","port":11435}}`; env DECIDR_DECIDER_4B__HOST. `laya.model` is now an unknown-key error (config.invalid naming laya.model). An unset runner: `config.missing_setting` naming `decider_2b.host`.
Wire to the 4b stub: POST /v1/systemone body `{"state":..,"questions":..,"model":"decider-4b-v2.1"}`, no auth header. laya: "model":"laya:en".
Resolution rows (keys held -> `haiku`): ANTHROPIC only -> anthropic claude-haiku-5-5; NOUS only -> nous anthropic/claude-haiku-5.5; OPENROUTER only -> openrouter; none -> secrets.key_missing naming ANTHROPIC_API_KEY, OPENROUTER_API_KEY, TOGETHER_API_KEY, NOUS_API_KEY; `luna` with NOUS only -> nous openai/gpt-6-luna.
Doctor rows: the four decider keys appear; laya.model does not.

### Deletions (file:line @b4be55e6)
lib.rs:121-125 Runner.model; :139 Settings.laya; :160-162 starts_with("laya."); :177 "laya.model"; :281-289 laya block; :710-718 "local" row; :732-765 per-provider max_by_key body; :1199-1210 laya branch of send(). Tests: sending.rs:19,34-40,56-62 (provider "local", configured-local), config_and_values.rs:273,414,437,517-545 (laya.model cases), test_contract.py:636,737,816, loopback.py:105 (laya "model").

### Done checks (all deterministic: loopback Stub, injected environment and keyring seam, no sleeps)
Rust, crates/sc-agent/tests/wires.rs: `local_endpoints` (decider 4b stub body model decider-4b-v2.1 and no auth header; decider_2b stub receives decider-2b-v11; laya "laya:en"; unset decider_2b.host is config.missing_setting naming it). `keyless_resolution` (a table: ANTHROPIC only; NOUS only for haiku and luna with the nous ids at the stub; OPENROUTER only; TOGETHER+NOUS picks together; no key and a keyring seam that errors surfaces secrets.keyring_read, not key_missing) and `keyless_resolution_names_every_missing_key_and_redacts_the_used_one` (a no-entry seam names ANTHROPIC, OPENROUTER, TOGETHER and NOUS keys; a nous stub echoing its key in a 401 returns text with [redacted] and without the key). lib.rs unit test `resolve_test::a_keyless_candidate_beats_a_keyed_one_and_is_never_looked_up` (no table row mixes keyless and keyed, so the order is pinned on resolve directly).
config_and_values.rs: `config_local_endpoints` (decider keys decode into Settings.local; laya.model is the unknown-key error; doctor rows show the four decider keys). Existing tests ported: sending.rs provider table (laya via "laya"), models_use_provider_tables (model("decider") is decider_4b, model("laya") is "laya"), the laya.model cases in config_and_values.rs use laya.host.
Python: tests/contract/test_integration.py `test_export_requires_one_of_typesafe_anthropic_nous` and the updated export test; test_contract.py laya fixtures without "model".
Gate: `cargo test --workspace --all-features` (1.98.1), `cargo fmt --check`, `cargo clippy ... -D warnings`, `pytest tests/contract`, `python3 scripts/gate/rewrite.py` (Rust source lines N/2580; every push).

### Budget (against 2,580)
Measured on sprint/j-1 after the build, rewrite.py: Rust source 2402/2580 (O4+O5 are +62). scripts/integration.py and the Python tests are not counted by the gate.

## Signatures (closed list)

1. J1-S02: struct fields append, in this order, after `laya`:

Public-surface item J1-S02 (part A: restructure), struct fields in crates/sc-agent/src/lib.rs.

After (exact):
```rust
append, in this order, after `laya`:
    pub max_sends: u32,
    pub decidr: BTreeMap<String, String>,
```
Replaces: lib.rs:112-119 @8f68424a `pub struct Settings { project, workers, models, laya }`
Deletes: nothing
Body rule: max_sends caps sends of one request; decidr holds prompt overrides keyed '<name>-system-prompt'.
Test: config_and_values.rs::max_sends_and_system_prompt_keys_load_default_and_report: defaults give max_sends 5 source default; file max_sends 3 and decidr.luna-system-prompt=P load; max_sends 0 and decidr.sonnet-system-prompt are rejected with config.invalid
Requirement: ['REQ-SAG-0601', 'ANC-014']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

2. J1-S03: const rows append in order: "max_sends", "decidr.luna-system-prompt", "

Public-surface item J1-S03 (part A: restructure), const rows in crates/sc-agent/src/lib.rs.

After (exact):
```rust
append in order: "max_sends", "decidr.luna-system-prompt", "decidr.haiku-system-prompt"
```
Replaces: lib.rs:148-155 @8f68424a `const KEYS: &[&str]`
Deletes: nothing
Body rule: Closed key list; any other decidr.* key is config.invalid.
Test: config_and_values.rs::max_sends_and_system_prompt_keys_load_default_and_report: defaults give max_sends 5 source default; file max_sends 3 and decidr.luna-system-prompt=P load; max_sends 0 and decidr.sonnet-system-prompt are rejected with config.invalid
Requirement: ['REQ-SAG-0601', 'ANC-014']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

3. J1-S07: struct field append as last field: pub open: bool,

Public-surface item J1-S07 (part A: restructure), struct field in crates/sc-agent/src/lib.rs.

After (exact):
```rust
append as last field: pub open: bool,
```
Replaces: lib.rs:592-599 @8f68424a `pub struct Provider { name, url, key, wire, models }`
Deletes: nothing
Body rule: open means an id not in the rows is accepted as typed.
Test: existing config_and_values.rs model tests plus sending.rs openrouter/together/nous cases (open) and openai unknown id (closed)
Requirement: ['REQ-SAG-0102']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

4. J1-S10: type aliases pub type DecisionRequest = Value;

Public-surface item J1-S10 (part A: restructure), type aliases in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub type DecisionRequest = Value;
pub type DecisionResult = Value;
```
Replaces: (new) 
Deletes: nothing
Body rule: Aliases for serde_json::Value.
Test: compile-time
Requirement: ['owner design']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

5. J1-S11: trait pub trait Decision {

Public-surface item J1-S11 (part A: restructure), trait in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub trait Decision {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error>;
}
```
Replaces: (new) 
Deletes: nothing
Body rule: Owner trait, one method; the returned value is NOT schema-validated here (send validates, J1-S20).
Test: wires.rs::jev_decide_posts_the_request_with_the_resolved_model: Jev.decide posts request+model=jev-latest to /v1/systemone and returns the stub answer
Requirement: ['ANC-012']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

6. J1-S12: const pub const DEFAULT_SYSTEM_PROMPT: &str = "Answer every questi

Public-surface item J1-S12 (part A: restructure), const in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub const DEFAULT_SYSTEM_PROMPT: &str = "Answer every question in the decision request. Reply with one JSON object that matches the supplied schema, including probabilities and confidence.";
```
Replaces: (new) 
Deletes: nothing
Body rule: Default prompt text, exact.
Test: wires.rs::luna_and_haiku_send_the_default_then_the_configured_system_prompt: unset key sends this const, decidr.<name>-system-prompt=X sends X (luna messages[0].content, haiku top-level system)
Requirement: ['ANC-014']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

7. J1-S14: struct pub struct Route<'a> {

Public-surface item J1-S14 (part A: restructure), struct in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub struct Route<'a> {
    pub options: &'a Options,
    pub provider: &'static Provider,
    pub model: String,
    pub base: String,
    pub cancel: &'a Cancel,
    pub wait: &'a (dyn Fn(Duration) + Sync),
    pub keyring: &'a (dyn Fn(&str) -> Result<String, KeyringError> + Sync),
    pub sends: &'a Cell<u32>,
}
```
Replaces: (new) 
Deletes: nothing
Body rule: The one grouped parameter set (ANC-008), built once per send(). base = options.base_url if set, else the provider url (or `Settings.local[provider.name]` for a local row). sends records the attempt number of the last send.
Test: wires.rs::max_sends_setting_limits_the_sends_of_one_request: max_sends=2 and two 502 replies give http.provider_busy with attempts 2 and exactly 2 stub requests
Requirement: ['ANC-008', 'REQ-SAG-0601']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

8. J1-S15: types pub struct Jev<'a>(pub &'a Route<'a>);

Public-surface item J1-S15 (part A: restructure), types in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub struct Jev<'a>(pub &'a Route<'a>);
pub struct OpenAiCompat<'a>(pub &'a Route<'a>);
pub struct AnthropicCompat<'a>(pub &'a Route<'a>);
```
Replaces: (new) 
Deletes: nothing
Body rule: One type per wire format.
Test: compile-time
Requirement: ['ANC-012', 'ANC-013']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

9. J1-S17: impl impl Decision for Jev<'_> {

Public-surface item J1-S17 (part A: restructure), impl in crates/sc-agent/src/lib.rs.

After (exact):
```rust
impl Decision for Jev<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        let mut body = req.clone();
        if let Some(body) = body.as_object_mut() {
            body.insert("model".into(), json!(self.0.model));
        }
        let reply = self.0.post("systemone", BEARER, &body)?;
        schema(req, &reply).map(|()| reply)
    }
}
```
Replaces: lib.rs:879-887 @8f68424a `wire() systemone arm`
Deletes: nothing
Body rule: Request plus model, posted to systemone with bearer; the reply is schema-checked here (decision.schema_mismatch, REQ-SAG-0901).
Test: J1-S11 test
Requirement: ['ANC-012']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

10. J1-S18: impls impl Decision for OpenAiCompat<'_> {

Public-surface item J1-S18 (part A: restructure), impls in crates/sc-agent/src/lib.rs.

After (exact):
```rust
impl Decision for OpenAiCompat<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        ask(self, self.0, req)
    }
}

impl Decision for AnthropicCompat<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        ask(self, self.0, req)
    }
}
```
Replaces: lib.rs:888-937 @8f68424a `wire()/response() chat/completions and messages arms`
Deletes: nothing
Body rule: Both bodies are exactly ask(self, self.0, req); ask returns a schema-checked reply (decision.schema_mismatch, REQ-SAG-0901).
Test: J1-S12 test
Requirement: ['ANC-012', 'ANC-014']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

11. J1-S21: fn     pub fn check(&self) -> Result<(), Error> {

Public-surface item J1-S21 (part A: restructure), fn in crates/sc-agent/src/lib.rs.

After (exact):
```rust
    pub fn check(&self) -> Result<(), Error> {
        match self.is_cancelled() {
            true => Err(error("decision.cancelled", "request cancelled")),
            false => Ok(()),
        }
    }
```
Replaces: lib.rs:977-978,1042-1048,1078-1079,1174-1175 @8f68424a `six inline `if cancel.is_cancelled() { return Err(error("decision.cancelled",...)) }``
Deletes: crates/sc-agent/src/lib.rs:977-979, 1042-1049, 1174-1176
Body rule: One cancellation check.
Test: existing sending.rs cancel tests
Requirement: ['ANC-010']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

12. J1-S22: fn     pub fn recv<T>(&self, receiver: &Receiver<T>, id: &'stat

Public-surface item J1-S22 (part A: restructure), fn in crates/sc-agent/src/lib.rs.

After (exact):
```rust
    pub fn recv<T>(&self, receiver: &Receiver<T>, id: &'static str) -> Result<T, Error> {
        loop {
            self.check()?;
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Err(RecvTimeoutError::Timeout) => {}
                other => return other.map_err(|e| error(id, e)),
            }
        }
    }
```
Replaces: lib.rs:1077-1087 @8f68424a `cancel-polling receive loop in send; same loop in decidr main.rs:78-93`
Deletes: crates/sc-agent/src/lib.rs:1077-1087
Body rule: Shared poll-and-check receive used by post and by decidr's stdin reader. Cancel owns check, recv and lock (S32); there is no Cancel::run.
Test: existing cancel tests (sending.rs, test_contract.py SIGINT/SIGTERM)
Requirement: ['ANC-010']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

13. J1-S23: match arm add arm: "decision.api_status" => "other",   // after "http.

Public-surface item J1-S23 (part A: restructure), match arm in crates/sc-agent/src/lib.rs.

After (exact):
```rust
add arm: "decision.api_status" => "other",   // after "http.network" => "network",
```
Replaces: lib.rs:57-70 @8f68424a `error() category match`
Deletes: nothing
Body rule: Category the removed hand-built Failure literal set.
Test: existing sending.rs status tests
Requirement: ['REQ-SAG-0017']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

14. J1-S24: fn pub fn load(

Public-surface item J1-S24 (part A: restructure), fn in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub fn load(
    config_dir: Option<&Path>,
    project: Option<&str>,
) -> Result<(Configuration, PathBuf, String), Error> {
    let (root, repo) = repository()?;
    let local = root.join(".sc/sc-agent");
    let environment = std::env::vars_os().collect::<Vec<_>>();
    let local = root.join(".git").exists().then_some(local.as_path());
    Ok((
        config(local, config_dir, &environment, project)?,
        root,
        repo,
    ))
}
```
Replaces: decidr main.rs:41-49, sc-agent-py lib.rs:158-171, lib.rs:1326-1337 (three copies) @8f68424a `repository() + local dir + config(...)`
Deletes: nothing
Body rule: The one place that finds the repo, picks the local config folder and loads settings.
Test: existing doctor/config contract tests (test_contract.py doctor tests)
Requirement: ['ANC-010']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

15. J1-S25: fn pub fn doctor(config_dir: Option<&Path>, project: Option<&st

Public-surface item J1-S25 (part A: restructure), fn in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub fn doctor(config_dir: Option<&Path>, project: Option<&str>) -> Result<Value, Error> {
    let (loaded, ..) = load(config_dir, project)?;
    Ok(json!({"settings":loaded.rows, "files":loaded.files, "log_root":log_root("decidr")?}))
}
```
Replaces: lib.rs:1326-1337 @8f68424a `pub fn doctor (12 lines)`
Deletes: crates/sc-agent/src/lib.rs:1326-1337
Body rule: Uses load().
Test: same as J1-S24
Requirement: ['ANC-010']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

16. J1-S30: fn DELETED DELETED

Public-surface item J1-S30 (part A: restructure), fn DELETED in crates/sc-agent/src/lib.rs.

After (exact):
```rust
DELETED
```
Replaces: lib.rs:879-937 @8f68424a `fn wire(request:&Value, model:&str, wire:&str)->Value; fn response(request:&Value, raw:Value, wire:&str)->Result<Value,Error>`
Deletes: crates/sc-agent/src/lib.rs:879-937 (answer_schema 823-878 kept)
Body rule: Replaced by J1-S16..S18 and B04..B06.
Test: existing sending.rs suite
Requirement: ['ANC-012']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

17. J1-B01: struct #[derive(Debug)]

Public-surface item J1-B01 (part B: new capability), struct in crates/sc-agent/src/lib.rs.

After (exact):
```rust
#[derive(Debug)]
pub struct LlmRequest {
    pub system: Option<String>,
    pub messages: Value,
    pub schema: Option<Value>,
}
```
Replaces: (new) 
Deletes: nothing
Body rule: Provider-neutral LLM request.
Test: openai_compat_call test
Requirement: ['ANC-013']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

18. J1-B02: trait pub trait Llm {

Public-surface item J1-B02 (part B: new capability), trait in crates/sc-agent/src/lib.rs.

After (exact):
```rust
pub trait Llm {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error>;
}
```
Replaces: (new) 
Deletes: nothing
Body rule: Owner trait, one method, JSON in and JSON out.
Test: anthropic_compat_call test
Requirement: ['ANC-013']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

19. J1-B04: impl impl Llm for OpenAiCompat<'_> {

Public-surface item J1-B04 (part B: new capability), impl in crates/sc-agent/src/lib.rs.

After (exact):
```rust
impl Llm for OpenAiCompat<'_> {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error> {
        let system = req
            .system
            .iter()
            .map(|text| json!({"role":"system","content":text}));
        let messages = system
            .chain(req.messages.as_array().cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        let format = req.schema.as_ref().map(|schema| json!({"type":"json_schema","json_schema":{"name":"decision","strict":true,"schema":schema}}));
        let body = json!({"model":self.0.model,"messages":messages,"response_format":format});
        self.0.text(
            "chat/completions",
            BEARER,
            body,
            "/choices/0/message/content",
        )
    }
}
```
Replaces: (new) 
Deletes: nothing
Body rule: System text becomes a first system message; schema becomes response_format {type:json_schema, json_schema:{name:decision, strict:true, schema}}; text at /choices/0/message/content.
Test: openai_compat_call_returns_the_json_the_model_wrote
Requirement: ['ANC-013']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

20. J1-B05: impl impl Llm for AnthropicCompat<'_> {

Public-surface item J1-B05 (part B: new capability), impl in crates/sc-agent/src/lib.rs.

After (exact):
```rust
impl Llm for AnthropicCompat<'_> {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error> {
        let format = req
            .schema
            .as_ref()
            .map(|schema| json!({"format":{"type":"json_schema","schema":schema}}));
        let body = json!({"model":self.0.model,"max_tokens":ANTHROPIC_MAX_TOKENS,"messages":req.messages,"system":req.system,"output_config":format});
        self.0.text("messages", ANTHROPIC, body, "/content/0/text")
    }
}
```
Replaces: (new) 
Deletes: nothing
Body rule: max_tokens is the Messages API's required field, set to the one constant ANTHROPIC_MAX_TOKENS (a provider limit, not an sc-agent cap; rule 0); system top-level; schema as output_config.format; text at /content/0/text.
Test: anthropic_compat_call_returns_the_json_the_model_wrote
Requirement: ['ANC-013']
Source: scratchpad/j-1-signatures.md; starting code on sprint/j-1 @b4be55e6.

21. O45-01: Runner { host, port } (model removed)

Public-surface item J1-O45-01 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
pub struct Runner { pub host: String, pub port: u16 }
```
Replaces: crates/sc-agent/src/lib.rs:121-125
Deletes: field Runner.model; key laya.model
New capability: False
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

22. O45-02: Settings.local replaces Settings.laya

Public-surface item J1-O45-02 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
pub struct Settings { ..., pub local: BTreeMap<String, Runner>, ... }
```
Replaces: lib.rs:139 and the laya block in config() lib.rs:281-289
Deletes: Settings.laya; the laya.* Option<Runner> branch
New capability: False
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

23. O45-03: Local rows: laya, decider_4b, decider_2b

Public-surface item J1-O45-03 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
Provider { name: "laya"|"decider_4b"|"decider_2b", url: "", key: "", wire: Wire::Jev, models: &[("laya","laya:en","2026-09-23")] | &[("decider","decider-4b-v2.1","2026-09-24")] | &[("decider","decider-2b-v11","2026-09-24")], open: false }
```
Replaces: lib.rs:710-718 (single "local" row)
Deletes: provider name "local"; the runner.model override
New capability: True
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

24. O45-04: KEYS: decider endpoint keys, laya.model removed

Public-surface item J1-O45-04 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
const KEYS: &[&str] = &[..., "laya.host", "laya.port", "decider_4b.host", "decider_4b.port", "decider_2b.host", "decider_2b.port", ...]
```
Replaces: lib.rs:171-181
Deletes: "laya.model"
New capability: True
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

25. O45-05: setting(): missing local setting by table

Public-surface item J1-O45-05 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
fn setting<T: serde::de::DeserializeOwned>(resolved: &Resolved, path: &str, default: Value) -> Result<T, Error> // missing <local provider>.<key> is config.missing_setting
```
Replaces: lib.rs:160-162 (starts_with("laya."), path: &'static str)
Deletes: the laya name test
New capability: False
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

26. O45-06: send(): local base from Settings.local, resolved id on the wire

Public-surface item J1-O45-06 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
base = http://{host}:{port}/v1 from options.settings.local[provider.name]; unset: config.missing_setting naming <provider.name>.host
```
Replaces: lib.rs:1199-1210
Deletes: hard-coded laya.host and runner.model
New capability: False
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

27. O45-07: candidates(): every row for a selector, newest first

Public-surface item J1-O45-07 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
fn candidates(name: &str, provider: Option<&str>, configured: &BTreeMap<String, ModelEntry>) -> Result<Vec<(&'static Provider, String)>, Error>
```
Replaces: body of model_with lib.rs:732-765
Deletes: per-provider max_by_key closure
New capability: True
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

28. O45-08: model_with/model keep signatures over candidates

Public-surface item J1-O45-08 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
pub fn model_with(name: &str, provider: Option<&str>, configured: &BTreeMap<String, ModelEntry>) -> Result<(&'static Provider, String), Error> // candidates(..)?.remove(0)
```
Replaces: lib.rs:732-765 bodies
Deletes: the duplicated lookup body
New capability: False
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

29. O45-09: resolve(): keyless first, else first held key

Public-surface item J1-O45-09 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
fn resolve(found: Vec<(&'static Provider, String)>, options: &Options, keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync)) -> Result<(&'static Provider, String), Error>
```
Replaces: direct model_with call in send() lib.rs:1194
Deletes: nothing (new selection rule, REQ-SAG-0103)
New capability: True
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

30. O45-10: Nous rows: haiku, luna

Public-surface item J1-O45-10 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
nous Provider models: &[("haiku","anthropic/claude-haiku-5.5","2026-10-07"),("luna","openai/gpt-6-luna","2026-09-22")]
```
Replaces: models: &[] on nous, lib.rs:703
Deletes: empty nous table
New capability: True
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

31. O45-11: integration export: any available provider key

Public-surface item J1-O45-11 (units O4 decider local row / O5 keyless haiku-luna).

Exact Rust:
```rust
scripts/integration.py --export-env: every keyed variable of the Rust table is looked up and written when held; one of TYPESAFE_API_KEY, ANTHROPIC_API_KEY, NOUS_API_KEY is required
```
Replaces: scripts/integration.py:35-39 (fixed typesafe/anthropic/openai list, TypeSafe required)
Deletes: the fixed provider list and the TypeSafe-required check
New capability: True
Source: scratchpad/j-1-signatures.md section 'Units O4/O5'; code sprint/j-1 @f8bb024a (rule-3 review PASS).

32. J1-S31: typed wire

Public-surface item J1-S31 (part A: restructure), enum in crates/sc-agent/src/lib.rs.

After (exact):
```rust
#[derive(Clone, Copy)]
pub enum Wire { Jev, Chat, Messages }   // Provider.wire: Wire (Jev for TypeSafe and the local rows; Chat and Messages for the compat providers)
```
Replaces: the `wire: &'static str` field of Provider and its string match in send()
Deletes: the string wire values and the catch-all arm (ANC-012: no wire strings outside the impls)
Body rule: send() matches Wire exhaustively, so a new wire is a compile error until an impl exists.
Test: existing sending.rs provider table (each row's wire reaches its stub path)
Requirement: ['ANC-012']
Source: plan-qa-4 finding applied to scratchpad/j-1-signatures.md; code sprint/j-1.

33. J1-S32: Cancel::lock: cancellable lock

Public-surface item J1-S32 (part A: restructure), fn in crates/sc-agent/src/lib.rs.

After (exact):
```rust
    pub fn lock<'a, T>(&self, mutex: &'a Mutex<T>) -> Result<MutexGuard<'a, T>, Error> {
        loop {
            self.check()?;
            match mutex.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::WouldBlock) => std::thread::park_timeout(Duration::from_millis(20)),
                Err(TryLockError::Poisoned(e)) => return Err(error("http.network", e)),
            }
        }
    }
```
Replaces: the two `pause.lock()` calls in Route::post (holdoff)
Deletes: the blocking holdoff lock
Body rule: A sender waiting on a host's holdoff polls Cancel like recv does, so cancel ends the wait at once (REQ-SAG-0601); lock() is a Cancel method beside check and recv. Cancel::run does not exist: Cancel's methods are cancel, is_cancelled, wait, check, recv, lock.
Test: wires.rs::cancel_ends_a_wait_on_the_host_holdoff (a test holds the holdoff mutex through a second host handle and cancels from the stub's respond handler; no sleep, no timing assertion)
Requirement: ['REQ-SAG-0601', 'ANC-010']
Source: plan-qa-4 finding applied to scratchpad/j-1-signatures.md; code sprint/j-1.

34. J1-O45-12: built-in model rows newest first

Public-surface item J1-O45-12 (part A: restructure), const rows in crates/sc-agent/src/lib.rs.

After (exact):
```rust
each Provider.models array lists its rows newest date first (REQ-SAG-0102); order within a date is unchanged
```
Replaces: the current row order of PROVIDERS in lib.rs (reorder only)
Deletes: nothing (+6/-6, restructure, net 0)
Body rule: REQ-SAG-0102 requires built-in rows newest first; candidates() sorts by date but listings (decidr models, doctor) show table order.
Test: config_and_values.rs::built_in_rows_are_newest_first (every PROVIDERS row's model dates are non-increasing)
Requirement: ['REQ-SAG-0102']
Source: plan-qa-4 finding applied to scratchpad/j-1-signatures.md; code sprint/j-1.

Release check: decider requests carry the row's model id (REQ-SAG-0103 MUST); confirm with one live POST to the decider server before release (llama note j1-bead.txt:430-433 says it needs model omitted).

## Tests (exact code; all deterministic, loopback Stub, no_wait injected, no sleeps)
tests/support/mod.rs options(): add `max_sends: 5,` and `decidr: Default::default(),` after `local: Default::default(),` (no import).
A test builds a Route directly with `with_route` (in wires.rs below): model_with resolves (provider, name), then a Route literal over the Stub url, Cancel::default(), &no_wait, &keyring, and a local Cell for sends.
tests/wires.rs (new file, complete):
```rust
pub mod support;
use sc_agent::{
    AnthropicCompat, Cancel, DEFAULT_SYSTEM_PROMPT, Decision, Jev, Llm, LlmRequest, OpenAiCompat,
    Options, Route, model_with, result, send,
};
use serde_json::{Value, json};
use std::cell::Cell;
use support::*;

fn header<'a>(seen: &'a support::Request, name: &str) -> Option<&'a str> {
    seen.headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}
fn llm_request() -> LlmRequest {
    LlmRequest {
        system: Some("S".into()),
        messages: json!([{"role":"user","content":"hi"}]),
        schema: Some(json!({"type":"object"})),
    }
}
fn with_route<T>(
    options: &Options,
    (provider, name): (&str, &str),
    base: &str,
    run: impl Fn(&Route<'_>) -> T,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
    let (provider, model) = model_with(name, Some(provider), &Default::default())?;
    let sends = Cell::new(0);
    let route = Route {
        options,
        provider,
        model,
        base: base.into(),
        cancel: &Cancel::default(),
        wait: &no_wait,
        keyring: &keyring,
        sends: &sends,
    };
    Ok(run(&route))
}

#[test]
fn jev_decide_posts_the_request_with_the_resolved_model() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("typesafe", "jev"), &stub.url, |route| {
        Jev(route).decide(&request())
    })??;
    assert_eq!(reply, answer());
    let seen = stub.finish()?;
    let mut expected = request();
    expected["model"] = json!("jev-latest");
    assert_eq!(seen[0].path, "/v1/systemone");
    assert_eq!(seen[0].body, expected);
    Ok(())
}

#[test]
fn openai_compat_call_returns_the_json_the_model_wrote() -> Test {
    let root = Temp::new()?;
    let raw = json!({"choices":[{"message":{"content":"{\"a\":1}"}}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("openai", "luna"), &stub.url, |route| {
        OpenAiCompat(route).call(&llm_request())
    })??;
    assert_eq!(reply, json!({"a":1}));
    let seen = stub.finish()?;
    assert_eq!(seen[0].path, "/v1/chat/completions");
    assert_eq!(
        seen[0].body["messages"],
        json!([{"role":"system","content":"S"},{"role":"user","content":"hi"}])
    );
    assert_eq!(
        seen[0].body["response_format"]["json_schema"]["schema"],
        json!({"type":"object"})
    );
    assert_eq!(
        header(&seen[0], "authorization"),
        Some("Bearer test-secret")
    );
    Ok(())
}

#[test]
fn anthropic_compat_call_returns_the_json_the_model_wrote() -> Test {
    let root = Temp::new()?;
    let raw = json!({"content":[{"type":"text","text":"{\"a\":1}"}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("anthropic", "haiku"), &stub.url, |route| {
        AnthropicCompat(route).call(&llm_request())
    })??;
    assert_eq!(reply, json!({"a":1}));
    let seen = stub.finish()?;
    assert_eq!(seen[0].path, "/v1/messages");
    assert_eq!(seen[0].body["system"], "S");
    assert_eq!(
        seen[0].body["messages"],
        json!([{"role":"user","content":"hi"}])
    );
    assert_eq!(
        seen[0].body["output_config"]["format"]["schema"],
        json!({"type":"object"})
    );
    assert_eq!(header(&seen[0], "x-api-key"), Some("test-secret"));
    assert_eq!(header(&seen[0], "anthropic-version"), Some("2023-06-01"));
    assert_eq!(header(&seen[0], "authorization"), None);
    Ok(())
}

#[test]
fn luna_and_haiku_send_the_default_then_the_configured_system_prompt() -> Test {
    for (model, text) in [
        ("luna", "luna-system-prompt"),
        ("haiku", "haiku-system-prompt"),
    ] {
        for configured in [None, Some("X")] {
            let root = Temp::new()?;
            let logger = logger(&root)?;
            let content = answer().to_string();
            let raw = if model == "luna" {
                json!({"choices":[{"message":{"content":content}}]})
            } else {
                json!({"content":[{"type":"text","text":content}]})
            };
            let stub = Stub::new(vec![http(200, raw)])?;
            let mut opts = options(&root, &stub.url);
            opts.model = model.into();
            if let Some(prompt) = configured {
                opts.settings.decidr.insert(text.into(), prompt.into());
            }
            let value = send(
                &request(),
                &opts,
                &logger,
                &Cancel::default(),
                &no_wait,
                &keyring,
            )?;
            assert_eq!(value["answers"], answer()["answers"]);
            let body = &stub.finish()?[0].body;
            let system = if model == "luna" {
                body["messages"][0]["content"].clone()
            } else {
                body["system"].clone()
            };
            assert_eq!(system, json!(configured.unwrap_or(DEFAULT_SYSTEM_PROMPT)));
            logger.shutdown()?;
        }
    }
    Ok(())
}

#[test]
fn decide_over_an_llm_rejects_an_answer_of_the_wrong_type() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let wrong = json!({"answers":{"q":{"type":"choice","choice":"a","confidence":1.0,"probabilities":{"a":1.0}}}});
    let raw = json!({"choices":[{"message":{"content":wrong.to_string()}}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let mut opts = options(&root, &stub.url);
    opts.model = "luna".into();
    let failure = result(send(
        &request(),
        &opts,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    ));
    assert_eq!(failure["error"]["id"], "decision.schema_mismatch");
    let _ = stub.finish()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn max_sends_setting_limits_the_sends_of_one_request() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let busy = || http(502, json!({"error":"busy"}));
    let stub = Stub::new(vec![busy(), busy()])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 2;
    let failure = result(send(
        &request(),
        &opts,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    ));
    assert_eq!(failure["error"]["id"], "http.provider_busy");
    assert_eq!(failure["error"]["attempts"], 2);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    let _: Value = failure;
    Ok(())
}

#[test]
fn compat_call_reports_response_parse_for_text_that_is_not_json_or_missing() -> Test {
    for (reply, expected) in [
        (
            json!({"choices":[{"message":{"content":"not json"}}]}),
            "not json",
        ),
        (json!({"choices":[]}), "provider reply has no text"),
    ] {
        let root = Temp::new()?;
        let stub = Stub::new(vec![http(200, reply)])?;
        let opts = options(&root, &stub.url);
        let failure = with_route(&opts, ("openai", "luna"), &stub.url, |route| {
            OpenAiCompat(route).call(&llm_request())
        })?;
        let failure = result(failure);
        assert_eq!(failure["error"]["id"], "decision.response_parse");
        let message = failure["error"]["message"].as_str().ok_or("no message")?;
        assert!(message.contains(expected), "{message}");
        let _ = stub.finish()?;
    }
    Ok(())
}
```
config_and_values.rs addition:
```diff
diff --git a/crates/sc-agent/tests/config_and_values.rs b/crates/sc-agent/tests/config_and_values.rs
index b47296ea..eb907f4f 100644
--- a/crates/sc-agent/tests/config_and_values.rs
+++ b/crates/sc-agent/tests/config_and_values.rs
@@ -546,3 +546,46 @@ fn environment_json_and_strings_share_file_validation() -> Test {
     }
     Ok(())
 }
+
+#[test]
+fn max_sends_and_system_prompt_keys_load_default_and_report() -> Test {
+    let root = Temp::new()?;
+    let defaults = config(None, Some(&root.0), &[], None)?;
+    assert_eq!(defaults.config.max_sends, 5);
+    let row = |loaded: &sc_agent::Configuration, path: &str| {
+        loaded.rows.iter().find(|row| row["path"] == path).cloned()
+    };
+    assert_eq!(
+        row(&defaults, "max_sends"),
+        Some(json!({"path":"max_sends","value":"5","source":"default"}))
+    );
+    fs::write(
+        root.0.join("sc-agent.json"),
+        json!({"max_sends":3,"decidr":{"luna-system-prompt":"P"}}).to_string(),
+    )?;
+    let loaded = config(None, Some(&root.0), &[], None)?;
+    assert_eq!(loaded.config.max_sends, 3);
+    assert_eq!(loaded.config.decidr["luna-system-prompt"], "P");
+    assert_eq!(
+        row(&loaded, "decidr.luna-system-prompt").ok_or("row missing")?["value"],
+        "P"
+    );
+    fs::write(
+        root.0.join("sc-agent.json"),
+        json!({"max_sends":0}).to_string(),
+    )?;
+    let failure = config(None, Some(&root.0), &[], None)
+        .err()
+        .ok_or("zero accepted")?
+        .to_string();
+    assert!(
+        failure.contains("config.invalid") && failure.contains("max_sends"),
+        "{failure}"
+    );
+    fs::write(
+        root.0.join("sc-agent.json"),
+        json!({"decidr":{"sonnet-system-prompt":"P"}}).to_string(),
+    )?;
+    assert!(config(None, Some(&root.0), &[], None).is_err());
+    Ok(())
+}
```
B06 parse-failure test is compat_call_reports_response_parse_for_text_that_is_not_json_or_missing (passes; 7 wires tests, workspace suite and clippy clean).

Plan-QA-4 additions (all deterministic; stub-driven, no sleeps):
- wires.rs `compat_reply_missing_the_schema_is_schema_mismatch` (OpenAiCompat and AnthropicCompat Decision::decide return decision.schema_mismatch, J-1-06); `system_prompt_override_by_full_id` (haiku/luna chosen by full id uses decidr.<name>-system-prompt, J-1-12); `body_read_failure_is_resent` (a stub that drops the connection mid-body: http.network after max_sends, resent once first, J-1-14); `cancel_ends_a_wait_on_the_host_holdoff` (J-1-15).
- sending.rs `cancel_after_a_lower_layer_error_keeps_it` (http.auth stays http.auth, J-1-05).
- config_and_values.rs `built_in_rows_are_newest_first` (J-1-07).
- tests/contract/test_contract.py: decidr batch interrupted: output has one line per request, unfinished ones decision.cancelled (J-1-16).

Also: design "Units O4/O5" Done checks and Budget.
