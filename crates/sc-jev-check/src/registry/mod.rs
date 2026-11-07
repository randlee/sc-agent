//! Backend registry (ADR-SAG-0009).
//!
//! Callers pass [`BackendId`] values to [`crate::run::run_check`]. A new
//! [`JudgeBackend`] is added by [`BackendRegistry::register`]. That does not
//! change the `run_check` signature. Production wiring resolves API keys with
//! [`sc_agent_secrets::resolve_key`] and constructs the Jev, Haiku, and Luna
//! clients. Those clients receive the resolved secret at construction.
//!
//! The production factory is data, not a `match`: the private `FACTORIES`
//! table has one row per built-in backend, each calling that backend's
//! `with_default_transport` (ADR-SAG-0009/0027 r-1). Adding a built-in backend
//! adds one row; `tests::test_every_backend_has_factory` checks the table
//! against [`sc_agent_types::BACKENDS`].
//!
//! The `test-util` feature exposes `register_scripted_backends` for tests
//! (ADR-SAG-0022). Production builds leave that feature off. It does not add
//! a `run_check` parameter (ADR-SAG-0009).

#![warn(missing_docs)]

use std::sync::Arc;

#[cfg(any(test, feature = "test-util"))]
mod scripted;

use sc_agent_secrets::resolve_key;
use sc_agent_types::{BackendId, BackendKeyId, JevError, JudgeBackend, RepoRoot, ResolvedApiKey};
use sc_jev::JevClient;
use sc_jev_backends::{AnthropicHaikuBackend, OpenAiLunaBackend};

#[cfg(any(test, feature = "test-util"))]
#[doc(inline)]
pub use scripted::register_scripted_backends;

/// Builds one production backend from its resolved key.
type Factory = fn(ResolvedApiKey) -> Arc<dyn JudgeBackend>;

/// One row per built-in backend (test-and-review convention, ADR-SAG-0027).
static FACTORIES: &[(BackendId, Factory)] = &[
    (BackendId::Jev, jev_factory),
    (BackendId::AnthropicHaiku, haiku_factory),
    (BackendId::OpenAiLuna, luna_factory),
];

fn jev_factory(key: ResolvedApiKey) -> Arc<dyn JudgeBackend> {
    Arc::new(JevClient::with_default_transport(key))
}

fn haiku_factory(key: ResolvedApiKey) -> Arc<dyn JudgeBackend> {
    Arc::new(AnthropicHaikuBackend::with_default_transport(key))
}

fn luna_factory(key: ResolvedApiKey) -> Arc<dyn JudgeBackend> {
    Arc::new(OpenAiLunaBackend::with_default_transport(key))
}

fn factory_for(id: BackendId) -> Result<Factory, JevError> {
    FACTORIES
        .iter()
        .find(|(row, _)| *row == id)
        .map(|(_, factory)| *factory)
        .ok_or_else(|| {
            JevError::registry_invalid(
                format!("Backend {id} has no production factory"),
                "Use a build of sc-agent that includes this backend.",
            )
        })
}

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
            return Err(JevError::registry_invalid(
                format!("Backend {id} is already registered"),
                "Register each backend id once.",
            ));
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
                JevError::registry_invalid(
                    format!("Backend {id} is not registered"),
                    "Register it before the run or remove it from the requested backends.",
                )
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

/// Resolves keys at `repo` and constructs one backend per `ids` entry.
///
/// This is the production path [`crate::run::run_check`] uses when the caller
/// does not inject a registry. Each backend is built with
/// [`resolve_key`]`(id.key_id(), repo)` and its `FACTORIES` row.
/// The key is not retained in the registry's `Debug` output.
///
/// # Errors
///
/// Returns [`JevError::KeyMissing`] when a credential is absent,
/// [`JevError::RegistryInvalid`] when a duplicate id is rejected or an id has
/// no factory row or credential, and `CONFIG_INVALID` when a key is
/// rejected. No judge request is sent.
pub fn registry_for_repo(repo: &RepoRoot, ids: &[BackendId]) -> Result<BackendRegistry, JevError> {
    registry_from_resolver(repo, ids, &mut |id, root| resolve_key(id, root))
}

pub(crate) fn registry_from_resolver(
    repo: &RepoRoot,
    ids: &[BackendId],
    resolve: &mut dyn FnMut(BackendKeyId, &RepoRoot) -> Result<ResolvedApiKey, JevError>,
) -> Result<BackendRegistry, JevError> {
    let mut registry = BackendRegistry::new();
    for id in ids.iter().copied() {
        let factory = factory_for(id)?;
        let key_id = id.key_id().ok_or_else(|| {
            JevError::registry_invalid(
                format!("Backend {id} needs no credential but has a credential factory"),
                "Report this build as a bug.",
            )
        })?;
        let key = resolve(key_id, repo)?;
        registry.register(factory(key))?;
    }
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use sc_agent_types::BACKENDS;
    use sc_agent_types::test_util::MockJudgeBackend;

    /// Every descriptor has exactly one factory row and every row has a
    /// descriptor (REQ-SAG-0105).
    #[test]
    fn test_every_backend_has_factory() {
        let mut rows = BTreeSet::new();
        for (id, _) in FACTORIES {
            assert!(
                rows.insert(*id),
                "backend {id} appears in two FACTORIES rows"
            );
            assert!(
                BACKENDS.iter().any(|row| row.id() == *id),
                "FACTORIES row {id} has no BACKENDS descriptor"
            );
        }
        for row in BACKENDS {
            assert!(
                rows.contains(&row.id()),
                "descriptor {} has no FACTORIES row",
                row.name()
            );
            let key = ResolvedApiKey::try_new(
                row.key_id().expect("built-in backends send a key"),
                "test-key".to_owned(),
            )
            .expect("key");
            assert_eq!(factory_for(row.id()).expect("factory")(key).id(), row.id());
        }
    }

    #[test]
    fn registry_accepts_a_new_judge_backend_without_an_api_change() {
        let mut registry = BackendRegistry::new();
        registry
            .register(Arc::new(MockJudgeBackend::new(
                BackendId::Jev,
                BTreeMap::new(),
            )))
            .expect("register");
        let error = registry
            .register(Arc::new(MockJudgeBackend::new(
                BackendId::Jev,
                BTreeMap::new(),
            )))
            .expect_err("duplicate");
        assert_eq!(error.code(), "CONFIG_INVALID");
        let duplicate_hint = error.fix_hint();
        assert!(duplicate_hint.contains("jev"), "{duplicate_hint}");
        assert!(!duplicate_hint.contains("check.toml"), "{duplicate_hint}");
        assert!(registry.get(BackendId::Jev).is_ok());
        let missing = match registry.get(BackendId::OpenAiLuna) {
            Ok(_) => panic!("openai_luna must be absent"),
            Err(error) => error,
        };
        assert_eq!(missing.code(), "CONFIG_INVALID");
        let missing_hint = missing.fix_hint();
        assert!(missing_hint.contains("openai_luna"), "{missing_hint}");
        assert!(!missing_hint.contains("check.toml"), "{missing_hint}");
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
                ResolvedApiKey::try_new(id, "test-key".to_owned())
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
