use serde_json::{Value, json};
use std::{env, fs, path::PathBuf, sync::mpsc, time::Duration};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Noul {
        noul: f64,
    },
    Choice {
        choice: String,
        confidence: f64,
        probabilities: std::collections::BTreeMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        legend: std::collections::BTreeMap<String, String>,
        probabilities: std::collections::BTreeMap<String, f64>,
    },
    Box {
        r#box: Coordinates,
        confidence: Option<f64>,
    },
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Coordinates {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

fn answer_schema(request: &Value) -> Result<Value, Value> {
    let questions = request
        .get("questions")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            error(
                "invalid_request",
                "Questions are missing",
                "Provide a questions object",
            )
        })?;
    let mut properties = serde_json::Map::new();
    for (name, question) in questions {
        let kind = question.get("type").and_then(Value::as_str).unwrap_or("");
        let criteria = question.get("criteria");
        let names: Vec<String> = if kind == "choice" {
            criteria
                .and_then(Value::as_object)
                .map(|v| v.keys().cloned().collect())
                .unwrap_or_default()
        } else if kind == "score" {
            criteria
                .and_then(Value::as_array)
                .map(|v| (0..v.len()).map(|n| n.to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if (kind == "choice" || kind == "score") && names.is_empty() {
            return Err(error(
                "invalid_question",
                "Question criteria are missing",
                "Provide question criteria",
            ));
        }
        let numeric: serde_json::Map<String, Value> = names
            .iter()
            .map(|n| (n.clone(), json!({"type":"number"})))
            .collect();
        let strings: serde_json::Map<String, Value> = names
            .iter()
            .map(|n| (n.clone(), json!({"type":"string"})))
            .collect();
        let probabilities = json!({"type":"object", "properties":numeric, "required":names, "additionalProperties":false});
        let legend = json!({"type":"object", "properties":strings, "required":names, "additionalProperties":false});
        let schema = match kind {
            "noul" => {
                json!({"type":"object", "properties":{"type":{"const":"noul"}, "noul":{"type":"number"}}, "required":["type","noul"], "additionalProperties":false})
            }
            "choice" => {
                json!({"type":"object", "properties":{"type":{"const":"choice"}, "choice":{"type":"string"}, "confidence":{"type":"number"}, "probabilities":probabilities}, "required":["type","choice","confidence","probabilities"], "additionalProperties":false})
            }
            "score" => {
                json!({"type":"object", "properties":{"type":{"const":"score"}, "score":{"type":"number"}, "confidence":{"type":"number"}, "legend":legend, "probabilities":probabilities}, "required":["type","score","confidence","legend","probabilities"], "additionalProperties":false})
            }
            "box" => {
                json!({"type":"object", "properties":{"type":{"const":"box"}, "box":{"type":"object", "properties":{"top":{"type":"number"},"bottom":{"type":"number"},"left":{"type":"number"},"right":{"type":"number"}}, "required":["top","bottom","left","right"], "additionalProperties":false}, "confidence":{"type":["number","null"]}}, "required":["type","box","confidence"], "additionalProperties":false})
            }
            _ => {
                return Err(error(
                    "invalid_question",
                    "Unsupported question type",
                    "Use noul, choice, score, or box",
                ));
            }
        };
        properties.insert(name.clone(), schema);
    }
    Ok(
        json!({"type":"object", "properties":{"answers":{"type":"object", "required":properties.keys().collect::<Vec<_>>(), "properties": properties, "additionalProperties":false}}, "required":["answers"], "additionalProperties":false}),
    )
}

fn valid_answers(value: &Value, questions: &Value) -> bool {
    let Some(answers) = value.get("answers").and_then(Value::as_object) else {
        return false;
    };
    let Some(questions) = questions.as_object() else {
        return false;
    };
    answers
        .values()
        .all(|answer| serde_json::from_value::<Answer>(answer.clone()).is_ok())
        && questions.iter().all(|(name, question)| {
            answers.get(name).and_then(|answer| answer.get("type")) == question.get("type")
                && answers.contains_key(name)
        })
}

const PROVIDERS: &[(&str, &str, &str)] = &[
    ("typesafe", "https://api.typesafe.ai/v1", "TYPESAFE_API_KEY"),
    (
        "anthropic",
        "https://api.anthropic.com/v1",
        "ANTHROPIC_API_KEY",
    ),
    ("openai", "https://api.openai.com/v1", "OPENAI_API_KEY"),
    (
        "openrouter",
        "https://openrouter.ai/api/v1",
        "OPENROUTER_API_KEY",
    ),
    ("together", "https://api.together.ai/v1", "TOGETHER_API_KEY"),
    (
        "nous",
        "https://inference-api.nousresearch.com/v1",
        "NOUS_API_KEY",
    ),
];

pub fn error(id: &str, message: &str, action: &str) -> Value {
    json!({"ok": false, "error": {"id": id, "message": message, "action": action}})
}

const KEYS: &[sc_config::layers::Key] = &[
    sc_config::layers::Key {
        path: "project",
        env: sc_config::layers::EnvValue::String,
    },
    sc_config::layers::Key {
        path: "workers",
        env: sc_config::layers::EnvValue::Json,
    },
    sc_config::layers::Key {
        path: "local_runners",
        env: sc_config::layers::EnvValue::Json,
    },
    sc_config::layers::Key {
        path: "local_runners.*.host",
        env: sc_config::layers::EnvValue::String,
    },
    sc_config::layers::Key {
        path: "local_runners.*.port",
        env: sc_config::layers::EnvValue::Json,
    },
    sc_config::layers::Key {
        path: "local_runners.*.model",
        env: sc_config::layers::EnvValue::String,
    },
];

fn configuration(explicit: Option<&str>) -> Result<sc_config::layers::Layered<Value>, Value> {
    use sc_config::layers::{FileLayer, Inputs};
    let local = env::var_os("SC_AGENT_TEST_REPO_ROOT")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_default();
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let paths = [
        local.join(".sc/decidr.json"),
        local.join(".sc/sc-agent.json"),
        home.join(".sc-agent/decidr.json"),
        home.join(".sc-agent/sc-agent.json"),
    ];
    let mut flags = std::collections::BTreeMap::new();
    if let Some(value) = explicit.filter(|v| !v.is_empty()) {
        flags.insert("project".to_owned(), json!(value));
    }
    let environment: Vec<_> = env::vars_os().collect();
    let inputs = Inputs {
        files: paths
            .into_iter()
            .map(|path| FileLayer {
                path,
                accepts: KEYS,
            })
            .collect(),
        env_prefix: "DECIDR_",
        env_aliases: &[("SC_PROJECT", "project")],
        env_accepts: KEYS,
        flags,
        flag_names: Default::default(),
        keys: KEYS,
        environment: &environment,
    };
    sc_config::layers::load(
        &inputs,
        |value| Ok::<_, std::convert::Infallible>(value),
        |_| None,
    )
    .map_err(|_| {
        error(
            "invalid_config",
            "Cannot load configuration",
            "Check configuration files and values",
        )
    })
}

pub fn project(explicit: Option<&str>) -> Result<(String, Option<String>), Value> {
    let config = configuration(explicit)?;
    match config
        .config
        .get("project")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(value) => Ok((
            value.to_owned(),
            config.sources.get("project").map(ToString::to_string),
        )),
        None => Err(error(
            "missing_project",
            "Project missing: --project, SC_PROJECT, local decidr.json, local sc-agent.json, global decidr.json, global sc-agent.json",
            "Set a project",
        )),
    }
}

pub fn doctor(explicit: Option<&str>) -> Value {
    let config = match configuration(explicit) {
        Ok(config) => config,
        Err(e) => return e,
    };
    let mut settings = serde_json::Map::new();
    for row in sc_config::layers::resolved(&config, KEYS) {
        settings.insert(row.path, json!({"value": row.value, "source": row.source}));
    }
    json!({"ok": true, "value": settings})
}

fn config() -> Result<Value, Value> {
    configuration(None).map(|v| v.config)
}

fn key(name: &str) -> Result<String, Value> {
    if let Ok(value) = env::var(name) {
        if !value.is_empty() {
            return Ok(value);
        }
    }
    let root = env::var_os("SC_AGENT_TEST_REPO_ROOT")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_default();
    if let Ok(iter) = dotenvy::from_path_iter(root.join(".env")) {
        for (k, v) in iter.flatten() {
            if k == name && !v.is_empty() {
                return Ok(v);
            }
        }
    }
    if let Ok(value) = env::var("SC_AGENT_TEST_KEYRING_VALUE") {
        return Ok(value);
    }
    let name = name.to_owned();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let value = keyring::Entry::new("sc-agent", &name)
            .ok()
            .and_then(|e| e.get_password().ok());
        let _ = tx.send(value);
    });
    match rx.recv_timeout(Duration::from_secs(15)).ok().flatten() {
        Some(value) => Ok(value),
        None => Err(error(
            "missing_key",
            "API key unavailable",
            "Set the provider API key",
        )),
    }
}

pub fn models() -> Value {
    json!({"typesafe": {"jev": "jev-1.14.0", "decider": "decider"}, "anthropic": {"haiku": "claude-haiku-5-5"}, "openai": {"luna": "gpt-6-luna"}, "openrouter": {"haiku": "anthropic/claude-haiku-5.5", "luna": "openai/gpt-6-luna"}, "together": {"haiku": "anthropic/claude-haiku-5-5", "luna": "openai/gpt-6-luna"}, "nous": {}, "local": {"laya": "laya"}})
}

fn redact(value: &mut Value, secret: &str) {
    match value {
        Value::String(text) if !secret.is_empty() => *text = text.replace(secret, "[redacted]"),
        Value::Array(items) => {
            for item in items {
                redact(item, secret);
            }
        }
        Value::Object(items) => {
            for item in items.values_mut() {
                redact(item, secret);
            }
        }
        _ => {}
    }
}

pub fn send(
    input: &str,
    model: &str,
    provider: Option<&str>,
    explicit_project: Option<&str>,
    agent_role: Option<&str>,
    sleep: &dyn Fn(Duration),
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Value {
    if let Err(e) = project(explicit_project) {
        return e;
    }
    let request: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(_) => {
            return error(
                "invalid_request",
                "Request is not JSON",
                "Provide valid JSON",
            );
        }
    };
    let tables = models();
    let detected = if model == "laya" {
        Some("local")
    } else {
        PROVIDERS.iter().find_map(|(name, _, _)| {
            tables
                .get(*name)
                .and_then(Value::as_object)
                .filter(|table| {
                    table.get(model).is_some()
                        || table.values().any(|id| id.as_str() == Some(model))
                })
                .map(|_| *name)
        })
    };
    let name = provider.or(detected).unwrap_or("");
    let known = tables
        .get(name)
        .and_then(Value::as_object)
        .is_some_and(|table| {
            table.get(model).is_some() || table.values().any(|id| id.as_str() == Some(model))
        });
    if !matches!(name, "openrouter" | "together" | "nous" | "local") && !known {
        return error(
            "unknown_model",
            "Model is unknown",
            "Choose a supported model",
        );
    }
    let (base, secret, local_model) = if name == "local" {
        let settings = match config() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let runner = settings
            .get("local_runners")
            .and_then(|v| v.get("laya"))
            .unwrap_or(&Value::Null);
        let Some(host) = runner.get("host").and_then(Value::as_str) else {
            return error(
                "invalid_runner",
                "Laya host is missing",
                "Configure local_runners.laya.host",
            );
        };
        let Some(port) = runner
            .get("port")
            .and_then(Value::as_u64)
            .filter(|p| *p > 0 && *p <= 65535)
        else {
            return error(
                "invalid_runner",
                "Laya port is missing",
                "Configure local_runners.laya.port",
            );
        };
        let Some(local_model) = runner
            .get("model")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            return error(
                "invalid_runner",
                "Laya model is missing",
                "Configure local_runners.laya.model",
            );
        };
        (
            format!("http://{host}:{port}/v1"),
            String::new(),
            Some(local_model.to_owned()),
        )
    } else {
        let Some((_, base, key_name)) = PROVIDERS.iter().find(|(p, _, _)| *p == name) else {
            return error(
                "unknown_provider",
                "Provider is unknown",
                "Choose a supported provider",
            );
        };
        let secret = match key(key_name) {
            Ok(value) => value,
            Err(e) => return e,
        };
        ((*base).to_owned(), secret, None)
    };
    if let Some(role) = agent_role {
        if let Err(e) = session(role, explicit_project, input, Some(&secret)) {
            return e;
        }
    }
    let id = tables
        .get(name)
        .and_then(|v| v.get(model))
        .and_then(Value::as_str)
        .unwrap_or(model)
        .to_owned();
    let base = env::var("SC_AGENT_TEST_BASE_URL").unwrap_or(base);
    let questions = request.get("questions").cloned().unwrap_or(Value::Null);
    let (path, body) = if name == "typesafe" || name == "local" {
        let mut request = request;
        if let Some(object) = request.as_object_mut() {
            object.insert("model".into(), json!(local_model.unwrap_or(id)));
        }
        ("systemone", request)
    } else {
        let schema = match answer_schema(&request) {
            Ok(schema) => schema,
            Err(e) => return e,
        };
        let prompt = request.to_string();
        let system =
            "Answer every question in the supplied request using the required JSON schema.";
        if name == "anthropic" {
            (
                "messages",
                json!({"model": id, "max_tokens": 4096, "system": system, "messages": [{"role": "user", "content": prompt}], "output_config": {"format": {"type": "json_schema", "schema": schema}}}),
            )
        } else {
            (
                "chat/completions",
                json!({"model": id, "messages": [{"role": "system", "content": system}, {"role": "user", "content": prompt}], "response_format": {"type": "json_schema", "json_schema": {"name": "decisions", "strict": true, "schema": schema}}}),
            )
        }
    };
    for attempt in 0..3 {
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
            return error("cancelled", "Request cancelled", "Retry when ready");
        }
        let mut call = ureq::post(&format!("{base}/{path}"))
            .timeout(Duration::from_secs(60))
            .set("Content-Type", "application/json");
        call = if name == "anthropic" {
            call.set("x-api-key", &secret)
                .set("anthropic-version", "2023-06-01")
        } else if name == "local" {
            call
        } else {
            call.set("Authorization", &format!("Bearer {secret}"))
        };
        match call.send_json(body.clone()) {
            Ok(response) => {
                let value: Value = match response.into_json() {
                    Ok(v) => v,
                    Err(_) => {
                        return error(
                            "invalid_response",
                            "Provider returned invalid JSON",
                            "Retry or check provider",
                        );
                    }
                };
                let parsed = if name == "typesafe" || name == "local" {
                    Some(value.clone())
                } else {
                    let text = if name == "anthropic" {
                        value.pointer("/content/0/text")
                    } else {
                        value.pointer("/choices/0/message/content")
                    };
                    text.and_then(Value::as_str)
                        .and_then(|v| serde_json::from_str(v).ok())
                };
                if let Some(mut value) = parsed.filter(|value| valid_answers(value, &questions)) {
                    redact(&mut value, &secret);
                    return json!({"ok": true, "value": value});
                }
                if name != "typesafe" && name != "local" && attempt < 2 {
                    continue;
                }
                return error(
                    "schema_mismatch",
                    "Provider returned an invalid answer",
                    "Check the response and retry",
                );
            }
            Err(ureq::Error::Status(code, response)) => {
                let delay = response
                    .header("Retry-After")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1);
                let body = response.into_string().unwrap_or_default();
                if ![429, 502, 529].contains(&code)
                    || body.to_lowercase().contains("quota")
                    || attempt == 2
                {
                    return error(
                        if code == 401 {
                            "authentication"
                        } else {
                            "provider_error"
                        },
                        "Provider rejected request",
                        "Check provider status and credentials",
                    );
                }
                sleep(Duration::from_secs(delay));
            }
            Err(_) if attempt < 2 => sleep(Duration::from_secs(1)),
            Err(_) => {
                return error(
                    "network_error",
                    "Provider could not be reached",
                    "Check the network and retry",
                );
            }
        }
    }
    error("network_error", "Provider could not be reached", "Retry")
}

pub fn open_default_logger(
    name: &str,
    root_override: &str,
) -> Result<sc_observability::v2::Logger, Value> {
    let service = sc_observability::ServiceName::new(name).map_err(|_| {
        error(
            "invalid_name",
            "Invalid log name",
            "Use a valid client name",
        )
    })?;
    let root = env::var_os(root_override)
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".sc-agent")))
        .ok_or_else(|| error("missing_home", "Home directory unavailable", "Set HOME"))?;
    let mut config = sc_observability::v2::LoggerConfig::default_for(service, root);
    config.enable_console_sink = false;
    sc_observability::v2::Logger::new(config)
        .map_err(|_| error("log_open", "Cannot open log", "Check log directory"))
}

pub fn log(logger: &sc_observability::v2::Logger, action: &str) -> Result<(), Value> {
    let event = log_event(logger.service_name().clone(), action)?;
    logger
        .log(event)
        .map_err(|_| error("log_write", "Cannot write log", "Check log directory"))?;
    logger
        .flush()
        .map_err(|_| error("log_write", "Cannot flush log", "Check log directory"))
}

fn log_event(
    service: sc_observability::ServiceName,
    action: &str,
) -> Result<sc_observability::LogEvent, Value> {
    use sc_observability::{
        ActionName, Level, LogEvent, ProcessIdentity, SchemaVersion, TargetCategory, Timestamp,
    };
    let invalid = || error("invalid_event", "Invalid log event", "Use a valid event");
    let event = LogEvent {
        version: SchemaVersion::new("v1").map_err(|_| invalid())?,
        timestamp: Timestamp::now_utc(),
        level: Level::Info,
        service,
        target: TargetCategory::new("sc-agent").map_err(|_| invalid())?,
        action: ActionName::new(action).map_err(|_| invalid())?,
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: Default::default(),
    };
    Ok(event)
}

pub fn session(
    agent_role: &str,
    explicit_project: Option<&str>,
    request: &str,
    provider_secret: Option<&str>,
) -> Result<(), Value> {
    use sc_observability::v2::LogSink;
    let (project, _) = project(explicit_project)?;
    let valid = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    };
    if !valid(agent_role) || !valid(&project) {
        return Err(error(
            "invalid_session",
            "Invalid session path",
            "Use valid role and project names",
        ));
    }
    let root = env::var_os("DECIDR_LOG_ROOT")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".sc-agent")))
        .ok_or_else(|| error("missing_home", "Home directory unavailable", "Set HOME"))?;
    let folder = env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|v| v.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "unknown".into());
    let origin = std::process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    let repo = if let Some(url) = origin {
        url.trim()
            .trim_end_matches(".git")
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("ssh://")
            .trim_start_matches("git@")
            .replacen(':', "/", 1)
    } else {
        let host = std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "unknown".into());
        format!("local/{}/{folder}", host.trim())
    };
    if repo
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\\'))
    {
        return Err(error(
            "invalid_repo",
            "Invalid repository identity",
            "Check the git origin URL",
        ));
    }
    let directory = root
        .join("sessions")
        .join(&repo)
        .join(agent_role)
        .join(&project);
    fs::create_dir_all(&directory).map_err(|_| {
        error(
            "session_write",
            "Cannot open session log",
            "Check log directory",
        )
    })?;
    let sink = sc_observability::v2::JsonlFileSink::open(
        directory.join("sessions.jsonl"),
        Default::default(),
    )
    .map_err(|_| {
        error(
            "session_write",
            "Cannot open session log",
            "Check log directory",
        )
    })?;
    let service = sc_observability::ServiceName::new("decidr")
        .map_err(|_| error("session_write", "Invalid session", "Check session"))?;
    let mut event = log_event(service, "decidr.send")?;
    event
        .fields
        .insert("session_id".into(), json!(ulid::Ulid::new().to_string()));
    event.fields.insert("repo".into(), json!(repo));
    event.fields.insert("agent_role".into(), json!(agent_role));
    event.fields.insert("project".into(), json!(project));
    let mut request = json!(request);
    if let Some(secret) = provider_secret {
        redact(&mut request, secret);
    }
    for (_, _, name) in PROVIDERS {
        if let Ok(secret) = env::var(name) {
            redact(&mut request, &secret);
        }
    }
    let repo_root = env::var_os("SC_AGENT_TEST_REPO_ROOT")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_default();
    if let Ok(entries) = dotenvy::from_path_iter(repo_root.join(".env")) {
        for (name, secret) in entries.flatten() {
            if PROVIDERS.iter().any(|(_, _, key)| *key == name) {
                redact(&mut request, &secret);
            }
        }
    }
    event.fields.insert("request".into(), request);
    sink.write(&event).map_err(|_| {
        error(
            "session_write",
            "Cannot write session log",
            "Check log directory",
        )
    })
}

pub fn send_batch(
    inputs: &[String],
    model: &str,
    provider: Option<&str>,
    project: Option<&str>,
    agent_role: Option<&str>,
    workers: usize,
    sleep: &(dyn Fn(Duration) + Sync),
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Value {
    if workers == 0 {
        return error(
            "invalid_workers",
            "Worker count must be positive",
            "Choose at least one worker",
        );
    }
    let mut rows = Vec::new();
    for (chunk_number, chunk) in inputs.chunks(workers).enumerate() {
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
            break;
        }
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .enumerate()
                .map(|(offset, input)| {
                    scope.spawn(move || {
                        (
                            chunk_number * workers + offset,
                            send(
                                input, model, provider, project, agent_role, sleep, cancelled,
                            ),
                        )
                    })
                })
                .collect();
            for handle in handles {
                match handle.join() {
                    Ok((position, result)) => rows.push(json!({"position": position, "result": result})),
                    Err(_) => rows.push(json!({"result": error("worker_error", "Worker failed", "Retry the request")})),
                }
            }
        });
    }
    json!({"ok": true, "value": rows})
}

pub fn question_hash(question: &str) -> Result<String, Value> {
    sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
        utf8_file_bytes: question.as_bytes(),
    })
    .map(|hash| hash.template().to_hex())
    .map_err(|_| {
        error(
            "invalid_question",
            "Question must be UTF-8 text",
            "Provide valid text",
        )
    })
}
