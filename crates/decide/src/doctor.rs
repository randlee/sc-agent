//! `decide doctor` (REQ-SAG-0503): detected keys in use order, provider models missing from
//! the catalog, and config models tying a built-in.

use sc_agent_config::AgentConfig;
use sc_agent_log::{GeneralLogger, Level, LogContext};
use sc_agent_secrets::{KeySource, ResolvedKey};
use sc_agent_types::{CancelToken, ErrorId, MODELS, ModelId, Provider};
use sc_llm_provider::{LlmClient, LlmProvider, catalog_model_id};
use serde_json::{Map, Value};

use crate::DecideError;
use crate::select::{
    Context, LOG_TARGET, detected_keys, flushed_result, probe, probe_client, probe_setup,
    typesafe_backend,
};

/// One detected provider key: the provider and where its key was found, never the value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DetectedKey {
    /// The provider.
    pub provider: Provider,
    /// `env`, `.env` or `keyring`.
    #[serde(serialize_with = "key_source_str")]
    pub source: KeySource,
}

fn key_source_str<S: serde::Serializer>(
    source: &KeySource,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(source.as_str())
}

/// A model a provider lists that neither the built-in list nor the configuration knows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MissingModel {
    /// The provider that lists it.
    pub provider: Provider,
    /// The id as the provider lists it.
    pub model: String,
}

/// A config model released on the same date as a built-in of its line (the config model wins).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Tie {
    /// The configured model.
    pub config: ModelId,
    /// The built-in model of the same line and date.
    pub builtin: ModelId,
}

/// What `decide doctor` prints.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DoctorReport {
    /// Every detected provider key, in `Provider::PRIORITY` order.
    pub keys: Vec<DetectedKey>,
    /// Provider models missing from the catalog; empty when none is.
    pub missing: Vec<MissingModel>,
    /// Config models tying a built-in; empty when none does.
    pub ties: Vec<Tie>,
}

/// Options of [`doctor`].
#[derive(Clone, Debug)]
pub struct DoctorOptions {
    /// The shared seams.
    pub context: Context,
}

/// Checks keys, provider model lists and config ties; writes one `decide.doctor` record.
///
/// Model lists are fetched with `Timeouts::probe()` (the context transport when set).
///
/// # Errors
///
/// `Config`, `Log`, `Secrets` (a key that is malformed or a source that fails), `Llm` and
/// `Decision` (a request or reply), `Http` (a send).
pub fn doctor(options: &DoctorOptions, cancel: &CancelToken) -> Result<DoctorReport, DecideError> {
    let (config, general, owned) = probe_setup(&options.context)?;
    let result = examine(&options.context, &config, &general, cancel);
    let count = |pick: fn(&DoctorReport) -> usize| {
        result
            .as_ref()
            .map_or(Value::Null, |report| pick(report).into())
    };
    let mut fields = Map::new();
    fields.insert("service".into(), options.context.service.into());
    fields.insert("keys".into(), count(|report| report.keys.len()));
    fields.insert("missing".into(), count(|report| report.missing.len()));
    fields.insert("ties".into(), count(|report| report.ties.len()));
    fields.insert(
        "outcome".into(),
        result
            .as_ref()
            .map_or_else(|error| error.id(), |_| "ok")
            .into(),
    );
    // A failed write is counted by sc-agent-log and reported by its flush.
    let _ = general.write(
        Level::Info,
        LOG_TARGET,
        "decide.doctor",
        LogContext::NONE,
        fields,
    );
    flushed_result(result, &general, owned)
}

fn examine(
    context: &Context,
    config: &AgentConfig,
    general: &GeneralLogger,
    cancel: &CancelToken,
) -> Result<DoctorReport, DecideError> {
    let catalog = config.catalog().map_err(DecideError::from)?;
    let found = detected_keys(context)?;
    let http = probe_client(context, config, general);
    let mut missing = Vec::new();
    for (provider, key) in &found {
        for model in provider_models(*provider, key.clone(), &http, general, cancel)? {
            if catalog.line_of(model.catalog.as_str()).is_none() {
                missing.push(MissingModel {
                    provider: *provider,
                    model: model.listed,
                });
            }
        }
    }
    Ok(DoctorReport {
        keys: found
            .iter()
            .map(|(provider, key)| DetectedKey {
                provider: *provider,
                source: key.source,
            })
            .collect(),
        missing,
        ties: ties(config),
    })
}

/// One listed model: as the provider lists it, and mapped back to a catalog id.
struct Listed {
    listed: String,
    catalog: String,
}

/// The model list of `provider`, each id mapped back with `catalog_model_id`.
fn provider_models(
    provider: Provider,
    key: ResolvedKey,
    http: &sc_agent_http::HttpClient,
    general: &GeneralLogger,
    cancel: &CancelToken,
) -> Result<Vec<Listed>, DecideError> {
    match LlmProvider::from_provider(provider) {
        None => {
            let backend = typesafe_backend(key)?;
            let request = backend.models_request().map_err(DecideError::from)?;
            let response = probe(http, &request, cancel)?;
            let list = backend.decode_models(response).map_err(DecideError::from)?;
            Ok(list
                .models
                .into_iter()
                .map(|model| Listed {
                    catalog: model.name.clone(),
                    listed: model.name,
                })
                .collect())
        }
        Some(llm) => {
            let client = LlmClient::new(llm, key.key)
                .map_err(DecideError::from)?
                .with_log(general.clone());
            let request = client.models_request().map_err(DecideError::from)?;
            let response = probe(http, &request, cancel)?;
            let ids = client.decode_models(response).map_err(DecideError::from)?;
            Ok(ids
                .into_iter()
                .map(|id| Listed {
                    catalog: catalog_model_id(llm, &id),
                    listed: id,
                })
                .collect())
        }
    }
}

/// One tie per config model whose release date equals a built-in model of its line.
fn ties(config: &AgentConfig) -> Vec<Tie> {
    let mut ties = Vec::new();
    for (model, extra) in &config.models {
        for row in MODELS {
            if row.line() == extra.line && row.released() == extra.released && row.model() != model
            {
                ties.push(Tie {
                    config: model.clone(),
                    builtin: row.model().clone(),
                });
            }
        }
    }
    ties
}
