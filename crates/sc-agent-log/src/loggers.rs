//! [`Loggers`]: both loggers, each injected or the default (REQ-SAG-0008).

use std::path::Path;
use std::sync::Arc;

use sc_observability::v2::Logger;

use crate::{GeneralLogger, LogError, SessionIdentity, SessionLogger};

/// The general and session loggers of one process (REQ-SAG-0008).
///
/// Inject either, both or neither; anything not injected is the default
/// ([`GeneralLogger::open_default`], [`SessionLogger::open_default`]).
#[derive(Debug, Clone)]
pub struct Loggers {
    /// The general (diagnostic) log.
    pub general: GeneralLogger,
    /// The session log.
    pub session: SessionLogger,
}

impl Loggers {
    /// Opens both loggers: `general` and `session` are injected loggers, `None` gets the default.
    ///
    /// # Errors
    ///
    /// The [`LogError`] of [`GeneralLogger::open_default`] or [`SessionLogger::open_default`]
    /// for a default that cannot open.
    pub fn open(
        log_root: &Path,
        service: &'static str,
        identity: SessionIdentity,
        general: Option<Arc<Logger>>,
        session: Option<Arc<Logger>>,
    ) -> Result<Self, LogError> {
        let general = match general {
            Some(logger) => GeneralLogger::injected(logger),
            None => GeneralLogger::open_default(log_root, service)?,
        };
        let session = match session {
            Some(logger) => SessionLogger::injected(logger, identity),
            None => SessionLogger::open_default(log_root, identity)?,
        };
        Ok(Self { general, session })
    }
}
