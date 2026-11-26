//! Provider endpoints, model rows and credential resolution for a Run.
use crate::{Context, Environment, Error, KeyringError, Secret, Settings, error};
use serde::Serialize;
use serde_json::{Value, json};
use std::{borrow::Cow, collections::BTreeMap, path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum Wire {
    #[serde(rename = "systemone")]
    SystemOne,
    #[serde(rename = "messages")]
    Messages,
    #[serde(rename = "chat/completions")]
    ChatCompletions,
}
#[derive(Clone, Copy, Debug)]
pub struct ReplyShape {
    pub model: &'static str,
    pub input: &'static str,
    pub output: &'static str,
}
impl Wire {
    pub fn path(self) -> &'static str {
        match self {
            Self::SystemOne => "systemone",
            Self::Messages => "messages",
            Self::ChatCompletions => "chat/completions",
        }
    }
    pub fn auth(
        self,
        post: ureq::RequestBuilder<ureq::typestate::WithBody>,
        key: &str,
    ) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
        match self {
            Self::Messages => post
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01"),
            _ => post.header("Authorization", format!("Bearer {key}")),
        }
    }
    pub const fn reply(self) -> ReplyShape {
        match self {
            Self::ChatCompletions => ReplyShape {
                model: "/model",
                input: "/usage/prompt_tokens",
                output: "/usage/completion_tokens",
            },
            _ => ReplyShape {
                model: "/model",
                input: "/usage/input_tokens",
                output: "/usage/output_tokens",
            },
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "at", rename_all = "lowercase")]
pub enum Base {
    Url(&'static str),
    Local(&'static str),
}
#[derive(Debug, Serialize)]
pub struct Provider {
    pub name: &'static str,
    pub base: Base,
    pub wire: Wire,
    pub key: Option<&'static str>,
}
// The registry derives its public list from the same rows that declare the endpoint constants.
macro_rules! providers {
    ($($constant:ident: $name:literal, $base:expr, $wire:ident, $key:expr;)*) => {
        $(pub static $constant: Provider = Provider { name:$name,base:$base,wire:Wire::$wire,key:$key };)*
        pub static PROVIDERS: &[&Provider] = &[$(&$constant),*];
    };
}
providers! {
    TYPESAFE: "typesafe", Base::Url("https://api.typesafe.ai/v1"), SystemOne, Some("TYPESAFE_API_KEY");
    ANTHROPIC: "anthropic", Base::Url("https://api.anthropic.com/v1"), Messages, Some("ANTHROPIC_API_KEY");
    OPENAI: "openai", Base::Url("https://api.openai.com/v1"), ChatCompletions, Some("OPENAI_API_KEY");
    OPENROUTER: "openrouter", Base::Url("https://openrouter.ai/api/v1"), ChatCompletions, Some("OPENROUTER_API_KEY");
    TOGETHER: "together", Base::Url("https://api.together.ai/v1"), ChatCompletions, Some("TOGETHER_API_KEY");
    NOUS: "nous", Base::Url("https://inference-api.nousresearch.com/v1"), ChatCompletions, Some("NOUS_API_KEY");
    LAYA: "laya", Base::Local("laya"), SystemOne, None;
    DECIDER_4B: "decider_4b", Base::Local("decider_4b"), SystemOne, None;
    DECIDER_2B: "decider_2b", Base::Local("decider_2b"), SystemOne, None;
}
#[derive(Clone, Copy, Debug)]
pub struct ModelRow<'a> {
    pub alias: &'a str,
    pub id: &'a str,
    pub released: &'a str,
    pub provider: &'static Provider,
}
const fn row(
    alias: &'static str,
    id: &'static str,
    released: &'static str,
    provider: &'static Provider,
) -> ModelRow<'static> {
    ModelRow {
        alias,
        id,
        released,
        provider,
    }
}
pub static MODELS: &[ModelRow<'static>] = &[
    row("fable", "claude-fable-5-2", "2026-11-09", &ANTHROPIC),
    row(
        "fable",
        "anthropic/claude-fable-5.2",
        "2026-11-09",
        &OPENROUTER,
    ),
    row(
        "fable",
        "anthropic/claude-fable-5-2",
        "2026-11-09",
        &TOGETHER,
    ),
    row("terra", "gpt-6.1-terra", "2026-10-26", &OPENAI),
    row("terra", "openai/gpt-6.1-terra", "2026-10-26", &OPENROUTER),
    row("terra", "openai/gpt-6.1-terra", "2026-10-26", &TOGETHER),
    row("jev", "jev-latest", "2026-10-14", &TYPESAFE),
    row("haiku", "claude-haiku-5-5", "2026-10-07", &ANTHROPIC),
    row(
        "haiku",
        "anthropic/claude-haiku-5.5",
        "2026-10-07",
        &OPENROUTER,
    ),
    row(
        "haiku",
        "anthropic/claude-haiku-5-5",
        "2026-10-07",
        &TOGETHER,
    ),
    row("haiku", "anthropic/claude-haiku-5.5", "2026-10-07", &NOUS),
    row("sol", "gpt-6.1-sol", "2026-09-29", &OPENAI),
    row("sol", "openai/gpt-6.1-sol", "2026-09-29", &OPENROUTER),
    row("sol", "openai/gpt-6.1-sol", "2026-09-29", &TOGETHER),
    row("sonnet", "claude-sonnet-5-5", "2026-09-28", &ANTHROPIC),
    row(
        "sonnet",
        "anthropic/claude-sonnet-5.5",
        "2026-09-28",
        &OPENROUTER,
    ),
    row(
        "sonnet",
        "anthropic/claude-sonnet-5-5",
        "2026-09-28",
        &TOGETHER,
    ),
    row("decider", "decider-4b-v2.1", "2026-09-24", &DECIDER_4B),
    row("decider", "decider-2b-v11", "2026-09-24", &DECIDER_2B),
    row("laya", "laya:en", "2026-09-23", &LAYA),
    row("opus", "claude-opus-5-5", "2026-09-22", &ANTHROPIC),
    row(
        "opus",
        "anthropic/claude-opus-5.5",
        "2026-09-22",
        &OPENROUTER,
    ),
    row("opus", "anthropic/claude-opus-5-5", "2026-09-22", &TOGETHER),
    row("luna", "gpt-6-luna", "2026-09-22", &OPENAI),
    row("luna", "openai/gpt-6-luna", "2026-09-22", &OPENROUTER),
    row("luna", "openai/gpt-6-luna", "2026-09-22", &TOGETHER),
    row("luna", "openai/gpt-6-luna", "2026-09-22", &NOUS),
    row("astra", "gpt-6-astra", "2026-09-03", &OPENAI),
    row("astra", "openai/gpt-6-astra", "2026-09-03", &OPENROUTER),
    row("astra", "openai/gpt-6-astra", "2026-09-03", &TOGETHER),
];
pub fn named(name: &str) -> Result<&'static Provider, Error> {
    PROVIDERS
        .iter()
        .copied()
        .find(|p| p.name == name)
        .ok_or_else(|| error("types.unknown_provider", name))
}
pub fn table(settings: &Settings) -> Vec<ModelRow<'_>> {
    settings
        .models
        .iter()
        .map(|(alias, entry)| ModelRow {
            alias,
            id: &entry.id,
            released: entry.released.as_deref().unwrap_or(""),
            provider: entry.provider,
        })
        .chain(MODELS.iter().copied())
        .collect()
}
pub fn candidates<'a>(selector: &str, settings: &'a Settings) -> Vec<ModelRow<'a>> {
    let rows = table(settings);
    if let Some(configured) = rows
        .iter()
        .take(settings.models.len())
        .find(|r| r.alias == selector || r.id == selector)
    {
        return vec![*configured];
    }
    rows.into_iter()
        .filter(|r| r.alias == selector || r.id == selector)
        .collect()
}
pub fn resolve(selector: &str, ctx: &Context) -> Result<ResolvedModel, Error> {
    resolve_using(selector, ctx, &|key| ctx.keys.find(key))
}
pub(crate) fn resolve_using(
    selector: &str,
    ctx: &Context,
    lookup: &dyn Fn(&str) -> Result<Option<Credential>, Error>,
) -> Result<ResolvedModel, Error> {
    let rows = candidates(selector, &ctx.settings);
    let mut memo = BTreeMap::new();
    let mut failure = None;
    for row in rows
        .iter()
        .filter(|r| r.provider.key.is_none())
        .chain(rows.iter().filter(|r| r.provider.key.is_some()))
    {
        let credential = if let Some(key) = row.provider.key {
            match memo.entry(key).or_insert_with(|| lookup(key)) {
                Ok(Some(key)) => Some(key.clone()),
                Ok(None) => continue,
                Err(e) => {
                    failure = Some(e.clone());
                    continue;
                }
            }
        } else {
            None
        };
        return Ok(ResolvedModel {
            alias: row.alias.into(),
            id: row.id.into(),
            provider: row.provider,
            credential,
        });
    }
    Err(failure.unwrap_or_else(|| {
        error(
            if rows.is_empty() {
                "types.unknown_short_name"
            } else {
                "secrets.key_missing"
            },
            if rows.is_empty() {
                selector.into()
            } else {
                format!(
                    "Credentials not found in environment, .env or keyring: {}",
                    rows.iter()
                        .filter_map(|r| r.provider.key)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        )
    }))
}
#[derive(Clone, Debug)]
pub struct ResolvedModel {
    pub alias: String,
    pub id: String,
    pub provider: &'static Provider,
    pub credential: Option<Credential>,
}
pub type KeyringLookup = dyn Fn(&str) -> Result<String, KeyringError> + Send + Sync;
pub struct KeySources {
    environment: Environment,
    dotenv: Environment,
    keyring: Arc<KeyringLookup>,
}
#[derive(Clone, Debug)]
pub struct Credential {
    pub value: Secret<String>,
    pub source: Source,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Environment,
    #[serde(rename = ".env")]
    DotEnv,
    Keyring,
}
impl KeySources {
    pub fn new(
        environment: Environment,
        root: &Path,
        keyring: Arc<KeyringLookup>,
    ) -> Result<Self, Error> {
        let path = root.join(".env");
        let mut dotenv = Vec::new();
        if path.exists() {
            for entry in dotenvy::from_path_iter(path).map_err(|e| error("config.dotenv", e))? {
                let (name, value) = entry.map_err(|e| error("config.dotenv", e))?;
                dotenv.push((name.into(), value.into()));
            }
        }
        Ok(Self {
            environment,
            dotenv: Environment(Secret::new(dotenv)),
            keyring,
        })
    }
    pub fn find(&self, variable: &str) -> Result<Option<Credential>, Error> {
        self.find_using(variable, &*self.keyring)
    }
    pub(crate) fn find_using(
        &self,
        variable: &str,
        lookup: &dyn Fn(&str) -> Result<String, KeyringError>,
    ) -> Result<Option<Credential>, Error> {
        for (environment, source) in [
            (&self.environment, Source::Environment),
            (&self.dotenv, Source::DotEnv),
        ] {
            if let Some((_, value)) = environment
                .0
                .expose()
                .iter()
                .find(|(name, _)| name == variable)
            {
                return credential(variable, value.to_string_lossy().into_owned(), source)
                    .map(Some);
            }
        }
        let value = lookup(variable).map_err(|e| keyring_error(variable, e))?;
        if value.is_empty() {
            return Ok(None);
        }
        credential(variable, value, Source::Keyring).map(Some)
    }
}
fn credential(variable: &str, value: String, source: Source) -> Result<Credential, Error> {
    if value.is_empty() {
        return Err(error("secrets.key_missing", variable));
    }
    if !value.is_ascii() || value.bytes().any(|b| b <= 32 || b == 127) {
        return Err(error(
            "secrets.key_malformed",
            format!("{variable} contains whitespace, controls or non-ASCII characters"),
        ));
    }
    Ok(Credential {
        value: Secret::new(value),
        source,
    })
}
pub(crate) fn keyring_error(variable: &str, e: KeyringError) -> Error {
    match e {
        KeyringError::Read(_) => error(
            "secrets.keyring_read",
            format!(
                "The platform keyring is unavailable for {variable}. Set {variable} in the environment or repository .env."
            ),
        ),
        KeyringError::Timeout => error(
            "secrets.keyring_timeout",
            format!(
                "OS keychain did not answer for {variable}; it may be waiting for approval. Approve access or set {variable} in the environment or .env."
            ),
        ),
    }
}
pub fn provider_keys() -> Vec<&'static str> {
    PROVIDERS.iter().filter_map(|p| p.key).collect()
}
pub fn doctor_key_rows(keys: &KeySources) -> Vec<Value> {
    provider_keys().into_iter().filter_map(|name| match keys.find(name) {
        Ok(Some(key)) => Some(json!({"name":name,"source":key.source})),
        Ok(None) => None,
        Err(Error::Failure {id,category,..}) => Some(json!({"name":name,"source":"keyring","error":{"id":id,"category":category,"action":"Unlock or repair the platform keyring, or set the variable in the environment or .env."}})),
    }).collect()
}
pub fn redact<'a>(text: &'a str, key: &str) -> Cow<'a, str> {
    if key.is_empty() || !text.contains(key) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.replace(key, "[redacted]"))
    }
}
