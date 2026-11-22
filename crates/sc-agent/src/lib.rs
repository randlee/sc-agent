use sc_config::layers::{self, EnvValue, FileLayer, Inputs, Key, Layered};
use sc_observability::{LoggerConfig, ServiceName, v2::Logger};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, atomic::Ordering},
    time::Duration,
};

#[derive(Default)]
pub struct Cancel {
    state: Mutex<bool>,
    changed: Condvar,
}
impl Cancel {
    pub fn cancel(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = true;
        }
        self.changed.notify_all();
    }
    pub fn is_cancelled(&self) -> bool {
        self.state.lock().map_or(true, |state| *state)
    }
    pub fn wait(&self, delay: Duration) {
        if let Ok(state) = self.state.lock() {
            let _ = self
                .changed
                .wait_timeout_while(state, delay, |state| !*state);
        }
    }
}

#[derive(Debug, Serialize, thiserror::Error)]
#[serde(untagged)]
#[error("{id}: {message}")]
pub enum Error {
    Failure {
        id: &'static str,
        message: String,
        action: &'static str,
        category: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        attempts: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
}
pub fn error(id: &'static str, message: impl ToString) -> Error {
    let category = match id {
        "secrets.key_missing"
        | "secrets.key_malformed"
        | "secrets.keyring_read"
        | "secrets.keyring_timeout"
        | "http.auth" => "auth",
        "http.network" => "network",
        "http.provider_busy" => "busy",
        "http.quota_exhausted" => "quota",
        "decision.cancelled" => "cancelled",
        "decision.response_parse" | "decision.schema_mismatch" => "response_invalid",
        s if s.starts_with("config.") => "config",
        _ => "validation",
    };
    Error::Failure {
        id,
        message: message.to_string(),
        category,
        action: match category {
            "auth" => "Check the provider API key.",
            "busy" | "network" => "Retry later.",
            _ => "Fix the named input or provider response.",
        },
        attempts: None,
        session_id: None,
    }
}
pub fn result(value: Result<Value, Error>) -> Value {
    match value {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Runner {
    pub host: String,
    pub port: u16,
    pub model: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default = "default_workers")]
    pub workers: usize,
    #[serde(default)]
    pub log_root: Option<PathBuf>,
    #[serde(default)]
    pub laya: Option<Runner>,
}
fn default_workers() -> usize {
    8
}
impl Settings {
    pub fn project_required(&self) -> Result<&str, Error> {
        self.project.as_deref().filter(|s| !s.is_empty()).ok_or_else(||error("config.missing_project", "Set project/phase through --project, SC_PROJECT, local decidr.json, local sc-agent.json, global decidr.json, or global sc-agent.json"))
    }
}
pub const KEYS: &[Key] = &[
    Key {
        path: "log_root",
        env: EnvValue::String,
    },
    Key {
        path: "project",
        env: EnvValue::String,
    },
    Key {
        path: "workers",
        env: EnvValue::Json,
    },
    Key {
        path: "laya.host",
        env: EnvValue::String,
    },
    Key {
        path: "laya.port",
        env: EnvValue::Json,
    },
    Key {
        path: "laya.model",
        env: EnvValue::String,
    },
];
pub fn config(
    local: Option<&Path>,
    global: &Path,
    environment: &[(OsString, OsString)],
    project: Option<&str>,
) -> Result<Layered<Settings>, Error> {
    let files = local
        .into_iter()
        .chain(std::iter::once(global))
        .flat_map(|folder| {
            ["decidr.json", "sc-agent.json"].map(|name| FileLayer {
                path: folder.join(name),
                accepts: KEYS,
            })
        })
        .collect();
    let env_keys = KEYS
        .iter()
        .copied()
        .filter(|key| key.path != "project")
        .collect::<Vec<_>>();
    let flags = project
        .map(|s| ("project".into(), json!(s)))
        .into_iter()
        .collect();
    layers::load(
        &Inputs {
            files,
            env_prefix: "DECIDR_",
            env_aliases: &[("SC_PROJECT", "project")],
            env_accepts: &env_keys,
            flags,
            flag_names: BTreeMap::new(),
            keys: KEYS,
            environment,
        },
        |value| serde_json::from_value::<Settings>(value).map_err(|e| error("config.invalid", e)),
        |_| None,
    )
    .map_err(|e| error("config.invalid", e))
}
pub fn slug(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(error(
            "log.invalid_slug",
            "project and agent_role must be lowercase slugs",
        ));
    }
    Ok(())
}
pub fn log_root(name: &str) -> Result<PathBuf, Error> {
    std::env::var_os(format!(
        "{}_LOG_ROOT",
        name.to_uppercase().replace('-', "_")
    ))
    .filter(|s| !s.is_empty())
    .map(PathBuf::from)
    .map(Ok)
    .unwrap_or_else(|| {
        std::env::home_dir()
            .map(|p| p.join(".sc-agent"))
            .ok_or_else(|| error("config.home", "Home directory unavailable"))
    })
}
pub fn default_logger(name: &str) -> Result<Logger, Error> {
    slug(name)?;
    let mut config = LoggerConfig::default_for(
        ServiceName::new(name).map_err(|e| error("config.logger", e))?,
        log_root(name)?,
    );
    config.enable_console_sink = false;
    Logger::new(config).map_err(|e| error("config.logger", e))
}
pub fn key(
    variable: &str,
    environment: &[(OsString, OsString)],
    root: &Path,
    keyring: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, Error> {
    let env = environment
        .iter()
        .find(|(name, _)| name == variable)
        .map(|(_, v)| v.to_string_lossy().into_owned());
    let dotenv = || -> Result<Option<String>, Error> {
        let path = root.join(".env");
        if !path.exists() {
            return Ok(None);
        }
        for entry in dotenvy::from_path_iter(path).map_err(|e| error("config.dotenv", e))? {
            let (name, value) = entry.map_err(|e| error("config.dotenv", e))?;
            if name == variable {
                return Ok(Some(value));
            }
        }
        Ok(None)
    };
    let value = match env {
        Some(value) => value,
        None => match dotenv()? {
            Some(value) => value,
            None => keyring(variable).map_err(|e| error("secrets.keyring_read", e))?,
        },
    };
    if value.is_empty() {
        return Err(error(
            "secrets.key_missing",
            format!("Set {variable}, repo-root .env, or the OS keyring"),
        ));
    }
    if !value.is_ascii() || value.bytes().any(|b| b <= 32 || b == 127) {
        return Err(error(
            "secrets.key_malformed",
            format!("{variable} contains whitespace, controls or non-ASCII characters"),
        ));
    }
    Ok(value)
}
pub fn os_keyring(variable: &str) -> Result<String, String> {
    let variable = variable.to_owned();
    let account = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .map_err(|e| e.to_string())?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let value = keyring::Entry::new(&variable, &account)
            .and_then(|entry| entry.get_password())
            .map_err(|e| e.to_string());
        let _ = sender.send(value);
    });
    receiver
        .recv_timeout(Duration::from_secs(15))
        .map_err(|e| e.to_string())?
}
#[derive(Debug, Serialize)]
pub struct Provider {
    pub name: &'static str,
    pub url: &'static str,
    pub key: &'static str,
    pub wire: &'static str,
    pub models: &'static [(&'static str, &'static str, &'static str)],
}
const CLAUDE: &[(&str, &str, &str)] = &[
    ("haiku", "claude-haiku-5-5", "2026-10-07"),
    ("sonnet", "claude-sonnet-5-5", "2026-09-28"),
    ("opus", "claude-opus-5-5", "2026-09-22"),
    ("fable", "claude-fable-5-2", "2026-11-09"),
];
const GPT: &[(&str, &str, &str)] = &[
    ("luna", "gpt-6-luna", "2026-09-22"),
    ("terra", "gpt-6.1-terra", "2026-10-26"),
    ("sol", "gpt-6.1-sol", "2026-09-29"),
    ("astra", "gpt-6-astra", "2026-09-03"),
];
pub static PROVIDERS: &[Provider] = &[
    Provider {
        name: "typesafe",
        url: "https://api.typesafe.ai/v1",
        key: "TYPESAFE_API_KEY",
        wire: "systemone",
        models: &[("jev", "jev-1.14.0", "2026-10-14")],
    },
    Provider {
        name: "anthropic",
        url: "https://api.anthropic.com/v1",
        key: "ANTHROPIC_API_KEY",
        wire: "messages",
        models: CLAUDE,
    },
    Provider {
        name: "openai",
        url: "https://api.openai.com/v1",
        key: "OPENAI_API_KEY",
        wire: "chat/completions",
        models: GPT,
    },
    Provider {
        name: "openrouter",
        url: "https://openrouter.ai/api/v1",
        key: "OPENROUTER_API_KEY",
        wire: "chat/completions",
        models: &[
            ("haiku", "anthropic/claude-haiku-5.5", "2026-10-07"),
            ("sonnet", "anthropic/claude-sonnet-5.5", "2026-09-28"),
            ("opus", "anthropic/claude-opus-5.5", "2026-09-22"),
            ("fable", "anthropic/claude-fable-5.2", "2026-11-09"),
            ("luna", "openai/gpt-6-luna", "2026-09-22"),
            ("terra", "openai/gpt-6.1-terra", "2026-10-26"),
            ("sol", "openai/gpt-6.1-sol", "2026-09-29"),
            ("astra", "openai/gpt-6-astra", "2026-09-03"),
        ],
    },
    Provider {
        name: "together",
        url: "https://api.together.ai/v1",
        key: "TOGETHER_API_KEY",
        wire: "chat/completions",
        models: &[
            ("haiku", "anthropic/claude-haiku-5-5", "2026-10-07"),
            ("sonnet", "anthropic/claude-sonnet-5-5", "2026-09-28"),
            ("opus", "anthropic/claude-opus-5-5", "2026-09-22"),
            ("fable", "anthropic/claude-fable-5-2", "2026-11-09"),
            ("luna", "openai/gpt-6-luna", "2026-09-22"),
            ("terra", "openai/gpt-6.1-terra", "2026-10-26"),
            ("sol", "openai/gpt-6.1-sol", "2026-09-29"),
            ("astra", "openai/gpt-6-astra", "2026-09-03"),
        ],
    },
    Provider {
        name: "nous",
        url: "https://inference-api.nousresearch.com/v1",
        key: "NOUS_API_KEY",
        wire: "chat/completions",
        models: &[],
    },
    Provider {
        name: "local",
        url: "",
        key: "",
        wire: "systemone",
        models: &[("laya", "laya", "")],
    },
];
pub fn model(name: &str, provider: Option<&str>) -> Result<(&'static Provider, String), Error> {
    let rows = |p: &Provider| {
        p.models
            .iter()
            .filter(|(short, id, _)| *short == name || *id == name)
            .max_by_key(|(_, _, date)| *date)
            .map(|(_, id, _)| (*id).to_owned())
    };
    if let Some(provider) = provider {
        let p = PROVIDERS
            .iter()
            .find(|p| p.name == provider)
            .ok_or_else(|| error("types.unknown_provider", provider))?;
        if let Some(id) = rows(p) {
            return Ok((p, id));
        }
        if ["openrouter", "together", "nous"].contains(&provider) {
            return Ok((p, name.into()));
        }
    } else if let Some(found) = PROVIDERS.iter().find_map(|p| rows(p).map(|id| (p, id))) {
        return Ok(found);
    }
    Err(error("types.unknown_short_name", name))
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Bounds {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Choice {
        choice: String,
        confidence: f64,
        probabilities: BTreeMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
    },
    Noul {
        noul: f64,
    },
    Box {
        r#box: Option<Bounds>,
        confidence: Option<f64>,
    },
}
pub fn schema(request: &Value, response: &Value) -> Result<(), Error> {
    let questions = request
        .get("questions")
        .and_then(Value::as_object)
        .ok_or_else(|| error("decision.request_parse", "questions must be an object"))?;
    let answers = response
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| error("decision.schema_mismatch", "answers must be an object"))?;
    if questions.len() != answers.len() {
        return Err(error(
            "decision.schema_mismatch",
            "answer ids do not match question ids",
        ));
    }
    for (id, question) in questions {
        let answer = answers
            .get(id)
            .ok_or_else(|| error("decision.schema_mismatch", format!("missing answer {id}")))?;
        let _: Answer = serde_json::from_value(answer.clone())
            .map_err(|e| error("decision.schema_mismatch", format!("{id}: {e}")))?;
        if answer.get("type") != question.get("type") {
            return Err(error(
                "decision.schema_mismatch",
                format!("{id}: wrong question type"),
            ));
        }
    }
    Ok(())
}
pub fn digest(request: &Value, question: &str) -> Result<String, Error> {
    let mut input = request
        .as_object()
        .cloned()
        .ok_or_else(|| error("decision.request_parse", "request must be an object"))?;
    let selected = input
        .remove("questions")
        .and_then(|v| v.get(question).cloned())
        .ok_or_else(|| error("decision.request_parse", "question id missing"))?;
    input.insert("questions".into(), json!({question:selected}));
    let text = serde_json::to_vec(&input).map_err(|e| error("decision.encode", e))?;
    sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
        utf8_file_bytes: &text,
    })
    .map(|h| h.template().to_string())
    .map_err(|e| error("decision.encode", e))
}

pub struct Options {
    pub environment: Vec<(OsString, OsString)>,
    pub settings: Settings,
    pub model: String,
    pub provider: Option<String>,
    pub agent_role: String,
    pub timeout: Duration,
    pub base_url: Option<String>,
    pub root: PathBuf,
    pub log_root: PathBuf,
    pub repo: String,
}
pub fn log(logger: &Logger, action: &str, fields: Value) -> Result<(), Error> {
    let event = serde_json::from_value(
        json!({"version":"v1","timestamp":sc_observability::Timestamp::now_utc(),"level":if action=="error" {sc_observability::Level::Error}else{sc_observability::Level::Info},"service":logger.service_name(),"target":"sc-agent","action":action,"identity":sc_observability::ProcessIdentity::default(),"fields":fields}),
    ).map_err(|e| error("log.invalid_event", e))?;
    logger.log(event).map_err(|e| error("log.write", e))
}

fn answer_schema(question: &Value) -> Value {
    let kind = question.get("type").and_then(Value::as_str).unwrap_or("");
    let names = if kind == "choice" {
        question
            .get("criteria")
            .and_then(Value::as_object)
            .map(|v| v.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    } else {
        question
            .get("criteria")
            .and_then(Value::as_array)
            .map(|v| (0..v.len()).map(|i| i.to_string()).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let object = |value_type| {
        json!({
            "type":"object",
            "properties":names.iter().map(|name|(name.clone(),json!({"type":value_type}))).collect::<BTreeMap<_,_>>(),
            "required":names,
            "additionalProperties":false
        })
    };
    let mut fields = json!({"type":{"type":"string","enum":[kind]}});
    if let Some(fields) = fields.as_object_mut() {
        match kind {
            "choice" => {
                fields.insert("choice".into(), json!({"type":"string","enum":names}));
                fields.insert("confidence".into(), json!({"type":"number"}));
                fields.insert("probabilities".into(), object("number"));
            }
            "score" => {
                fields.insert("score".into(), json!({"type":"number"}));
                fields.insert("confidence".into(), json!({"type":"number"}));
                fields.insert("legend".into(), object("string"));
                fields.insert("probabilities".into(), object("number"));
            }
            "box" => {
                fields.insert("box".into(),json!({
                    "type":["object","null"],
                    "properties":(["top","bottom","left","right"].into_iter().map(|name|(name,json!({"type":"number"}))).collect::<BTreeMap<_,_>>()),
                    "required":["top","bottom","left","right"],"additionalProperties":false
                }));
                fields.insert("confidence".into(), json!({"type":["number","null"]}));
            }
            _ => {
                fields.insert("noul".into(), json!({"type":"number"}));
            }
        }
    }
    let required = fields
        .as_object()
        .map(|v| v.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
fn wire(request: &Value, model: &str, wire: &str) -> Value {
    if wire == "systemone" {
        let mut body = request.clone();
        if let Some(body) = body.as_object_mut() {
            body.insert("model".into(), json!(model));
        }
        return body;
    }
    let properties = request
        .get("questions")
        .and_then(Value::as_object)
        .map(|questions| {
            questions
                .iter()
                .map(|(id, question)| (id.clone(), answer_schema(question)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let required = properties.keys().cloned().collect::<Vec<_>>();
    let schema = json!({
        "type":"object", "required":["answers"], "additionalProperties":false,
        "properties":{"answers":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}
    });
    let messages = json!([{"role":"user","content":format!("Answer every question in this decision request using its required schema, including probabilities and confidence: {request}")}]);
    if wire == "messages" {
        json!({"model":model,"max_tokens":8192,"messages":messages,"output_config":{"format":{"type":"json_schema","schema":schema}}})
    } else {
        json!({"model":model,"messages":messages,"response_format":{"type":"json_schema","json_schema":{"name":"decision","strict":true,"schema":schema}}})
    }
}
fn response(request: &Value, raw: Value, wire: &str) -> Result<Value, Error> {
    let answer = match wire {
        "messages" => raw
            .get("content")
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find_map(|v| v.get("text").and_then(Value::as_str))
            }),
        "chat/completions" => raw
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str),
        _ => {
            schema(request, &raw)?;
            return Ok(raw);
        }
    }
    .ok_or_else(|| {
        error(
            "decision.response_parse",
            format!("provider reply has no text: {raw}"),
        )
    })?;
    let parsed: Value = serde_json::from_str(answer)
        .map_err(|e| error("decision.response_parse", format!("{e}: {answer}")))?;
    schema(request, &parsed)?;
    Ok(parsed)
}
static HOLDOFFS: Mutex<BTreeMap<String, Arc<Mutex<()>>>> = Mutex::new(BTreeMap::new());
static SESSION_APPEND: Mutex<()> = Mutex::new(());

pub fn host_holdoff(host: &str) -> Result<Arc<Mutex<()>>, Error> {
    Ok(HOLDOFFS
        .lock()
        .map_err(|e| error("http.network", e))?
        .entry(host.to_owned())
        .or_default()
        .clone())
}

pub fn validate_request_size(request: &Value) -> Result<(), Error> {
    if serde_json::to_vec(request)
        .map_err(|e| error("decision.encode", e))?
        .len()
        > 8 * 1024 * 1024
    {
        return Err(error(
            "decision.request_too_large",
            "request exceeds the 8 MiB serialized input limit",
        ));
    }
    Ok(())
}
pub fn workers(value: i64) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| error("decidr.invalid_workers", "workers must be positive"))
}

pub fn send(
    request: &Value,
    options: &Options,
    logger: &Logger,
    cancel: &Cancel,
    wait: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, String> + Sync),
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
    let mut sends = 0;
    let mut outcome = (|| {
        validate_request_size(request)?;
        if cancel.is_cancelled() {
            return Err(error("decision.cancelled", "request cancelled"));
        }
        let project = options.settings.project_required()?;
        slug(project)?;
        slug(&options.agent_role)?;
        let (provider, model) = model(&options.model, options.provider.as_deref())?;
        let (base, model) = if provider.name == "local" {
            let runner = options
                .settings
                .laya
                .as_ref()
                .ok_or_else(|| error("config.runner", "Set laya host, port and model"))?;
            (
                format!("http://{}:{}/v1", runner.host, runner.port),
                runner.model.clone(),
            )
        } else {
            (provider.url.into(), model)
        };
        let url = format!(
            "{}/{}",
            options
                .base_url
                .as_deref()
                .unwrap_or(&base)
                .trim_end_matches('/'),
            provider.wire
        );
        let host = url
            .parse::<ureq::http::Uri>()
            .map_err(|e| error("http.network", e))?
            .host()
            .ok_or_else(|| error("http.network", "endpoint has no host"))?
            .to_ascii_lowercase();
        let pause = host_holdoff(&host)?;
        let key = if provider.key.is_empty() {
            String::new()
        } else {
            key(provider.key, &options.environment, &options.root, keyring)?
        };
        let body = wire(request, &model, provider.wire);
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(options.timeout))
            .build()
            .new_agent();
        for attempt in 1..=5 {
            if cancel.is_cancelled() {
                return Err(error("decision.cancelled", "request cancelled"));
            }
            // A busy reply holds off senders to this host through its injected wait.
            drop(pause.lock().map_err(|e| error("http.network", e))?);
            if cancel.is_cancelled() {
                return Err(error("decision.cancelled", "request cancelled"));
            }
            let mut post = agent.post(&url).header("Content-Type", "application/json");
            if provider.name == "anthropic" {
                post = post
                    .header("x-api-key", &key)
                    .header("anthropic-version", "2023-06-01");
            } else if !key.is_empty() {
                post = post.header("Authorization", format!("Bearer {key}"));
            }
            sends = attempt;
            let sent = post.send_json(&body);
            let (status, delay, text) = match sent {
                Ok(mut reply) => {
                    let status = reply.status().as_u16();
                    let delay = reply
                        .headers()
                        .get("Retry-After")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|h| h.parse::<f64>().ok())
                        .and_then(|s| Duration::try_from_secs_f64(s).ok());
                    let text = reply
                        .body_mut()
                        .read_to_string()
                        .map_err(|e| error("decision.response_parse", e))?;
                    (
                        status,
                        delay,
                        if key.is_empty() {
                            text
                        } else {
                            text.replace(&key, "[redacted]")
                        },
                    )
                }
                Err(e) => {
                    if attempt < 5 {
                        wait(Duration::from_secs(2u64.pow(attempt - 1)));
                        continue;
                    }
                    return Err(Error::Failure {
                        id: "http.network",
                        action: "Retry later.",
                        message: if key.is_empty() {
                            e.to_string()
                        } else {
                            e.to_string().replace(&key, "[redacted]")
                        },
                        category: "network",
                        attempts: Some(attempt),
                        session_id: None,
                    });
                }
            };
            if (200..300).contains(&status) {
                let decoded = serde_json::from_str(&text)
                    .map_err(|e| error("decision.response_parse", format!("{e}: {text}")))
                    .and_then(|raw| response(request, raw, provider.wire));
                return decoded;
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
            if busy && attempt < 5 {
                let _hold = pause.lock().map_err(|e| error("http.network", e))?;
                wait(delay.unwrap_or_else(|| Duration::from_secs(2u64.pow(attempt - 1))));
                continue;
            }
            return Err(Error::Failure {
                id: if quota {
                    "http.quota_exhausted"
                } else if busy {
                    "http.provider_busy"
                } else if status == 401 {
                    "http.auth"
                } else {
                    "decision.api_status"
                },
                message: text,
                action: if busy {
                    "Retry later."
                } else if status == 401 {
                    "Check the provider API key."
                } else {
                    "Check the provider response."
                },
                category: if quota {
                    "quota"
                } else if busy {
                    "busy"
                } else if status == 401 {
                    "auth"
                } else {
                    "other"
                },
                attempts: Some(attempt),
                session_id: None,
            });
        }
        Err(error("http.network", "send count exhausted"))
    })();
    if cancel.is_cancelled() {
        outcome = Err(error("decision.cancelled", "request cancelled"));
    }
    if let Err(Error::Failure {
        session_id: id,
        attempts,
        ..
    }) = &mut outcome
    {
        *id = Some(session_id.clone());
        if cancel.is_cancelled() {
            *attempts = Some(sends);
        }
    }
    if let Ok(value) = &mut outcome
        && let Some(fields) = value.as_object_mut()
    {
        fields.insert("attempts".into(), json!(sends));
        fields.insert("session_id".into(), json!(session_id));
        fields.insert("question_digests".into(), json!(digests));
    }
    if let Ok(value) = &outcome {
        let _ = log(logger, "decision.result", json!({"response":value}));
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
            let _ = log(logger, "error", json!(error("log.session_write", e)));
        }
    }
    let _ = logger.flush();
    outcome
}
pub fn batch(
    requests: &[Value],
    options: &Options,
    logger: &Logger,
    cancel: &Cancel,
    wait: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, String> + Sync),
    on_result: &(dyn Fn(usize, &Value) + Sync),
) -> Result<Value, Error> {
    requests.iter().try_for_each(validate_request_size)?;
    if options.settings.workers == 0 {
        return Err(error("decidr.invalid_workers", "workers must be positive"));
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::channel();
    Ok(std::thread::scope(|scope| {
        for _ in 0..options.settings.workers.min(requests.len()) {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(request) = requests.get(index) else {
                        break;
                    };
                    let value = report(
                        logger,
                        send(request, options, logger, cancel, wait, keyring),
                    );
                    on_result(index, &value);
                    let _ = sender.send((index, value));
                }
            });
        }
        drop(sender);
        let mut results = receiver.into_iter().collect::<Vec<_>>();
        results.sort_by_key(|(index, _)| *index);
        json!(
            results
                .into_iter()
                .map(|(index, value)| json!({"index":index,"result":value}))
                .collect::<Vec<_>>()
        )
    }))
}
pub fn repository() -> Result<(PathBuf, String), Error> {
    let cwd = std::env::current_dir().map_err(|e| error("config.repo", e))?;
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let root = git(&["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or(cwd);
    let origin = git(&["remote", "get-url", "origin"]);
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("config.repo", "host name unavailable"))?;
    let name = repository_identity(origin.as_deref(), &host, &root)?;
    Ok((root, name))
}
pub fn repository_identity(origin: Option<&str>, host: &str, root: &Path) -> Result<String, Error> {
    let remote = origin
        .map(|s| s.trim_end_matches('/').trim_end_matches(".git"))
        .map(|s| {
            s.split_once("://")
                .map_or_else(|| s.replacen(':', "/", 1), |(_, s)| s.to_owned())
        })
        .and_then(|s| {
            let s = s.split_once('@').map(|(_, s)| s).unwrap_or(&s);
            let (host, path) = s.split_once('/')?;
            let host = host.split_once(':').map(|(host, _)| host).unwrap_or(host);
            (!host.is_empty()).then(|| format!("{host}/{path}"))
        });
    let name = remote.unwrap_or_else(|| {
        format!(
            "local/{host}/{}",
            root.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(error("config.repo", "invalid repository identity"));
    }
    Ok(name)
}

pub fn doctor(config_dir: Option<&Path>, project: Option<&str>) -> Result<Value, Error> {
    let (root, _) = repository()?;
    let local = root.join(".sc/sc-agent");
    let global = config_dir
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(|| {
            std::env::home_dir()
                .map(|p| p.join(".sc-agent"))
                .ok_or_else(|| error("config.home", "Home directory unavailable"))
        })?;
    let loaded = config(
        root.join(".git").exists().then_some(local.as_path()),
        &global,
        &std::env::vars_os().collect::<Vec<_>>(),
        project,
    )?;
    Ok(json!({"settings":layers::resolved(&loaded,KEYS)}))
}

pub fn report(logger: &Logger, outcome: Result<Value, Error>) -> Value {
    if let Err(error) = &outcome {
        let _ = log(logger, "error", json!(error));
    }
    result(outcome)
}
