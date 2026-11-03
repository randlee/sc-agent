//! Backend registry (ADR-SAG-0009).
//!
//! Callers pass [`BackendId`] values to [`crate::run::run_check`]. A new
//! [`JudgeBackend`] is added by [`BackendRegistry::register`]. That does not
//! change the `run_check` signature. Production wiring resolves API keys with
//! [`sc_agent_secrets::resolve_key`] and constructs the Jev, Haiku, and Luna
//! clients. Those clients receive the resolved secret at construction.
//!
//! The `test-util` feature exposes `register_scripted_backends` for tests.
//! Production builds leave that feature off. It does not add a `run_check`
//! parameter.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::Arc;

#[cfg(any(test, feature = "test-util"))]
mod scripted;

use sc_agent_secrets::resolve_key;
use sc_agent_types::{BackendId, BackendKeyId, JevError, JudgeBackend, RepoRoot, ResolvedApiKey};
use sc_jev::{HttpsTransport, JevClient};
use sc_jev_backends::{AnthropicHaikuBackend, OpenAiLunaBackend, UreqTransport};

#[cfg(any(test, feature = "test-util"))]
#[doc(inline)]
pub use scripted::register_scripted_backends;

/// Selected judge backends for one run.
///
/// The registry is a list of trait objects. Registering a local judge does
/// not change [`crate::run::run_check`]'s parameters.
#[derive(Clone, Default)]
pub struct BackendRegistry {
    backends: Vec<Arc<dyn JudgeBackend>>,
}

impl BackendRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// Registers `backend`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::registry_invalid`] when another backend with the same
    /// [`JudgeBackend::id`] is already registered. The code is `CONFIG_INVALID`.
    /// The fix hint names that id and does not mention `check.toml`.
    pub fn register(&mut self, backend: Arc<dyn JudgeBackend>) -> Result<(), JevError> {
        let id = backend.id();
        if self.backends.iter().any(|existing| existing.id() == id) {
            return Err(JevError::registry_invalid(&format!(
                "Backend {id} is already registered. Register each backend id once."
            )));
        }
        self.backends.push(backend);
        Ok(())
    }

    /// Returns the backend registered for `id`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::registry_invalid`] when `id` was not registered.
    /// The code is `CONFIG_INVALID`. The fix hint names `id` and does not
    /// mention `check.toml`.
    pub fn get(&self, id: BackendId) -> Result<Arc<dyn JudgeBackend>, JevError> {
        self.backends
            .iter()
            .find(|backend| backend.id() == id)
            .cloned()
            .ok_or_else(|| {
                JevError::registry_invalid(&format!(
                    "Backend {id} is not registered. Register it before the run or remove it from the requested backends."
                ))
            })
    }

    /// Reports whether `id` is registered.
    #[must_use]
    pub fn contains(&self, id: BackendId) -> bool {
        self.backends.iter().any(|backend| backend.id() == id)
    }
}

impl std::fmt::Debug for BackendRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<BackendId> = self.backends.iter().map(|backend| backend.id()).collect();
        formatter
            .debug_struct("BackendRegistry")
            .field("backends", &ids)
            .finish()
    }
}

/// Credential id used to construct `id`.
#[must_use]
pub fn backend_key_id(id: BackendId) -> BackendKeyId {
    match id {
        BackendId::Jev => BackendKeyId::TypeSafe,
        BackendId::AnthropicHaiku => BackendKeyId::Anthropic,
        BackendId::OpenAiLuna => BackendKeyId::OpenAi,
    }
}

/// Resolves keys at `repo` and constructs one backend per `ids` entry.
///
/// This is the production path [`crate::run::run_check`] uses when the caller
/// does not inject a registry. Each backend is built with
/// [`resolve_key`]`(backend_key_id(id), repo)` and the crate's HTTPS transport.
/// The key is not retained in the registry's `Debug` output.
///
/// # Errors
///
/// Returns [`JevError::KeyMissing`] when a credential is absent,
/// [`JevError::registry_invalid`] when a duplicate id is rejected,
/// [`JevError::ConfigInvalid`] when a key is rejected, and the transport
/// constructors' errors. No judge request is sent.
pub fn registry_for_repo(repo: &RepoRoot, ids: &[BackendId]) -> Result<BackendRegistry, JevError> {
    registry_from_resolver(repo, ids, &mut |id, root| resolve_key(id, root))
}

pub(crate) fn registry_from_resolver(
    repo: &RepoRoot,
    ids: &[BackendId],
    resolve: &mut dyn FnMut(BackendKeyId, &RepoRoot) -> Result<ResolvedApiKey, JevError>,
) -> Result<BackendRegistry, JevError> {
    let mut registry = BackendRegistry::new();
    for id in ids {
        let key = resolve(backend_key_id(*id), repo)?;
        registry.register(construct_backend(*id, key)?)?;
    }
    Ok(registry)
}

fn construct_backend(
    id: BackendId,
    key: ResolvedApiKey,
) -> Result<Arc<dyn JudgeBackend>, JevError> {
    match id {
        BackendId::Jev => {
            let client = JevClient::new(key, Arc::new(HttpsTransport::new()))?;
            Ok(Arc::new(client))
        }
        BackendId::AnthropicHaiku => Ok(Arc::new(AnthropicHaikuBackend::new(
            key,
            UreqTransport::new(),
        )?)),
        BackendId::OpenAiLuna => Ok(Arc::new(OpenAiLunaBackend::new(key, UreqTransport::new())?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::{JudgeOutcome, JudgeRequest, test_util::MockJudgeBackend};
    use std::sync::Mutex;

    struct LocalJudge {
        seen: Mutex<u32>,
    }

    impl JudgeBackend for LocalJudge {
        fn id(&self) -> BackendId {
            BackendId::Jev
        }

        fn validate_for_send(&self, _req: &JudgeRequest) -> Result<(), JevError> {
            Ok(())
        }

        fn judge(&self, _req: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
            *self.seen.lock().unwrap_or_else(|error| error.into_inner()) += 1;
            Ok(JudgeOutcome {
                answers: serde_json::json!({"relevant": {"choice": "yes"}}),
                backend: BackendId::Jev,
                probabilities: None,
                provider_meta: None,
            })
        }
    }

    #[test]
    fn registry_accepts_a_new_judge_backend_without_an_api_change() {
        let mut registry = BackendRegistry::new();
        registry
            .register(Arc::new(LocalJudge {
                seen: Mutex::new(0),
            }))
            .expect("register");
        let error = registry
            .register(Arc::new(MockJudgeBackend::new(
                BackendId::Jev,
                JudgeOutcome {
                    answers: serde_json::json!({}),
                    backend: BackendId::Jev,
                    probabilities: None,
                    provider_meta: None,
                },
            )))
            .expect_err("duplicate");
        assert_eq!(error.code(), "CONFIG_INVALID");
        let duplicate_hint = error.fix_hint();
        assert!(
            duplicate_hint.contains("jev"),
            "duplicate hint names the backend: {duplicate_hint}"
        );
        assert!(
            !duplicate_hint.contains("check.toml"),
            "duplicate hint is not a check.toml hint: {duplicate_hint}"
        );
        let backend = registry.get(BackendId::Jev).expect("jev");
        let missing = match registry.get(BackendId::OpenAiLuna) {
            Ok(_) => panic!("openai_luna must be absent"),
            Err(error) => error,
        };
        assert_eq!(missing.code(), "CONFIG_INVALID");
        let missing_hint = missing.fix_hint();
        assert!(
            missing_hint.contains("openai_luna"),
            "missing hint names the backend: {missing_hint}"
        );
        assert!(
            !missing_hint.contains("check.toml"),
            "missing hint is not a check.toml hint: {missing_hint}"
        );
        let _ = backend;
    }

    #[test]
    fn resolver_builds_backends_with_the_repo_root_and_no_send() {
        let dir = crate::extract::test_support::temp_dir("registry-root");
        let repo = RepoRoot::new(dir.path());
        let mut seen = Vec::new();
        let registry = registry_from_resolver(
            &repo,
            &[BackendId::Jev, BackendId::AnthropicHaiku],
            &mut |id, root| {
                assert_eq!(root.as_path(), repo.as_path());
                seen.push(id);
                Ok(ResolvedApiKey::new("test-key".to_owned()))
            },
        )
        .expect("registry");
        assert_eq!(seen, [BackendKeyId::TypeSafe, BackendKeyId::Anthropic]);
        assert!(registry.contains(BackendId::Jev));
        assert!(registry.contains(BackendId::AnthropicHaiku));
        let rendered = format!("{registry:?}");
        assert!(!rendered.contains("test-key"));
    }
}
