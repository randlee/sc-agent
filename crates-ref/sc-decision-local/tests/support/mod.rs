//! Shared test helpers: real `ApiKey` values, runner hosts, backends and canned replies.

use std::num::NonZeroU16;

use sc_agent_config::RunnerHost;
use sc_agent_http::HttpResponse;
use sc_agent_secrets::{
    ApiKey, EnvSource, KeySources, KeyringSource, LAYA_API_KEY, SecretsError, resolve_var,
};
use sc_agent_types::ModelId;
use sc_decision_local::{DaemonKind, LocalBackend};
use sc_decision_schema::{DecisionRequest, SystemOneRequest};

/// The key text tests use; asserted absent from every Debug output and log record.
pub const SECRET: &str = "sk-test-local-0123456789";

/// The configured model of every test runner.
pub const MODEL: &str = "jev-latest";

struct OneVar;

impl EnvSource for OneVar {
    fn var(&self, name: &str) -> Option<String> {
        (name == LAYA_API_KEY).then(|| SECRET.to_owned())
    }
}

struct NoKeyring;

impl KeyringSource for NoKeyring {
    fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

/// `SECRET` as an `ApiKey`, resolved from `LAYA_API_KEY` as sc-agent-secrets resolves keys.
pub fn key() -> ApiKey {
    let var = LAYA_API_KEY.parse().expect("var name");
    let sources = KeySources {
        env: &OneVar,
        repo_root: None,
        keyring: &NoKeyring,
    };
    resolve_var(&var, &sources).expect("test key").key
}

pub fn model() -> ModelId {
    MODEL.parse().expect("model id")
}

pub fn host(text: &str) -> RunnerHost {
    RunnerHost::try_from(text.to_owned()).expect("runner host")
}

pub fn port(number: u16) -> NonZeroU16 {
    NonZeroU16::new(number).expect("non-zero port")
}

/// A backend of `kind` at 127.0.0.1:8000 serving `MODEL`.
pub fn loopback(kind: DaemonKind, key: Option<ApiKey>) -> LocalBackend {
    LocalBackend::new(kind, &host("127.0.0.1"), port(8000), model(), key).expect("backend")
}

pub fn json(status: u16, body: &str) -> HttpResponse {
    HttpResponse::new(
        status,
        vec![("content-type".to_owned(), "application/json".to_owned())],
        body.as_bytes().to_vec(),
    )
}

pub fn decision_request() -> DecisionRequest {
    let body = SystemOneRequest::from_json(
        r#"{"model":"jev-latest","state":"I was charged twice.","questions":{"billing":{"type":"noul"}}}"#,
    )
    .expect("json");
    DecisionRequest::try_from(body).expect("valid")
}

/// A 2xx System One reply.
pub fn decided() -> HttpResponse {
    json(
        200,
        r#"{"model":"jev-latest","answers":{"billing":{"type":"noul","noul":0.123456789}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
    )
}
