//! File-backed judge sessions (REQ-SAG-0600, REQ-SAG-0607, ADR-SAG-0008,
//! ADR-SAG-0024).
//!
//! Each session is one `<id>.json` file with mode `0600` in a directory with
//! mode `0700` (when this store creates it). Expiry is `created_at` plus the
//! store TTL; touching the file does not extend the session. Haiku and Luna
//! `ask` calls reuse the stored payload, system prompt, and prior answers.
//! Jev has no rationale field, so `ask` refuses a Jev session.
//!
//! [`FileSessionStore`] is stateless: a directory, a TTL, and a clock `fn`
//! pointer. It holds no registry, logger, or lock. [`FileSessionStore::ask`]
//! takes the [`BackendRegistry`] and [`SharedSessionLog`] for that call.
//! `crate::run::run` writes a session once, at the end of a run, and only
//! when a backend whose descriptor has `supports_session_ask` returned
//! `Ok`.
//!
//! # Concurrency model (RSH-004)
//!
//! There is no lock, in process or across processes.
//!
//! - [`FileSessionStore::save`] writes the document to
//!   `<id>.json.tmp.<pid>.<random>` opened with `create_new`, fsyncs it,
//!   renames it over `<id>.json`, then fsyncs the parent directory. Any
//!   failure removes the temporary file. A rename is atomic on one file
//!   system, so a reader sees either the old document or the new one, never
//!   a torn write.
//! - Two writers of the same id: the last rename wins. Neither corrupts the
//!   file. Session ids are random UUIDs and `run` saves each id once, so
//!   this only happens when a caller saves the same [`StoredSession`] twice.
//!   On Windows, replacing a file that another writer is replacing at the
//!   same instant can fail with a sharing error; that save returns
//!   `SessionStoreIo` and removes its temporary file, and the other write
//!   stands.
//! - Readers re-verify every document they load: the stored id must match
//!   the file name's id and the payload must match its stored SHA-256, and
//!   every stored outcome is checked against the payload. A file that fails
//!   any check is `SessionDocumentInvalid`.
//! - [`FileSessionStore::ask`] reads the session and never rewrites it, so
//!   concurrent `ask` calls on one session cannot lose each other's data.
//! - A crashed writer can leave a temporary file behind.
//!   [`FileSessionStore::new`] removes `*.tmp.*` files older than one hour
//!   ([`STALE_TMP_AGE`]); younger ones may belong to a live writer.

#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::collections::hash_map::RandomState;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sc_agent_types::observability::v2::{LogEvent, SharedSessionLog};
use sc_agent_types::{
    BackendId, JevError, JudgeOutcome, JudgeRequest, LlmWireContext, PayloadBundle, QuestionAnswer,
    QuestionId, SessionId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::registry::BackendRegistry;

/// Default lifetime measured from `created_at`.
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Age after which [`FileSessionStore::new`] removes a leftover temporary file.
///
/// A save finishes in well under a second, so a temporary file this old
/// belongs to a writer that crashed. One hour leaves a wide margin for a
/// slow disk without letting leftovers pile up.
pub const STALE_TMP_AGE: Duration = Duration::from_secs(60 * 60);

/// Header inserted before prior answers in an LLM system prompt.
pub const SESSION_CONTEXT_HEADER: &str = "Session context:";

/// Largest session file this store will read.
const MAX_SESSION_BYTES: u64 = 8 * 1024 * 1024;

/// Session document version written and accepted by this store.
///
/// Version 1 (Phase A) files are refused: their outcomes carry the removed
/// `probabilities` field and untyped answers (r-1 D7.6).
const SESSION_VERSION: u64 = 2;

/// Session file extension.
const SESSION_EXT: &str = "json";

/// Marker between a session file name and a temporary suffix.
const TMP_MARKER: &str = ".tmp.";

/// One session document.
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
    /// Base LLM system prompt. `None` for a session with no LLM backend.
    pub system_prompt: Option<String>,
    /// Prior judge outcomes, in call order.
    pub outcomes: Vec<JudgeOutcome>,
    /// Follow-up texts included before a new one when the session is asked.
    ///
    /// `ask` does not append to this list or rewrite the file; a caller
    /// that keeps a conversation saves a new document itself.
    pub follow_ups: Vec<String>,
}

/// Sessions stored as `<id>.json` files under one directory.
///
/// Stateless and cheap to copy: `dir`, `ttl`, and a `clock` function
/// pointer. See the module documentation for the concurrency model.
#[derive(Debug, Clone)]
pub struct FileSessionStore {
    dir: PathBuf,
    ttl: Duration,
    clock: fn() -> SystemTime,
}

impl FileSessionStore {
    /// Creates a store rooted at `dir`, creating the directory when needed,
    /// and removes temporary files older than [`STALE_TMP_AGE`].
    ///
    /// Directories created here are mode `0700` on Unix. An existing
    /// directory's mode is left unchanged. Session files are mode `0600`.
    /// The sweep is best effort: a file that cannot be inspected or removed
    /// is left in place.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::session_store_io`] when `dir` cannot be created.
    /// The code is `CONFIG_INVALID`. The fix hint names `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self, JevError> {
        let dir = dir.into();
        create_private_dir(&dir).map_err(|error| {
            JevError::session_store_io(
                format!("Create session directory {}", dir.display()),
                error.kind(),
                "Choose a writable directory, then retry.",
            )
        })?;
        sweep_stale_tmp(&dir, SystemTime::now());
        Ok(Self {
            dir,
            ttl: DEFAULT_SESSION_TTL,
            clock: SystemTime::now,
        })
    }

    /// Sets the lifetime measured from `created_at`.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Replaces the clock used for expiry and by [`Self::now`].
    #[must_use]
    pub fn with_clock(mut self, clock: fn() -> SystemTime) -> Self {
        self.clock = clock;
        self
    }

    /// Directory that holds session files.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Path of the `<id>.json` file for `id`.
    #[must_use]
    pub fn path(&self, id: &SessionId) -> PathBuf {
        self.dir.join(format!("{id}.{SESSION_EXT}"))
    }

    /// The store clock's current time, used for a new session's `created_at`.
    #[must_use]
    pub fn now(&self) -> SystemTime {
        (self.clock)()
    }

    /// Writes `session` to `<id>.json`, replacing any existing file.
    ///
    /// The write is atomic (temporary file, fsync, rename, parent fsync) and
    /// removes its temporary file on any error. Last writer wins.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::SessionStoreIo`] (`CONFIG_INVALID`, naming the
    /// session file) when the file cannot be written, and `CONFIG_INVALID`
    /// when `created_at` is before the Unix epoch.
    pub fn save(&self, session: &StoredSession) -> Result<(), JevError> {
        let document = SessionDocument {
            version: SESSION_VERSION,
            session_id: session.session_id,
            backend: session.backend,
            created_at: unix_ms(session.created_at)?,
            payload_utf8: session.payload.as_str().to_owned(),
            prompt_sha256: session.payload.sha256().clone(),
            system_prompt: session.system_prompt.clone(),
            outcomes: session.outcomes.clone(),
            follow_ups: session.follow_ups.clone(),
        };
        let path = self.path(&session.session_id);
        let bytes = serde_json::to_vec(&document).map_err(|error| {
            session_parse(
                &path,
                &format!("could not be encoded ({})", json_cause(&error)),
            )
        })?;
        write_private(&path, &bytes)
            .map_err(|error| session_io(&path, &self.dir, "write", error.kind()))
    }

    /// Loads `id` when `created_at` plus the TTL is still in the future.
    ///
    /// File modification time is not consulted.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::SessionNotFound`] when the file is absent,
    /// [`JevError::SessionExpired`] when `created_at` is past the TTL,
    /// [`JevError::session_store_io`] when the file cannot be read, and
    /// [`JevError::session_document_invalid`] when the file is not a valid
    /// session document for `id`.
    pub fn load(&self, id: &SessionId) -> Result<StoredSession, JevError> {
        let document = self.read_document(id)?;
        if self.is_expired(document.created_at)? {
            return Err(JevError::session_expired(*id));
        }
        StoredSession::from_document(document, &self.path(id))
    }

    /// Sends `follow_up` to the session's Haiku or Luna backend.
    ///
    /// The backend comes from `registry`; one [`LogEvent`] with
    /// [`sc_agent_types::Operation::Ask`] goes to `log`. The session file is
    /// read and not rewritten.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::InputInvalid`] when `follow_up` is empty or the
    /// session's backend has no `ask`; the [`Self::load`] errors; the
    /// registry error when the backend is not registered; and the backend's
    /// `validate_for_send` or `judge` error.
    pub fn ask(
        &self,
        id: &SessionId,
        follow_up: &str,
        registry: &BackendRegistry,
        log: &SharedSessionLog,
    ) -> Result<JudgeOutcome, JevError> {
        if follow_up.trim().is_empty() {
            return Err(follow_up_invalid(
                "The follow-up text is empty. Pass non-empty text to ask.",
            ));
        }
        let stored = self.load(id)?;
        if !stored.backend.descriptor().supports_session_ask() {
            return Err(follow_up_invalid(
                "Jev sessions have no rationale field. Ask a Haiku or Luna session instead.",
            ));
        }
        let backend = registry.get(stored.backend)?;
        let mut prompt = stored.system_prompt.clone().unwrap_or_default();
        prompt.push_str(&render_session_context(&stored.outcomes)?);
        for prior in &stored.follow_ups {
            prompt.push_str("\n\nFollow-up:\n");
            prompt.push_str(prior);
        }
        prompt.push_str("\n\nFollow-up:\n");
        prompt.push_str(follow_up);
        let request = JudgeRequest::new(
            std::sync::Arc::new(stored.payload.clone()),
            Some(LlmWireContext::new(prompt)),
        );
        backend.validate_for_send(&request)?;
        let judged = backend.judge(&request);
        emit_log(
            log,
            LogEvent::new(
                *id,
                stored.backend,
                stored.payload.sha256().clone(),
                sc_agent_types::Operation::Ask,
            ),
        );
        judged
    }

    fn is_expired(&self, created_at: u64) -> Result<bool, JevError> {
        let now = unix_ms(self.now())?;
        let ttl_ms = u64::try_from(self.ttl.as_millis()).unwrap_or(u64::MAX);
        Ok(now >= created_at.saturating_add(ttl_ms))
    }

    fn read_document(&self, id: &SessionId) -> Result<SessionDocument, JevError> {
        let path = self.path(id);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(JevError::session_not_found(*id));
            }
            Err(error) => {
                return Err(session_io(&path, &self.dir, "open", error.kind()));
            }
        };
        let mut bytes = Vec::new();
        file.take(MAX_SESSION_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| session_io(&path, &self.dir, "read", error.kind()))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SESSION_BYTES {
            return Err(session_parse(
                &path,
                &format!("exceeds {MAX_SESSION_BYTES} bytes"),
            ));
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            session_parse(
                &path,
                &format!("is not a session document ({})", json_cause(&error)),
            )
        })?;
        check_version(&path, &value)?;
        let document: SessionDocument = serde_json::from_value(value).map_err(|error| {
            session_parse(
                &path,
                &format!("is not a session document ({})", json_cause(&error)),
            )
        })?;
        if document.session_id != *id {
            return Err(session_parse(&path, "does not match this session id"));
        }
        if sc_agent_types::Sha256Digest::of(document.payload_utf8.as_bytes())
            != document.prompt_sha256
        {
            return Err(session_parse(&path, "payload digest does not match"));
        }
        Ok(document)
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
pub(crate) fn render_session_context(outcomes: &[JudgeOutcome]) -> Result<String, JevError> {
    if outcomes.is_empty() {
        return Ok(String::new());
    }
    let prior: Vec<Prior<'_>> = outcomes
        .iter()
        .map(|outcome| Prior {
            backend: outcome.backend(),
            answers: outcome.answers(),
        })
        .collect();
    let json = serde_json::to_string(&prior)
        .map_err(|error| JevError::encode_failed("session context", &error))?;
    Ok(format!("\n\n{SESSION_CONTEXT_HEADER}\n{json}"))
}

pub(crate) fn emit_log(log: &SharedSessionLog, event: LogEvent) {
    let _ = log.emit(event);
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

fn session_io(path: &Path, dir: &Path, op: &str, kind: std::io::ErrorKind) -> JevError {
    JevError::session_store_io(
        format!("Session file {} {op}", path.display()),
        kind,
        format!(
            "Check that session directory {} is accessible, then retry.",
            dir.display()
        ),
    )
}

fn session_parse(path: &Path, problem: &str) -> JevError {
    JevError::session_document_invalid(
        format!("Session file {} {problem}", path.display()),
        "Delete it and start a new session.",
    )
}

/// Refuses any session document whose `version` is not [`SESSION_VERSION`].
///
/// A missing or older version is a Phase A file; a newer one was written by
/// a newer sc-agent. Both are `SessionDocumentInvalid` (`INPUT_INVALID`).
fn check_version(path: &Path, value: &Value) -> Result<(), JevError> {
    match value.get("version").and_then(Value::as_u64) {
        Some(SESSION_VERSION) => Ok(()),
        Some(newer) if newer > SESSION_VERSION => Err(JevError::session_document_invalid(
            format!(
                "Session file {} is version {newer}; this sc-agent reads version {SESSION_VERSION}",
                path.display()
            ),
            "written by a newer sc-agent; upgrade or re-run the check",
        )),
        older => Err(JevError::session_document_invalid(
            format!(
                "Session file {} is version {}; this sc-agent reads version {SESSION_VERSION}",
                path.display(),
                older.map_or_else(|| "missing".to_owned(), |v| v.to_string())
            ),
            "re-run the check; Phase A session files cannot be resumed",
        )),
    }
}

/// `INPUT_INVALID` for a refused `ask` whose fix hint is `recovery` verbatim.
fn follow_up_invalid(recovery: &str) -> JevError {
    JevError::operational_io(recovery)
}

fn unix_ms(time: SystemTime) -> Result<u64, JevError> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            JevError::operational_config(
                "the system clock is before the unix epoch, so the session timestamp cannot be written. Fix the system clock and retry.",
            )
        })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| {
        JevError::operational_config(
            "the system clock is out of range for a session timestamp. Fix the system clock and retry.",
        )
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

/// Unique temporary sibling of `path`: `<name>.tmp.<pid>.<random>`.
///
/// The random part is 64 bits from a fresh [`RandomState`] (seeded from the
/// OS), so two writers in one process or two processes never pick the same
/// name; `create_new` refuses the rare collision and the caller retries.
fn tmp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session");
    let random = RandomState::new().build_hasher().finish();
    path.with_file_name(format!(
        "{name}{TMP_MARKER}{}.{random:016x}",
        std::process::id()
    ))
}

/// Atomically replaces `path` with `bytes` (mode `0600`).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let (tmp, file) = loop {
        let tmp = tmp_path(path);
        match options.open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    };
    let written = finish_tmp(file, &tmp, bytes)
        .and_then(|()| fs::rename(&tmp, path))
        .and_then(|()| sync_parent(path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// Writes, fsyncs, and closes the temporary file.
fn finish_tmp(mut file: File, tmp: &Path, bytes: &[u8]) -> std::io::Result<()> {
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    tighten_0600(tmp)
}

/// Removes `<name>.tmp.*` files in `dir` last modified before `now - STALE_TMP_AGE`.
fn sweep_stale_tmp(dir: &Path, now: SystemTime) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let Some(cutoff) = now.checked_sub(STALE_TMP_AGE) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_str().is_some_and(|name| name.contains(TMP_MARKER)) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified < cutoff);
        if stale && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
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
    answers: &'a BTreeMap<QuestionId, QuestionAnswer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDocument {
    version: u64,
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
    fn from_document(document: SessionDocument, path: &Path) -> Result<Self, JevError> {
        let payload = PayloadBundle::decode(document.payload_utf8.as_bytes())
            .map_err(|_| session_parse(path, "holds a payload that is not canonical"))?;
        for outcome in &document.outcomes {
            outcome.check_against(&payload).map_err(|_| {
                session_parse(path, "holds an outcome that does not match its payload")
            })?;
        }
        Ok(Self {
            session_id: document.session_id,
            backend: document.backend,
            created_at: system_time_from_unix_ms(document.created_at),
            payload,
            system_prompt: document.system_prompt,
            outcomes: document.outcomes,
            follow_ups: document.follow_ups,
        })
    }
}

#[cfg(test)]
mod tests;
