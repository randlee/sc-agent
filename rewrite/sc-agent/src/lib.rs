use sc_config::layers::{self, EnvValue, FileLayer, Inputs, Key, Layered};
use sc_observability::{LoggerConfig, ServiceName, v2::Logger};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
#[error("{id}: {message}")]
pub enum Error {
    Failure {
        id: &'static str,
        message: String,
        category: &'static str,
        status: Option<u16>,
        attempts: Option<u32>,
        retry_after: Option<Duration>,
    },
}
impl Error {
    pub fn detail(&self) -> Value {
        let Self::Failure {
            id,
            message,
            category,
            status,
            attempts,
            retry_after,
        } = self;
        let mut out = json!({"id":id,"message":message,"category":category,"action":match *category {"auth"=>"Check the provider API key.","config"=>"Fix the named configuration setting.","busy"|"network"=>"Retry later.",_=>"Fix the named input or provider response."}});
        if let Some(value) = out.as_object_mut() {
            if let Some(status) = status {
                value.insert("status".into(), json!(status));
            }
            if let Some(attempts) = attempts {
                value.insert("attempts".into(), json!(attempts));
            }
            if let Some(delay) = retry_after {
                value.insert(
                    "retry_after_ms".into(),
                    json!(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX)),
                );
            }
        }
        out
    }
}
pub fn error(id: &'static str, message: impl ToString) -> Error {
    let category = match id {
        "secrets.key_missing"
        | "secrets.key_malformed"
        | "secrets.keyring_read"
        | "secrets.keyring_timeout" => "auth",
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
        status: None,
        attempts: None,
        retry_after: None,
    }
}
pub fn result(value: Result<Value, Error>) -> Value {
    match value {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error.detail()}),
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
    pub local_runners: BTreeMap<String, Runner>,
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
        path: "project",
        env: EnvValue::String,
    },
    Key {
        path: "workers",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners.*.host",
        env: EnvValue::String,
    },
    Key {
        path: "local_runners.*.port",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners.*.model",
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
    let flags = project
        .map(|s| ("project".into(), json!(s)))
        .into_iter()
        .collect();
    layers::load(
        &Inputs {
            files,
            env_prefix: "DECIDR_",
            env_aliases: &[("SC_PROJECT", "project")],
            env_accepts: KEYS,
            flags,
            flag_names: BTreeMap::new(),
            keys: KEYS,
            environment,
        },
        |value| {
            let settings: Settings =
                serde_json::from_value(value).map_err(|e| error("config.invalid", e))?;
            if !(1..=64).contains(&settings.workers) {
                return Err(error("config.invalid", "workers must be in 1..=64"));
            }
            for runner in settings.local_runners.values() {
                if runner.port == 0
                    || runner.host.is_empty()
                    || runner.host.contains(['/', ':', ' '])
                {
                    return Err(error(
                        "config.invalid",
                        "local_runners host/port is invalid",
                    ));
                }
            }
            Ok(settings)
        },
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
pub fn home() -> Result<PathBuf, Error> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| error("config.home", "Home directory unavailable"))
}
pub fn log_root(name: &str) -> Result<PathBuf, Error> {
    std::env::var_os(format!(
        "{}_LOG_ROOT",
        name.to_uppercase().replace('-', "_")
    ))
    .filter(|s| !s.is_empty())
    .map(PathBuf::from)
    .map(Ok)
    .unwrap_or_else(|| home().map(|p| p.join(".sc-agent")))
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
        models: &[
            ("jev", "jev-1.13.0", "2026-09-15"),
            ("jev", "jev-1.14.0", "2026-10-14"),
        ],
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
    },
    Noul {
        noul: f64,
    },
    Box {
        r#box: Value,
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
    input.remove("model");
    input.insert("questions".into(), json!({question:selected}));
    let text = serde_json::to_vec(&input).map_err(|e| error("decision.encode", e))?;
    sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
        utf8_file_bytes: &text,
    })
    .map(|h| h.template().to_string())
    .map_err(|e| error("decision.encode", e))
}

pub struct Options {
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
pub fn log(logger: &Logger, action: &str, fields: Value) {
    let event = serde_json::from_value(
        json!({"version":"v1","timestamp":sc_observability::Timestamp::now_utc(),"level":sc_observability::Level::Info,"service":logger.service_name(),"target":"sc-agent","action":action,"identity":sc_observability::ProcessIdentity::default(),"fields":fields}),
    );
    if let Ok(event) = event {
        let _ = logger.log(event);
    }
}
pub fn validate(request: &Value) -> Result<(), Error> {
    if request.to_string().len() > 8 * 1024 * 1024 {
        return Err(error("decision.limits", "request exceeds 8 MiB"));
    }
    let questions = request
        .get("questions")
        .and_then(Value::as_object)
        .filter(|q| !q.is_empty())
        .ok_or_else(|| {
            error(
                "decision.request_parse",
                "questions must be a nonempty object",
            )
        })?;
    if !request.is_object() || request.get("state").is_none() {
        return Err(error("decision.request_parse", "state is required"));
    }
    for (id, q) in questions {
        if !matches!(
            q.get("type").and_then(Value::as_str),
            Some("choice" | "score" | "noul" | "box")
        ) {
            return Err(error(
                "decision.request_parse",
                format!("questions.{id}: unsupported question type"),
            ));
        }
    }
    Ok(())
}
fn wire(request: &Value, model: &str, wire: &str) -> Value {
    if wire == "systemone" {
        let mut body = request.clone();
        if let Some(body) = body.as_object_mut() {
            body.insert("model".into(), json!(model));
        }
        return body;
    }
    let properties=request.get("questions").and_then(Value::as_object).map(|questions|questions.iter().map(|(id,q)|{
        let kind=q.get("type").and_then(Value::as_str).unwrap_or("");
        let mut fields=json!({"type":{"type":"string","enum":[kind]}});
        if let Some(properties)=fields.as_object_mut() {
            for (name,value) in match kind {"choice"=>vec![("choice",json!({"type":"string"})),("confidence",json!({"type":"number"})),("probabilities",json!({"type":"object","additionalProperties":{"type":"number"}}))],"score"=>vec![("score",json!({"type":"number"})),("confidence",json!({"type":"number"}))],"noul"=>vec![("noul",json!({"type":"number"}))],_=>vec![("box",json!({"type":"object"}))]} {properties.insert(name.into(),value);}
        }
        let required=fields.as_object().map(|m|m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
        (id.clone(),json!({"type":"object","properties":fields,"required":required,"additionalProperties":false}))
    }).collect::<BTreeMap<_,_>>()).unwrap_or_default();
    let required = properties.keys().cloned().collect::<Vec<_>>();
    let schema = json!({"type":"object","properties":{"answers":{"type":"object","properties":properties,"required":required,"additionalProperties":false}},"required":["answers"],"additionalProperties":false});
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
    let parsed = serde_json::from_str(answer)
        .map_err(|e| error("decision.response_parse", format!("{e}: {answer}")))?;
    schema(request, &parsed)?;
    Ok(parsed)
}
pub fn send(
    request: &Value,
    options: &Options,
    logger: &Logger,
    state: (&AtomicBool, &Mutex<()>),
    sleep: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, String> + Sync),
) -> Result<Value, Error> {
    let outcome = (|| {
        validate(request)?;
        let project = options.settings.project_required()?;
        slug(project)?;
        slug(&options.agent_role)?;
        let (provider, model) = model(&options.model, options.provider.as_deref())?;
        let (base, model) = if provider.name == "local" {
            let runner = options.settings.local_runners.get("laya").ok_or_else(|| {
                error(
                    "config.runner",
                    "Set local_runners.laya host, port and model",
                )
            })?;
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
        let key = if provider.key.is_empty() {
            String::new()
        } else {
            key(
                provider.key,
                &std::env::vars_os().collect::<Vec<_>>(),
                &options.root,
                keyring,
            )?
        };
        if !key.is_empty()
            && url.starts_with("http:")
            && !(url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:"))
        {
            return Err(error(
                "http.key_on_plain_http",
                "Keys require HTTPS except on loopback",
            ));
        }
        let body = wire(request, &model, provider.wire);
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(options.timeout))
            .build()
            .new_agent();
        for attempt in 1..=5 {
            if state.0.load(Ordering::SeqCst) {
                return Err(error("decision.cancelled", "request cancelled"));
            }
            // A busy reply owns this lock through its injected wait. All senders use it.
            drop(state.1.lock().map_err(|e| error("http.network", e))?);
            let mut post = agent.post(&url).header("Content-Type", "application/json");
            if provider.name == "anthropic" {
                post = post
                    .header("x-api-key", &key)
                    .header("anthropic-version", "2023-06-01");
            } else if !key.is_empty() {
                post = post.header("Authorization", format!("Bearer {key}"));
            }
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
                        .with_config()
                        .limit(8 * 1024 * 1024)
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
                        sleep(Duration::from_secs(1));
                        continue;
                    }
                    return Err(Error::Failure {
                        id: "http.network",
                        message: if key.is_empty() {
                            e.to_string()
                        } else {
                            e.to_string().replace(&key, "[redacted]")
                        },
                        category: "network",
                        status: None,
                        attempts: Some(attempt),
                        retry_after: None,
                    });
                }
            };
            if (200..300).contains(&status) {
                let raw = serde_json::from_str(&text)
                    .map_err(|e| error("decision.response_parse", format!("{e}: {text}")))?;
                return response(request, raw, provider.wire);
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
                let _hold = state.1.lock().map_err(|e| error("http.network", e))?;
                sleep(delay.unwrap_or(Duration::from_secs(1)));
                continue;
            }
            return Err(Error::Failure {
                id: if quota {
                    "http.quota_exhausted"
                } else if busy {
                    "http.provider_busy"
                } else {
                    "decision.api_status"
                },
                message: text,
                category: if quota {
                    "quota"
                } else if busy {
                    "busy"
                } else if [401, 403].contains(&status) {
                    "auth"
                } else {
                    "other"
                },
                status: Some(status),
                attempts: Some(attempt),
                retry_after: delay,
            });
        }
        Err(error("http.network", "send count exhausted"))
    })();
    match &outcome {
        Ok(value) => log(logger, "decision.result", json!({"response":value})),
        Err(err) => log(logger, "error", err.detail()),
    }
    if let Ok(project) = options.settings.project_required()
        && slug(project).is_ok()
        && slug(&options.agent_role).is_ok()
    {
        let session_id = ulid::Ulid::new().to_string();
        let digests = request
            .get("questions")
            .and_then(Value::as_object)
            .map(|q| {
                q.keys()
                    .filter_map(|id| digest(request, id).ok().map(|hash| (id.clone(), hash)))
                    .collect::<BTreeMap<_, _>>()
            });
        let row = json!({"session_id":session_id,"repo":options.repo,"agent_role":options.agent_role,"project":project,"request":request,"question_digests":digests,"result":match &outcome {Ok(v)=>json!({"ok":true,"value":v}),Err(e)=>json!({"ok":false,"error":e.detail()})}});
        let path = options
            .log_root
            .join("sessions")
            .join(&options.repo)
            .join(&options.agent_role)
            .join(project);
        let _append = state.1.lock().map_err(|e| error("config.session", e))?;
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
            return Err(error("config.session", e));
        }
    }
    let _ = logger.flush();
    outcome
}
pub fn batch(
    requests: &[Value],
    options: &Options,
    logger: &Logger,
    state: (&AtomicBool, &Mutex<()>),
    sleep: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, String> + Sync),
    on_result: &(dyn Fn(usize, &Value) + Sync),
) -> Result<Value, Error> {
    if requests.len() > 10000
        || requests.iter().map(|r| r.to_string().len()).sum::<usize>() > 256 * 1024 * 1024
    {
        return Err(error(
            "decision.limits",
            "batch exceeds 10000 requests or 256 MiB",
        ));
    }
    for (index, request) in requests.iter().enumerate() {
        validate(request)
            .map_err(|e| error("decision.request_parse", format!("line {}: {e}", index + 1)))?;
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::channel();
    Ok(std::thread::scope(|scope| {
        for _ in 0..options.settings.workers {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    if state.0.load(Ordering::SeqCst) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(request) = requests.get(index) else {
                        break;
                    };
                    let value = result(send(request, options, logger, state, sleep, keyring));
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
    let remote = origin
        .as_deref()
        .map(|s| s.trim_end_matches(".git").trim_end_matches('/'))
        .map(|s| s.split_once("://").map(|(_, s)| s).unwrap_or(s))
        .map(|s| s.split_once('@').map(|(_, s)| s).unwrap_or(s))
        .filter(|s| s.contains('/') && !s.starts_with('/'))
        .map(|s| s.replacen(':', "/", 1));
    let name = remote.unwrap_or_else(|| {
        format!(
            "local/{}/{}",
            std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".into()),
            root.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(error("config.repo", "invalid repository identity"));
    }
    Ok((root, name))
}
