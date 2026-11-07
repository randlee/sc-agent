//! File-backed judge sessions (REQ-SAG-0600, ADR-SAG-0008).
//!
//! Each session is a UUID file with mode `0600`. Expiry is `created_at` plus
//! the store TTL. Touching the file does not extend the session. Haiku and
//! Luna `ask` calls reuse the stored payload, system prompt, and prior
//! answers. Jev has no rationale field, so `ask` refuses a Jev session.
//!
//! [`SessionStore`] is an open extension point. [`FileSessionStore`] is the
//! Phase A implementation.
//!
//! # Durability and concurrency
//!
//! A session file is replaced atomically: the new document goes to a unique
//! `.{id}.{pid}.{nonce}.tmp` file created with `create_new`, is synced, then
//! renamed over the session file, and the directory is synced on Unix. A
//! failed write removes its temporary file. Session directories this store
//! creates are mode `0700` on Unix and session files are mode `0600`.
//!
//! The store assumes **one writer per session**. Clones of one store in one
//! process serialize read-modify-write through a shared lock. Two processes
//! that update the same session at the same time (two concurrent `ask`
//! calls) can lose one update; neither corrupts the file. There is no
//! cross-process lock.

#![warn(missing_docs)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
///
/// The shared file lock serializes session-file read-modify-write. A run and a
/// later `ask` can hold clones of one store, and the lock keeps those calls
/// from interleaving writes to the same UUID file.
#[derive(Clone)]
pub struct FileSessionStore {
    dir: PathBuf,
    ttl: Duration,
    clock: Arc<dyn Fn() -> SystemTime + Send + Sync>,
    registry: Arc<RwLock<BackendRegistry>>,
    session_log: SharedSessionLog,
    /// Shared by every clone. Session create, record, and `ask` take this lock
    /// for the whole read-modify-write so concurrent callers cannot corrupt a
    /// session file. `RwLock` is the wrong shape: the critical section updates
    /// the file, not an in-memory map.
    io: Arc<Mutex<()>>,
}

impl FileSessionStore {
    /// Creates a store rooted at `dir`, creating the directory when needed.
    ///
    /// Directories created here are mode `0700` on Unix. An existing
    /// directory's mode is left unchanged. Session files created later are
    /// mode `0600`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::session_store_io`] when `dir` cannot be created.
    /// The code is `CONFIG_INVALID`. The fix hint names `dir`.
    pub fn new(dir: impl Into<PathBuf>, session_log: SharedSessionLog) -> Result<Self, JevError> {
        let dir = dir.into();
        create_private_dir(&dir).map_err(|error| {
            JevError::session_store_io(&format!(
                "Create session directory {} or choose a writable directory (cause: {}).",
                dir.display(),
                error.kind()
            ))
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
    /// [`JevError::session_store_io`] when the session file cannot be written.
    /// The write failure code is `CONFIG_INVALID` and names the session file.
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
            payload_utf8: utf8_payload(bundle.bytes())?,
            prompt_sha256: bundle.sha256().clone(),
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
    /// Returns [`JevError::SessionNotFound`] when the file is absent,
    /// [`JevError::session_store_io`] when the file cannot be read, and
    /// [`JevError::session_document_invalid`] when the file is not a session
    /// document. The fix hint names the session file.
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
    /// [`JevError::SessionExpired`] when `created_at` is past the TTL,
    /// [`JevError::session_store_io`] when the file cannot be read, and
    /// [`JevError::session_document_invalid`] when the file is not a session
    /// document.
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
                return Err(session_io(
                    &path,
                    &self.dir,
                    &format!("could not be opened (cause: {})", error.kind()),
                ));
            }
        };
        let mut bytes = Vec::new();
        file.take(MAX_SESSION_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| {
                session_io(
                    &path,
                    &self.dir,
                    &format!("could not be read (cause: {})", error.kind()),
                )
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SESSION_BYTES {
            return Err(session_parse(
                &path,
                &format!("exceeds {MAX_SESSION_BYTES} bytes"),
            ));
        }
        let document: SessionDocument = serde_json::from_slice(&bytes).map_err(|error| {
            session_parse(
                &path,
                &format!("is not a session document ({})", json_cause(&error)),
            )
        })?;
        if document.version != SESSION_VERSION || document.session_id != *id {
            return Err(session_parse(
                &path,
                "does not match this session id or version",
            ));
        }
        let payload = utf8_payload(document.payload_utf8.as_bytes())?;
        if sc_agent_types::Sha256Digest::of(payload.as_bytes()) != document.prompt_sha256 {
            return Err(session_parse(&path, "payload digest does not match"));
        }
        Ok(document)
    }

    fn write_document(&self, document: &SessionDocument) -> Result<(), JevError> {
        let path = self.path(&document.session_id);
        let bytes = serde_json::to_vec(document).map_err(|error| {
            session_parse(
                &path,
                &format!("could not be encoded ({})", json_cause(&error)),
            )
        })?;
        write_private(&path, &bytes).map_err(|error| {
            session_io(
                &path,
                &self.dir,
                &format!("could not be written (cause: {})", error.kind()),
            )
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
            return Err(follow_up_invalid(
                "The follow-up text is empty. Pass non-empty text to ask.",
            ));
        }
        let stored = self.load(id)?;
        if !stored.backend.is_llm() {
            return Err(follow_up_invalid(
                "Jev sessions have no rationale field. Ask a Haiku or Luna session instead.",
            ));
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
                prompt_sha256: stored.payload.sha256().clone(),
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
    let json = serde_json::to_string(&prior)
        .map_err(|error| JevError::encode_failed("session context", &error))?;
    Ok(format!("\n\n{SESSION_CONTEXT_HEADER}\n{json}"))
}

pub(crate) fn emit_log(log: &SharedSessionLog, event: LogEvent) {
    let _ = log.try_log(event);
}

/// `serde_json` failure without its message, which can quote document values.
fn json_cause(error: &serde_json::Error) -> String {
    format!(
        "{:?} error at line {} column {}",
        error.classify(),
        error.line(),
        error.column()
    )
}

fn session_io(path: &Path, dir: &Path, problem: &str) -> JevError {
    JevError::session_store_io(&format!(
        "Session file {} {problem}. Check that session directory {} is accessible, then retry.",
        path.display(),
        dir.display()
    ))
}

fn session_parse(path: &Path, problem: &str) -> JevError {
    JevError::session_document_invalid(&format!(
        "Session file {} {problem}. Delete it and start a new session.",
        path.display()
    ))
}

/// Canonical payload bytes as UTF-8 text.
///
/// The one definition in this crate; session files store the payload as text.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the bytes are not UTF-8.
pub(crate) fn utf8_payload(bytes: &[u8]) -> Result<String, JevError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| JevError::InputInvalid {
        detail: "canonical payload is not UTF-8".into(),
    })
}

/// `INPUT_INVALID` for a refused `ask` whose fix hint is `recovery` verbatim.
fn follow_up_invalid(recovery: &str) -> JevError {
    JevError::operational_io(recovery)
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

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// Unique temporary sibling of `path`: `.{name}.{pid}.{nonce}.tmp`.
fn tmp_path(path: &Path) -> PathBuf {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{name}.{}.{nanos:08x}{nonce:04x}.tmp",
        std::process::id()
    ))
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let (tmp, mut file) = loop {
        let tmp = tmp_path(path);
        match options.open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    };
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| tighten_0600(&tmp));
    drop(file);
    if let Err(error) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    tighten_0600(path)?;
    sync_parent(path)
}

/// Syncs the directory entry after a rename so the new file survives a crash.
fn sync_parent(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            File::open(parent)?.sync_all()?;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
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
        assert_eq!(loaded.payload.bytes(), payload.bytes());
        assert_eq!(loaded.payload.sha256(), payload.sha256());
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

    #[test]
    fn session_parse_hint_names_the_file() {
        let (store, _dir) = store(Duration::from_secs(60));
        let id = store.create(BackendId::Jev, &bundle()).expect("create");
        let path = store.path(&id);
        fs::write(&path, b"not-json").expect("corrupt");
        let error = store.load(&id).expect_err("parse");
        assert_eq!(error.code(), "INPUT_INVALID");
        let hint = error.fix_hint();
        assert!(
            hint.contains(&path.display().to_string()),
            "parse hint names the session file: {hint}"
        );
        assert!(
            !hint.contains("check.toml"),
            "parse hint is not a check.toml hint: {hint}"
        );
        assert!(
            !hint.to_ascii_lowercase().contains("schema"),
            "parse hint is not a schema hint: {hint}"
        );
    }

    #[test]
    fn session_directory_failure_names_the_directory() {
        let parent = crate::extract::test_support::temp_dir("session-file-parent");
        let blocker = parent.path().join("not-a-directory");
        fs::write(&blocker, b"x").expect("blocker");
        let nested = blocker.join("sessions");
        let error =
            FileSessionStore::new(nested.as_path(), Arc::new(NoopSessionLog)).expect_err("dir");
        assert_eq!(error.code(), "CONFIG_INVALID");
        let hint = error.fix_hint();
        assert!(
            hint.contains(&nested.display().to_string()),
            "directory hint names the session directory: {hint}"
        );
        assert!(
            !hint.contains("check.toml"),
            "directory hint is not a check.toml hint: {hint}"
        );
    }

    #[test]
    fn session_writes_leave_no_temporary_files() {
        let (store, dir) = store(Duration::from_secs(60));
        let id = store.create(BackendId::Jev, &bundle()).expect("create");
        store
            .record_outcome(
                &id,
                &JudgeOutcome {
                    answers: serde_json::json!({}),
                    backend: BackendId::Jev,
                    probabilities: None,
                    provider_meta: None,
                },
            )
            .expect("record");
        let names: Vec<String> = fs::read_dir(dir.path())
            .expect("dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, [id.to_string()]);
    }

    #[test]
    fn temporary_names_are_unique_per_write() {
        let path = Path::new("/sessions/abc");
        let first = tmp_path(path);
        let second = tmp_path(path);
        assert_ne!(first, second);
        let name = first
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        assert!(name.starts_with(".abc."), "{name}");
        assert!(name.contains(&std::process::id().to_string()), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
    }

    #[test]
    fn failed_write_removes_its_temporary_file() {
        let dir = crate::extract::test_support::temp_dir("session-tmp");
        let target = dir.path().join("occupied");
        fs::create_dir(&target).expect("directory blocks the rename");
        fs::write(target.join("keep"), b"x").expect("non-empty");
        write_private(&target, b"{}").expect_err("rename onto a directory fails");
        let leftovers: Vec<String> = fs::read_dir(dir.path())
            .expect("dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn created_session_directory_is_mode_0700() {
        use std::os::unix::fs::PermissionsExt;
        let parent = crate::extract::test_support::temp_dir("session-mode");
        let nested = parent.path().join("a").join("sessions");
        FileSessionStore::new(&nested, Arc::new(NoopSessionLog)).expect("store");
        let mode = fs::metadata(&nested).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    fn file_set_modified(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).expect("open");
        file.set_modified(time).expect("mtime");
    }
}
