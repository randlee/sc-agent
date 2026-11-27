# j-1 signatures v2 (spec; items and counts in j-1-signatures.json)

Built and measured in a throwaway worktree from origin/plan/phase-j 8f68424a; the full diff is scratchpad/j-1-proto-patches/v2/j1-v2-total.patch. The code below is the verbatim rustfmt output that built. Full gate (venv, so the wheel steps run): all 16 gates pass, Rust source 2342/2500. Baseline 2366.

## Numbers
| | lines |
|---|---|
| A. Restructure (Decision path, shared transport, tables, send/batch, dedup of consumers) | **-77** |
| B. New capability (Llm trait, LlmRequest, Llm impls, text extraction) | **+53** |
| Total vs 2366 | -24 (2342) |

Existence check: `llm query` and `--resume` do not exist at 8f68424a (git grep of crates and scripts finds only the unrelated sitr `query`). They are not in this spec; B counts only the Llm trait and its impls.

A is below the target of 0: the trait design replaces wire()/response() and the inline send loop and also absorbs the dedup below. Without the consumer dedup and the docli deletion (S26-S28, -83) A would be +6.

## Before/after (hardcoded construct -> trait replacement)
| Construct (file:line @8f68424a) | Lines | Replacement | Lines |
|---|---|---|---|
| wire() lib.rs:879-908 and response() 909-937 | 59 | Jev/Decision impls S17-S18 + ask S16 | 9+10+23 = 42 |
| per-wire body/reply for Llm (was inside wire/response) | (in the 59) | B04 B05 B06 (+B01..B03) | 53 (class B) |
| send() 957-1229 incl. loop, provider-name header choice, 2 hand-built Failure literals | 273 | send S20 + post S19 | 113 + 114 = 227 |
| six inline cancel checks (977,1042,1047,1078,1174,1184) | 18 | Cancel::check S21 + 6 call sites | 6 + 6 |
| cancel-polling recv loop lib.rs:1077-1087 + decidr main.rs:78-93 | 11 + 16 | Cancel::recv S22 | 9 |
| repo+config copies: doctor 1326-1337, decidr 41-49, py 158-171 | 12+9+14 | load() S24 + doctor S25 | 14 + 4 |
| provider-name list lib.rs:719 | 1 | Provider.open S07-S09 | 1 + 1 + 7 |
| CliModelError and dead accessors, docli_from_clap.rs:13-34, 88-114 | 54 | deleted S28 | 0 |
Config additions required by records (not replacements): Settings +2, KEYS +3, init +2, check +3, imports +5.

## Per-item attribution of A (sums to -77)
S01 +5, S02 +2, S03 +3, S05 +2, S06 +3, S07 +1, S08 +7, S10 +2, S11 +3, S12 +1, S13 +2, S14 +10, S15 +3, S16 +23, S17 +9, S18 +10, S19 +114, S20 -157, S21 +6, S22 +9, S23 +1, S24 +14, S25 -8, S26 -20, S27 -9, S28 -54, S30 -59. B: B01 +5, B02 +3, B03 +1, B04 +19, B05 +10, B06 +15 = +53. S20 -157 includes 3 blank-line lines at span boundaries (measured hunk total +59 for lib.rs).

## Savings found beyond the trait replacement (all counted, tests pass)
| Item | Where @8f68424a | Saving | Allowed by | Behaviour test |
|---|---|---|---|---|
| docli CliModelError + test-only accessors | scripts/docli_from_clap.rs:13-34, 88-114 | 54 | dead code (allow(dead_code) pragma, only its own test used it); no record names it | the file's own test, run by both examples |
| hand-built Failure literals -> error() | lib.rs:1105-1118, 1141-1165 | in S19/S20 | REQ-SAG-0016/0017 give category by id; action text is not pinned (grep) | sending.rs status/quota/auth tests |
| repo+config copy x3 -> load() | decidr main.rs:41-49, py lib.rs:158-171, lib.rs:1326-1337 | 20 net | ANC-010 | contract doctor/CLI tests |
| cancel checks/recv | lib.rs 6 sites + decidr 4 sites | 14 net | ANC-010 | cancel tests |
Not done (considered): provider-row compaction by #[rustfmt::skip] (saves 66, suppression pragma, not used); py send/send_batch shared prelude (PyO3 signatures force the lists); emit/finish helper for decidr/sitr main (about -6).

## Behaviour changes the owner should know (none is pinned by a test or record I could find)
1. attempts is set on every failure after at least one send (and on cancel); before only on http failures and cancel. REQ-SAG-0016 says "when they apply".
2. decision.api_status and quota actions now use error()'s generic text; api_status category stays 'other'.
3. Anthropic reply text is read at /content/0/text (before: first block with text).
4. decidr's cancelled message is "request cancelled" (was "interrupted").
5. schema() runs in send(), so Decision::decide returns the unvalidated reply.
6. Network failures share the busy path but still take no host lock.
7. The compat Decision path now sends the prompt as a system message plus the bare request JSON as the user message (ANC-014); before it sent one user message "Answer every question in this decision request using its required schema, including probabilities and confidence: <request>" (old wire(), lib.rs:904).
8. Provider.open appears in decidr's providers JSON (json!(PROVIDERS), decidr main.rs:36).
9. doctor gains rows for max_sends and the two decidr.*-system-prompt keys (KEYS).

## Optional units
Measured on the old structure: O1 +37, O2 +2, O3 +42, O4 +19, O5 +32 (+132). NOT re-measured on this base (they edit send and the loop that moved). Projection from 2342: O1 2379, +O2 2381, +O3 2423, +O4 2442, +O5 2474; all five fit with 26 spare if the deltas hold. Treat as unverified; re-measure when j-1 is built.

## Gap closures
base_url: Route.base = options.base_url.clone().unwrap_or(base) at construction (in send). Key function is key(), not resolve(). Loop form: `for attempt in 1..=max`. Prompt key: format!("{}-system-prompt", options.model) as typed; a full id finds no override. Schema wrapper, user text, response_format name and the Anthropic header table are in the code below. Tests need no BTreeMap import (Default::default()). Route is item S14. Gate change applied (S29). Each function's max lines = its counted lines in the JSON (exact text, no latitude except rustfmt).

## Exact code
### Aliases
```rust
pub type DecisionRequest = Value;
pub type DecisionResult = Value;
```

### LlmRequest (B01)
```rust
#[derive(Debug)]
pub struct LlmRequest {
    pub system: Option<String>,
    pub messages: Value,
    pub schema: Option<Value>,
}
```

### Decision (S11)
```rust
pub trait Decision {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error>;
}
```

### Llm (B02)
```rust
pub trait Llm {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error>;
}
```

### Default prompt (S12)
```rust
pub const DEFAULT_SYSTEM_PROMPT: &str = "Answer every question in the decision request. Reply with one JSON object that matches the supplied schema, including probabilities and confidence.";
```

### Auth consts (S13) and ANTHROPIC (B03)
```rust
type Auth = &'static [(&'static str, &'static str)];
const BEARER: Auth = &[("Authorization", "Bearer {}")];
```

### ANTHROPIC
```rust
const ANTHROPIC: Auth = &[("x-api-key", "{}"), ("anthropic-version", "2023-06-01")];
```

### Route (S14)
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

### Wire types (S15)
```rust
pub struct Jev<'a>(pub &'a Route<'a>);
pub struct OpenAiCompat<'a>(pub &'a Route<'a>);
pub struct AnthropicCompat<'a>(pub &'a Route<'a>);
```

### ask (S16)
```rust
fn ask(llm: &impl Llm, route: &Route<'_>, req: &DecisionRequest) -> Result<DecisionResult, Error> {
    let prompt = route
        .options
        .settings
        .decidr
        .get(&format!("{}-system-prompt", route.options.model));
    let questions = req
        .get("questions")
        .and_then(Value::as_object)
        .into_iter()
        .flatten();
    let answers = questions
        .map(|(id, question)| (id, answer_schema(question)))
        .collect::<BTreeMap<_, _>>();
    llm.call(&LlmRequest {
        system: Some(prompt.map_or(DEFAULT_SYSTEM_PROMPT, String::as_str).into()),
        messages: json!([{"role":"user","content":req.to_string()}]),
        schema: Some(json!({
            "type":"object", "required":["answers"], "additionalProperties":false,
            "properties":{"answers":{"type":"object","properties":answers,"required":answers.keys().collect::<Vec<_>>(),"additionalProperties":false}}
        })),
    })
}
```

### Jev (S17)
```rust
impl Decision for Jev<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        let mut body = req.clone();
        if let Some(body) = body.as_object_mut() {
            body.insert("model".into(), json!(self.0.model));
        }
        self.0.post("systemone", BEARER, &body)
    }
}
```

### Decision for OpenAiCompat (S18)
```rust
impl Decision for OpenAiCompat<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        ask(self, self.0, req)
    }
}
```

### Decision for AnthropicCompat (S18)
```rust
impl Decision for AnthropicCompat<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        ask(self, self.0, req)
    }
}
```

### Llm for OpenAiCompat (B04)
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

### Llm for AnthropicCompat (B05)
```rust
impl Llm for AnthropicCompat<'_> {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error> {
        let format = req
            .schema
            .as_ref()
            .map(|schema| json!({"format":{"type":"json_schema","schema":schema}}));
        let body = json!({"model":self.0.model,"max_tokens":8192,"messages":req.messages,"system":req.system,"output_config":format});
        self.0.text("messages", ANTHROPIC, body, "/content/0/text")
    }
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
            // A busy reply holds off senders to this host through its injected wait.
            drop(pause.lock().map_err(|e| error("http.network", e))?);
            cancel.check()?;
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
            let (status, delay, text) = match cancel.recv(&receiver, "http.network")? {
                Ok((status, delay, text)) => (
                    status,
                    delay,
                    redact(text.map_err(|e| error("decision.response_parse", e))?),
                ),
                Err(e) => (0, None, redact(e.to_string())),
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
                let _hold = (status != 0)
                    .then(|| pause.lock())
                    .transpose()
                    .map_err(|e| error("http.network", e))?;
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
        let (provider, model) = model_with(
            &options.model,
            options.provider.as_deref(),
            &options.settings.models,
        )?;
        let (base, model) = if provider.url.is_empty() {
            let runner = options
                .settings
                .laya
                .as_ref()
                .ok_or_else(|| missing_setting("laya.host"))?;
            (
                format!("http://{}:{}/v1", runner.host, runner.port),
                runner.model.clone(),
            )
        } else {
            (provider.url.into(), model)
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
        let reply = match provider.wire {
            "messages" => AnthropicCompat(&route).decide(request),
            "chat/completions" => OpenAiCompat(&route).decide(request),
            _ => Jev(&route).decide(request),
        }?;
        schema(request, &reply).map(|()| reply)
    })();
    if let Err(e) = cancel.check() {
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

### Cancel::check (S21)
```rust
    pub fn check(&self) -> Result<(), Error> {
        match self.is_cancelled() {
            true => Err(error("decision.cancelled", "request cancelled")),
            false => Ok(()),
        }
    }
```

### Cancel::recv (S22)
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

### load (S24)
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

### doctor (S25)
```rust
pub fn doctor(config_dir: Option<&Path>, project: Option<&str>) -> Result<Value, Error> {
    let (loaded, ..) = load(config_dir, project)?;
    Ok(json!({"settings":loaded.rows, "files":loaded.files, "log_root":log_root("decidr")?}))
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
+                cancelled.check()?;
                 result
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

## Tests (exact code; all deterministic, loopback Stub, no_wait injected, no sleeps)
tests/support/mod.rs options(): add `max_sends: 5,` and `decidr: Default::default(),` after `laya: None,` (no import).
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

## Unresolved owner decisions
1. The behaviour changes above (1-6). 2. Key name max_sends. 3. Prompt key by typed name. 4. Wire-string dispatch in send(). 5. Whether to keep the docli dead-code deletion (outside sc-agent). 6. Cache hash definition (O3) and Run vs ADR-0001 (O1). 7. ANC-009 CLI half is j-2.

## Not independently verified
All counts and tests are my own runs. The Python contract suite passed only in my venv run of the gate. Optional-unit totals are projections. The final gate run predates the added B06 test (that test and clippy were run alone).

## Units O4/O5 (owner rulings 2026-11-24; placement: lead decision)
Supersedes the "Optional units" measurements for O4 (+19) and O5 (+32): those were taken on the old structure and are invalid. Both are new capability, counted separately from the -77 restructure. Code lands on sprint/j-1 @b4be55e6 and is updated in place. Items are in scratchpad/phase-j/j-1-o45-beads.json (one per public-surface item).

Goal (Rand's words): O4 "restore decider (Strands Decider) as a local model exactly like laya ... --model decider" (REQ-SAG-0801: decider resolves to the 4b row; each runner's model has a fixed id in the table; address from configuration). O5 "haiku/luna required for all tools with no TYPESAFE_API_KEY ... send works keyless for haiku/luna via provider keys" (REQ-SAG-0103: prefer a keyless provider; else the first whose key is found). Measure: the named tests below pass; no provider or model name is compared outside PROVIDERS.

Layout: crates/sc-agent/src/lib.rs (all of it; providers.rs is j-1's v1 plan, not this branch's); scripts/integration.py and tests/contract/test_integration.py (export); crates/sc-agent/tests/{config_and_values,sending,records,wires,support}.rs; tests/contract/test_contract.py and crates/sc-agent-py/tests/loopback.py (laya fixtures lose "model").

### Public surface (exact Rust; replaces; deletes)
O4-1 `pub struct Runner { pub host: String, pub port: u16 }`. Replaces Runner lib.rs:121-125. Deletes field `model` and key laya.model (the model id now lives in the table row).
O4-2 `pub struct Settings { ..., pub local: BTreeMap<String, Runner>, ... }` keyed by provider name, one entry per local provider configured. Replaces `laya: Option<Runner>` lib.rs:139 and the laya block in config() lib.rs:281-289. Deletes both.
O4-3 Three local rows in PROVIDERS, `Provider { name, url: "", key: "", wire: "systemone", models, open: false }` with models `("laya","laya:en","2026-09-23")`, `("decider","decider-4b-v2.1","2026-09-24")` (provider decider_4b), `("decider","decider-2b-v11","2026-09-24")` (provider decider_2b). Replaces the single "local" row lib.rs:710-718. Deletes provider name "local" (the --provider local spelling; "local" was never a record id).
O4-4 `const KEYS` gains decider_4b.host, decider_4b.port, decider_2b.host, decider_2b.port and loses laya.model. Replaces lib.rs:171-181. Deletes laya.model.
O4-5 `fn setting<T>(resolved: &Resolved, path: &str, default: Value)` : the missing-local-setting rule is `PROVIDERS.iter().any(|p| p.url.is_empty() && p.name == head)` for the first path segment. Replaces `path.starts_with("laya.")` lib.rs:160 (and `path: &'static str`).
O4-6 send(): a provider with url "" takes its base from `options.settings.local[provider.name]`; the request carries the resolved id. An unset runner is config.missing_setting naming `<provider.name>.host`. Replaces lib.rs:1199-1210 (hard-coded "laya.host", runner.model). Deletes the runner.model override.
O5-1 `fn candidates(name: &str, provider: Option<&str>, configured: &BTreeMap<String, ModelEntry>) -> Result<Vec<(&'static Provider, String)>, Error>`. A configured entry for the name (provider unset or equal) is the only candidate. Otherwise every table row whose short name or id equals the name, newest date first, equal dates in PROVIDERS order; an explicit provider limits the rows to it and, if it is open and has no row, is passthrough. No rows: types.unknown_short_name. Replaces the body of model_with lib.rs:732-765.
O5-2 `pub fn model_with(name, provider, configured) -> Result<(&'static Provider, String), Error>` and `pub fn model(name, provider)` keep their signatures: `candidates(..)?` first element. Replaces nothing (bodies shrink). Deletes the per-provider max_by_key closure.
O5-3 `fn resolve(found: Vec<(&'static Provider, String)>, options: &Options, keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync)) -> Result<(&'static Provider, String), Error>`: one candidate is returned unchanged (post() reports its key errors, so existing error order and keyring read counts are unchanged); otherwise the first keyless candidate, else the first whose key() succeeds, else the last key() error other than secrets.key_missing, else secrets.key_missing naming every candidate variable. Replaces the direct model_with call lib.rs:1194.
O5-4 Nous rows `("haiku","anthropic/claude-haiku-5.5","2026-10-07")` and `("luna","openai/gpt-6-luna","2026-09-22")` (hendrix 2026-11-24, same ids as openrouter). Replaces `models: &[]` on the nous provider lib.rs:703. Nous stays open (passthrough kept).
O5-5 scripts/integration.py --export-env: every keyed provider variable from the Rust table is looked up and exported when held; at least one of TYPESAFE_API_KEY, ANTHROPIC_API_KEY, NOUS_API_KEY is required. Replaces the fixed ("typesafe","anthropic","openai") list and the TypeSafe-required check integration.py:35-39. Deletes both.

No new type, no new pub item beyond O4-1/O4-2. Redaction already covers the key of whichever provider is used (post(): `redact` over `provider.key`), so a nous/anthropic/openai key is redacted; the test below pins it.

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
Gate: `cargo test --workspace --all-features` (1.98.1), `cargo fmt --check`, `cargo clippy ... -D warnings`, `pytest tests/contract`, `python3 scripts/gate/rewrite.py` (Rust source lines N/2500; every push).

### Budget (against 2,500; j-1 at 2342; new capability, restructure unchanged at -77)
Estimated before building: O4 +19, O5 +35 (+54, 2396). Measured on sprint/j-1 after the build, rewrite.py: Rust source 2404/2500 (+62 for O4+O5 together; the 8 over the estimate is the explicit single-candidate shortcut and the key_missing name list in resolve). 96 spare. scripts/integration.py and the Python tests are not counted by the gate. Not in these two units (still j-1 bead text, not these rulings): decision.model_mismatch for a local reply whose model differs from the resolved id.
