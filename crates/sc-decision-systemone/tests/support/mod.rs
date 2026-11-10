//! Shared test helpers: real `ApiKey` values and request builders.

use sc_agent_secrets::{
    ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, TYPESAFE_API_KEY, resolve_var,
};
use sc_decision_schema::{DecisionRequest, ImageFormat, ImagePart, SystemOneRequest};

/// The key text tests use; asserted absent from every Debug output.
pub const SECRET: &str = "sk-test-systemone-0123456789";

struct OneVar;

impl EnvSource for OneVar {
    fn var(&self, name: &str) -> Option<String> {
        (name == TYPESAFE_API_KEY).then(|| SECRET.to_owned())
    }
}

struct NoKeyring;

impl KeyringSource for NoKeyring {
    fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

/// `SECRET` as an `ApiKey`, resolved from `TYPESAFE_API_KEY` as sc-agent-secrets resolves keys.
pub fn key() -> ApiKey {
    let var = TYPESAFE_API_KEY.parse().expect("var name");
    let sources = KeySources {
        env: &OneVar,
        repo_root: None,
        keyring: &NoKeyring,
    };
    resolve_var(&var, &sources).expect("test key").key
}

/// A request of `json` with no image parts.
pub fn request(json: &str, long_context: bool) -> DecisionRequest {
    let body = SystemOneRequest::from_json(json).expect("request json");
    DecisionRequest::new(body, Vec::new(), long_context).expect("valid request")
}

/// The one place tests build an `ImagePart` (a-2 adds `ImagePart::new`).
pub fn image_part(name: &str, format: ImageFormat, bytes: Vec<u8>) -> ImagePart {
    ImagePart {
        name: name.to_owned(),
        format,
        bytes,
    }
}

/// The value of header `name` (ASCII case-insensitive) in `headers`.
pub fn header<'h>(headers: &'h [(String, String)], name: &str) -> Option<&'h str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
