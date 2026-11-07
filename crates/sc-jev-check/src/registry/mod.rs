//! Backend registry (ADR-SAG-0009, ADR-SAG-0024).
//!
//! Callers select backends with a [`BackendSet`] on
//! [`crate::run::RunConfig::backends`]. [`BackendRegistry::from_env`] is the
//! production registry: it resolves each backend's key with
//! [`sc_agent_secrets::resolve_key`] (the only key read site, ADR-SAG-0003,
//! ADR-SAG-0020) and constructs the Jev, Haiku, and Luna clients, which
//! receive the resolved secret at construction. [`BackendRegistry::from_resolver`]
//! builds the same backends from caller-resolved keys. Embedders that need
//! no secrets inject a registry through [`crate::run::RunConfig::registry`]
//! (EXT-009). [`BackendRegistry::register`] installs a double or scripted
//! backend for an existing [`BackendId`]; it cannot add an id.
//!
//! The production factory is data, not a `match`: the private `FACTORIES`
//! table has one row per built-in backend, each calling that backend's
//! `with_default_transport` (ADR-SAG-0009/0027 r-1). It is the only id
//! dispatch. Adding a built-in backend adds one row;
//! `tests::test_every_backend_has_factory` checks the table against
//! [`sc_agent_types::BACKENDS`].
//!
//! The `test-util` feature exposes `register_scripted_backends` for tests
//! (ADR-SAG-0022). Production builds leave that feature off. It adds no
//! `RunConfig` setter: the scripted registry goes through
//! [`crate::run::RunConfig::registry`].

#![warn(missing_docs)]

use std::sync::Arc;

#[cfg(any(test, feature = "test-util"))]
mod scripted;

use sc_agent_secrets::resolve_key;
use sc_agent_types::{
    BackendId, BackendKeyId, BackendSet, JevError, JudgeBackend, RepoRoot, ResolvedApiKey,
};
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
/// The registry is a list of trait objects, at most one per [`BackendId`].
/// Registering a double does not change any run API (ADR-SAG-0009).
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

    /// Production registry: resolves each key at `repo_root` and constructs
    /// one backend per `backends` entry.
    ///
    /// Each key comes from [`resolve_key`]`(id.key_id(), repo_root)`
    /// (process env, then the repo-root `.env`, then the keyring;
    /// ADR-SAG-0003). The key is not retained in the registry's `Debug`
    /// output. No judge request is sent.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when a credential is absent,
    /// `CONFIG_INVALID` when a key is rejected, and
    /// [`JevError::RegistryInvalid`] when an id has no factory row or no
    /// credential.
    pub fn from_env(repo_root: &RepoRoot, backends: &BackendSet) -> Result<Self, JevError> {
        Self::from_resolver(backends, |key_id| resolve_key(key_id, repo_root))
    }

    /// Constructs one production backend per `backends` entry from keys
    /// returned by `resolve`.
    ///
    /// `resolve` is called once per backend with that backend's
    /// [`BackendKeyId`]. Use it to supply keys from a source other than the
    /// environment; [`Self::from_env`] is this function over
    /// [`resolve_key`].
    ///
    /// # Errors
    ///
    /// Returns `resolve`'s error, or [`JevError::RegistryInvalid`] when an
    /// id has no factory row or no credential.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{BackendId, BackendSet, ResolvedApiKey};
    /// use sc_jev_check::registry::BackendRegistry;
    ///
    /// let backends: BackendSet = "jev,anthropic_haiku".parse()?;
    /// let registry = BackendRegistry::from_resolver(&backends, |key_id| {
    ///     ResolvedApiKey::try_new(key_id, "example-key".to_owned())
    /// })?;
    /// assert!(registry.contains(BackendId::AnthropicHaiku));
    /// # Ok::<(), sc_agent_types::JevError>(())
    /// ```
    pub fn from_resolver(
        backends: &BackendSet,
        resolve: impl Fn(BackendKeyId) -> Result<ResolvedApiKey, JevError>,
    ) -> Result<Self, JevError> {
        let mut registry = Self::new();
        for id in backends.iter() {
            let factory = factory_for(id)?;
            let key_id = id.key_id().ok_or_else(|| {
                JevError::registry_invalid(
                    format!("Backend {id} needs no credential but has a credential factory"),
                    "Report this build as a bug.",
                )
            })?;
            registry.register(factory(resolve(key_id)?))?;
        }
        Ok(registry)
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
    fn resolver_builds_backends_in_set_order_and_no_send() {
        let seen = std::sync::Mutex::new(Vec::new());
        let backends: BackendSet = "jev,anthropic_haiku".parse().expect("set");
        let registry = BackendRegistry::from_resolver(&backends, |id| {
            seen.lock().expect("seen").push(id);
            ResolvedApiKey::try_new(id, "test-key".to_owned())
        })
        .expect("registry");
        assert_eq!(
            *seen.lock().expect("seen"),
            [BackendKeyId::TypeSafe, BackendKeyId::Anthropic]
        );
        assert!(registry.contains(BackendId::Jev));
        assert!(registry.contains(BackendId::AnthropicHaiku));
        let rendered = format!("{registry:?}");
        assert!(!rendered.contains("test-key"));
    }
}
