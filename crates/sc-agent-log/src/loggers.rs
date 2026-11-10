//! [`Loggers`]: both loggers, each injected or the default (REQ-SAG-0008).

use std::path::Path;
use std::sync::Arc;

use sc_observability::v2::Logger;

use crate::writer::{Ledger, last_resort};
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
    /// When both are the same `Arc<Logger>`, the two loggers share one lost-row account, so a
    /// row that logger loses is reported by exactly one flush.
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
        let shared = match (&general, &session) {
            (Some(general), Some(session)) if Arc::ptr_eq(general, session) => {
                Some(Ledger::new(general))
            }
            _ => None,
        };
        let general = match general {
            Some(logger) => GeneralLogger::injected_in(logger, shared.clone()),
            None => GeneralLogger::open_default(log_root, service)?,
        };
        let session = match session {
            Some(logger) => SessionLogger::injected_in(logger, shared, identity),
            None => SessionLogger::open_default(log_root, identity)?,
        };
        Ok(Self { general, session })
    }

    /// Closes both loggers ([`GeneralLogger::close`], [`SessionLogger::close`]).
    ///
    /// Both are always closed. The general logger's error is returned first; when both fail, the
    /// session logger's error is written as one last-resort stderr line so it is not lost.
    ///
    /// # Errors
    ///
    /// [`LogError::Flush`] or [`LogError::RowsLost`] of either logger.
    pub fn close(self) -> Result<(), LogError> {
        let general = self.general.close();
        let session = self.session.close();
        match (general, session) {
            (Err(first), Err(second)) => {
                last_resort(crate::LogKind::Session, Some(&second), None);
                Err(first)
            }
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}
