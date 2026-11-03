//! File-backed judge sessions (REQ-SAG-0600, ADR-SAG-0008).
//!
//! Each session is a UUID file with mode `0600`. Expiry is `created_at` plus
//! the store TTL. Touching the file does not extend the session. Haiku and
//! Luna `ask` calls reuse the stored payload, system prompt, and prior
//! answers. Jev has no rationale field, so `ask` refuses a Jev session.
//!
//! [`SessionStore`] is an open extension point. [`FileSessionStore`] is the
//! Phase A implementation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sc_agent_types::observability::v2::{LogEvent, SharedSessionLog};
use sc_agent_types::{
    BackendId, JevError, JudgeOutcome, JudgeRequest, LlmWireContext, PayloadBundle, SessionId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::registry::BackendRegistry;

/// Default lifetime measured from `created_at`.
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Header inserted before prior answers in an LLM system prompt.
pub const SESSION_CONTEXT_HEADER: &str = "Session context:";

/// Largest session file this store will read.
const MAX_SESSION_BYTES: u64 = 8 * 1024 * 1024;

const SESSION_VERSION: u32 = 1;

/// Creates a session and continues an LLM session with `ask`.
///
/// The trait is open so callers can supply their own store. Adding a store
/// does not change [`crate::run::run_check`]'s signature.
pub trait SessionStore: Send + Sync {
    /// Persists `bundle` for `backend` and returns the new session id.
    ///
    /// # Errors
    ///
    /// Returns [`JevError`] when the session cannot be stored.
    fn create(&self, backend: BackendId, bundle: &PayloadBundle) -> Result<SessionId, JevError>;

    /// Sends `follow_up` to the session's Haiku or Luna backend.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::SessionNotFound`] when `id` is absent,
    /// [`JevError::SessionExpired`] when `created_at` plus the TTL is in the
    /// past, and [`JevError::InputInvalid`] when the session is Jev or the
    /// follow-up is empty.
    fn ask(&self, id: &SessionId, follow_up: &str) -> Result<JudgeOutcome, JevError>;
}

/// One session loaded from disk.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSession {
    /// File id.
    pub session_id: SessionId,
    /// Backend `ask` will call.
    pub backend: BackendId,
    /// Creation instant. Expiry is measured from this value.
    pub created_at: SystemTime,
    /// Canonical payload bytes and their digest.
    pub payload: PayloadBundle,
    /// Base LLM system prompt. `None` for a Jev-only session.
    pub system_prompt: Option<String>,
    /// Prior judge outcomes, in call order.
    pub outcomes: Vec<JudgeOutcome>,
    /// Follow-up texts already sent with `ask`.
    pub follow_ups: Vec<String>,
}

/// Sessions stored as UUID files under a directory.
///
/// Clone shares the registry, clock handle, log, and file lock. `with_ttl` and
/// `with_clock` replace those settings on the returned value only. Call them
/// before cloning the store into [`crate::run::RunOptions`].
#[derive(Clone)]
pub struct FileSessionStore {
    dir: PathBuf,
    ttl: Duration,
    clock: Arc<dyn Fn() -> SystemTime + Send + Sync>,
    registry: Arc<RwLock<BackendRegistry>>,
    session_log: SharedSessionLog,
    io: Arc<Mutex<()>>,
}

impl FileSessionStore {
    /// Creates a store rooted at `dir`, creating the directory when needed.
    ///
    /// The directory's existing mode is left unchanged. Session files created
    /// later are mode `0600`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when `dir` cannot be created.
    pub fn new(dir: impl Into<PathBuf>, session_log: SharedSessionLog) -> Result<Self, JevError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|error| JevError::ConfigInvalid {
            detail: format!("session directory {}: {error}", dir.display()),
        })?;
        Ok(Self {
            dir,
            ttl: DEFAULT_SESSION_TTL,
            clock: Arc::new(SystemTime::now),
            registry: Arc::new(RwLock::new(BackendRegistry::new())),
            session_log,
            io: Arc::new(Mutex::new(())),
        })
    }

    /// Sets the lifetime measured from `created_at`.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Replaces the clock used for `created_at` and expiry.
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> SystemTime + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// Installs `registry` for later `ask` calls.
    ///
    /// Clones of this store share the registry.
    pub fn install_registry(&self, registry: BackendRegistry) {
        *self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = registry;
    }

    /// Directory that holds session files.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Path of the UUID file for `id`.
    #[must_use]
    pub fn path(&self, id: &SessionId) -> PathBuf {
        self.dir.join(id.to_string())
    }

    /// Stores `bundle` and an optional base system prompt.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::InputInvalid`] when the payload is not UTF-8, and
    /// [`JevError::ConfigInvalid`] when the file cannot be written.
    pub fn open(
        &self,
        backend: BackendId,
        bundle: &PayloadBundle,
        system_prompt: Option<String>,
    ) -> Result<SessionId, JevError> {
        let session_id = SessionId::new();
        let created_at = unix_ms((self.clock)())?;
        let document = SessionDocument {
            version: SESSION_VERSION,
            session_id,
            backend,
            created_at,
            payload_utf8: utf8_payload(&bundle.bytes)?,
            prompt_sha256: bundle.sha256.clone(),
            system_prompt,
            outcomes: Vec::new(),
            follow_ups: Vec::new(),
        };
        let _guard = self
            .io
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.write_document(&document)?;
        Ok(session_id)
    }

    /// Loads `id`, including expired sessions.
    ///
    /// Callers that must refuse expired sessions use [`Self::load`].
    ///
    /// # Errors
    ///
    /// Returns [`JevError::SessionNotFound`] when the file is absent and
    /// [`JevError::InputInvalid`] when the file is not a session document.
    pub fn load_raw(&self, id: &SessionId) -> Result<StoredSession, JevError> {
        let _guard = self
            .io
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.read_document(id).map(StoredSession::from_document)
    }

    /// Loads `id` when `created_at` plus the TTL is still in the future.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::SessionNotFound`] when the file is absent,
    /// [`JevError::SessionExpired`] when `created_at` is past the TTL, and
    /// [`JevError::InputInvalid`] when the file is not a session document.
    /// File modification time is not consulted.
    pub fn load(&self, id: &SessionId) -> Result<StoredSession, JevError> {
        let stored = self.load_raw(id)?;
        self.ensure_fresh(id, &stored)?;
        Ok(stored)
    }

    /// Appends `outcome` to the session file.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::SessionNotFound`] when `id` is absent and
    /// [`JevError::SessionExpired`] when the session is past its TTL.
    pub fn record_outcome(&self, id: &SessionId, outcome: &JudgeOutcome) -> Result<(), JevError> {
        let _guard = self
            .io
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut document = self.read_document(id)?;
        self.ensure_fresh_document(id, &document)?;
        document.outcomes.push(outcome.clone());
        self.write_document(&document)
    }

    fn ensure_fresh(&self, id: &SessionId, stored: &StoredSession) -> Result<(), JevError> {
        let created_at = unix_ms(stored.created_at)?;
        if self.is_expired(created_at)? {
            Err(JevError::SessionExpired { session_id: *id })
        } else {
            Ok(())
        }
    }

    fn ensure_fresh_document(
        &self,
        id: &SessionId,
        document: &SessionDocument,
    ) -> Result<(), JevError> {
        if self.is_expired(document.created_at)? {
            Err(JevError::SessionExpired { session_id: *id })
        } else {
            Ok(())
        }
    }

    fn is_expired(&self, created_at: u64) -> Result<bool, JevError> {
        let now = unix_ms((self.clock)())?;
        let ttl_ms = u64::try_from(self.ttl.as_millis()).unwrap_or(u64::MAX);
        Ok(now >= created_at.saturating_add(ttl_ms))
    }

    fn read_document(&self, id: &SessionId) -> Result<SessionDocument, JevError> {
        let path = self.path(id);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(JevError::SessionNotFound { session_id: *id });
            }
            Err(error) => {
                return Err(JevError::ConfigInvalid {
                    detail: format!("session {id}: {error}"),
                });
            }
        };
        let mut bytes = Vec::new();
        file.take(MAX_SESSION_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| JevError::ConfigInvalid {
                detail: format!("session {id}: {error}"),
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SESSION_BYTES {
            return Err(JevError::InputInvalid {
                detail: format!("session {id} exceeds {MAX_SESSION_BYTES} bytes"),
            });
        }
        let document: SessionDocument =
            serde_json::from_slice(&bytes).map_err(|error| JevError::InputInvalid {
                detail: format!("session {id} is invalid: {error}"),
            })?;
        if document.version != SESSION_VERSION || document.session_id != *id {
            return Err(JevError::InputInvalid {
                detail: format!("session {id} is invalid"),
            });
        }
        let payload = utf8_payload(document.payload_utf8.as_bytes())?;
        if sc_agent_types::Sha256Digest::of(payload.as_bytes()) != document.prompt_sha256 {
            return Err(JevError::InputInvalid {
                detail: format!("session {id} payload digest does not match"),
            });
        }
        Ok(document)
    }

    fn write_document(&self, document: &SessionDocument) -> Result<(), JevError> {
        let bytes = serde_json::to_vec(document).map_err(|error| JevError::InputInvalid {
            detail: format!("session encoding failed: {error}"),
        })?;
        let path = self.path(&document.session_id);
        write_private(&path, &bytes).map_err(|error| JevError::ConfigInvalid {
            detail: format!("session {}: {error}", document.session_id),
        })
    }

    fn backend(&self, id: BackendId) -> Result<Arc<dyn sc_agent_types::JudgeBackend>, JevError> {
        self.registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
    }
}

impl SessionStore for FileSessionStore {
    fn create(&self, backend: BackendId, bundle: &PayloadBundle) -> Result<SessionId, JevError> {
        self.open(backend, bundle, None)
    }

    fn ask(&self, id: &SessionId, follow_up: &str) -> Result<JudgeOutcome, JevError> {
        if follow_up.trim().is_empty() {
            return Err(JevError::InputInvalid {
                detail: "follow-up text is empty".into(),
            });
        }
        let stored = self.load(id)?;
        if !is_llm(stored.backend) {
            return Err(JevError::InputInvalid {
                detail: "Jev sessions have no rationale field; ask a Haiku or Luna session".into(),
            });
        }
        let backend = self.backend(stored.backend)?;
        let mut prompt = stored.system_prompt.clone().unwrap_or_default();
        prompt.push_str(&render_session_context(&stored.outcomes)?);
        for prior in &stored.follow_ups {
            prompt.push_str("\n\nFollow-up:\n");
            prompt.push_str(prior);
        }
        prompt.push_str("\n\nFollow-up:\n");
        prompt.push_str(follow_up);
        let request = JudgeRequest {
            payload: stored.payload.clone(),
            llm: Some(LlmWireContext {
                system_prompt: prompt,
            }),
        };
        backend.validate_for_send(&request)?;
        let judged = backend.judge(&request);
        emit_log(
            &self.session_log,
            LogEvent {
                session_id: *id,
                backend: stored.backend,
                prompt_sha256: stored.payload.sha256.clone(),
                operation: "ask".to_owned(),
            },
        );
        let outcome = judged?;
        let _guard = self
            .io
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut document = self.read_document(id)?;
        self.ensure_fresh_document(id, &document)?;
        document.follow_ups.push(follow_up.to_owned());
        document.outcomes.push(outcome.clone());
        self.write_document(&document)?;
        Ok(outcome)
    }
}

impl std::fmt::Debug for FileSessionStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileSessionStore")
            .field("dir", &self.dir)
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

/// Renders prior answers for an LLM system prompt.
///
/// The canonical payload bytes are not modified. An empty `outcomes` slice
/// renders as an empty string.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the answers cannot be encoded.
pub fn render_session_context(outcomes: &[JudgeOutcome]) -> Result<String, JevError> {
    if outcomes.is_empty() {
        return Ok(String::new());
    }
    let prior: Vec<Prior<'_>> = outcomes
        .iter()
        .map(|outcome| Prior {
            backend: outcome.backend,
            answers: &outcome.answers,
        })
        .collect();
    let json = serde_json::to_string(&prior).map_err(|error| JevError::InputInvalid {
        detail: format!("session context encoding failed: {error}"),
    })?;
    Ok(format!("\n\n{SESSION_CONTEXT_HEADER}\n{json}"))
}

pub(crate) fn emit_log(log: &SharedSessionLog, event: LogEvent) {
    let _ = log.try_log(event);
}

pub(crate) fn is_llm(id: BackendId) -> bool {
    matches!(id, BackendId::AnthropicHaiku | BackendId::OpenAiLuna)
}

fn utf8_payload(bytes: &[u8]) -> Result<String, JevError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| JevError::InputInvalid {
        detail: "canonical payload is not UTF-8".into(),
    })
}

fn unix_ms(time: SystemTime) -> Result<u64, JevError> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| JevError::ConfigInvalid {
            detail: "session clock is before the unix epoch".into(),
        })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| JevError::ConfigInvalid {
        detail: "session clock is out of range".into(),
    })
}

fn system_time_from_unix_ms(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis)
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp_name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => format!(".{name}.tmp"),
        None => ".session.tmp".to_owned(),
    };
    let tmp = path.with_file_name(tmp_name);
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    tighten_0600(&tmp)?;
    fs::rename(&tmp, path)?;
    tighten_0600(path)?;
    Ok(())
}

fn tighten_0600(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct Prior<'a> {
    backend: BackendId,
    answers: &'a Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionDocument {
    version: u32,
    session_id: SessionId,
    backend: BackendId,
    created_at: u64,
    payload_utf8: String,
    prompt_sha256: sc_agent_types::Sha256Digest,
    system_prompt: Option<String>,
    outcomes: Vec<JudgeOutcome>,
    follow_ups: Vec<String>,
}

impl StoredSession {
    fn from_document(document: SessionDocument) -> Self {
        let bytes = document.payload_utf8.into_bytes();
        Self {
            session_id: document.session_id,
            backend: document.backend,
            created_at: system_time_from_unix_ms(document.created_at),
            payload: PayloadBundle::from_canonical_bytes(bytes),
            system_prompt: document.system_prompt,
            outcomes: document.outcomes,
            follow_ups: document.follow_ups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::observability::v2::NoopSessionLog;
    use sc_agent_types::{JudgeBackend, JudgeRequest};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct SharedBackend {
        prompts: Arc<Mutex<Vec<String>>>,
    }

    impl JudgeBackend for SharedBackend {
        fn id(&self) -> BackendId {
            BackendId::AnthropicHaiku
        }

        fn validate_for_send(&self, _request: &JudgeRequest) -> Result<(), JevError> {
            Ok(())
        }

        fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
            self.prompts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(
                    request
                        .llm
                        .as_ref()
                        .map(|llm| llm.system_prompt.clone())
                        .unwrap_or_default(),
                );
            Ok(JudgeOutcome {
                answers: serde_json::json!({"relevant": {"choice": "yes"}}),
                backend: BackendId::AnthropicHaiku,
                probabilities: None,
                provider_meta: None,
            })
        }
    }

    fn bundle() -> PayloadBundle {
        PayloadBundle::from_canonical_bytes(br#"{"questions":[],"state":[]}"#.to_vec())
    }

    fn store(ttl: Duration) -> (FileSessionStore, crate::extract::test_support::TempDir) {
        let dir = crate::extract::test_support::temp_dir("session");
        let session = FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog))
            .expect("store")
            .with_ttl(ttl);
        (session, dir)
    }

    #[test]
    fn test_session_round_trip() {
        let (store, _dir) = store(Duration::from_secs(3600));
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let mut registry = BackendRegistry::new();
        registry
            .register(Arc::new(SharedBackend {
                prompts: Arc::clone(&prompts),
            }))
            .expect("register");
        store.install_registry(registry);
        let payload = bundle();
        let id = store
            .open(
                BackendId::AnthropicHaiku,
                &payload,
                Some("base prompt".to_owned()),
            )
            .expect("create");
        let loaded = store.load(&id).expect("load");
        assert_eq!(loaded.session_id, id);
        assert_eq!(loaded.payload.bytes, payload.bytes);
        assert_eq!(loaded.payload.sha256, payload.sha256);
        assert_eq!(loaded.system_prompt.as_deref(), Some("base prompt"));
        let again = FileSessionStore::new(store.dir(), Arc::new(NoopSessionLog))
            .expect("reopen")
            .with_ttl(Duration::from_secs(3600));
        again.install_registry(
            store
                .registry
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
        let outcome = again.ask(&id, "why?").expect("ask");
        assert_eq!(outcome.backend, BackendId::AnthropicHaiku);
        assert_eq!(outcome.answers["relevant"]["choice"], "yes");
        let prompt = prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(prompt[0].contains("base prompt"));
        assert!(prompt[0].contains("why?"));
        let round = store.load(&id).expect("after ask");
        assert_eq!(round.outcomes.len(), 1);
        assert_eq!(round.follow_ups, ["why?"]);
        let missing = SessionId::new();
        assert_eq!(
            store.load(&missing).expect_err("missing").code(),
            "SESSION_NOT_FOUND"
        );
    }

    #[test]
    fn test_session_file_mode_0600() {
        let (store, _dir) = store(Duration::from_secs(60));
        let id = store.create(BackendId::Jev, &bundle()).expect("create");
        let path = store.path(&id);
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(id.to_string().as_str())
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn test_session_expiry_uses_created_at() {
        let now = Arc::new(AtomicU64::new(1_700_000_000_000));
        let clock_now = Arc::clone(&now);
        let (store, _dir) = store(Duration::from_secs(60));
        let store = store.with_clock(move || {
            UNIX_EPOCH + Duration::from_millis(clock_now.load(Ordering::Relaxed))
        });
        let id = store.create(BackendId::Jev, &bundle()).expect("create");
        let path = store.path(&id);
        let text = fs::read_to_string(&path).expect("session json");
        let document: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(document["created_at"], 1_700_000_000_000_u64);
        file_set_modified(&path, UNIX_EPOCH + Duration::from_secs(4_000_000_000));
        store.load(&id).expect("fresh created_at ignores mtime");
        now.store(1_700_000_000_000 + 60_000, Ordering::Relaxed);
        file_set_modified(&path, UNIX_EPOCH + Duration::from_millis(1_700_000_000_000));
        let expired = store.load(&id).expect_err("expired");
        assert_eq!(expired.code(), "SESSION_EXPIRED");
        assert!(expired.fix_hint().contains(&id.to_string()));
    }

    fn file_set_modified(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).expect("open");
        file.set_modified(time).expect("mtime");
    }
}
