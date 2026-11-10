//! [`LogError`]: every failure of sc-agent-log, with stable `log.*` ids (REQ-SAG-0002).

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use sc_agent_types::{ErrorDetail, ErrorId};
use sc_observability::v2::{EventError, FlushError, InitError};

/// Every error a public sc-agent-log function returns.
///
/// `Display` writes `"<id>: <message>"` only. A wrapped lower-layer error (an io error or an
/// sc-observability error) is kept unchanged as [`Error::source`] and appears, with its code and
/// message, in [`ErrorId::cause_detail`] and [`ErrorId::detail`].
///
/// # Examples
///
/// ```
/// use sc_agent_log::{LogError, Slug};
/// use sc_agent_types::ErrorId;
///
/// let error = Slug::new("project", "Phase B").unwrap_err();
/// assert_eq!(error.id(), "log.invalid_slug");
/// assert!(matches!(error, LogError::InvalidSlug { field: "project", .. }));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum LogError {
    /// `project` or `agent_role` is not a slug (REQ-SAG-0004); it is never rewritten.
    InvalidSlug {
        /// `"project"` or `"agent_role"`.
        field: &'static str,
        /// The refused text.
        value: String,
    },
    /// A repo value or git origin URL is not usable. `value` never contains userinfo.
    InvalidRepo {
        /// The refused value, userinfo dropped.
        value: String,
        /// Why it was refused.
        reason: &'static str,
    },
    /// git could not be started.
    GitUnavailable {
        /// The spawn failure.
        source: std::io::Error,
    },
    /// git printed output that is not UTF-8.
    GitOutput {
        /// The git subcommand.
        command: &'static str,
    },
    /// `hostname::get()` failed (`source`) or is not UTF-8 (`None`).
    HostUnknown {
        /// The host name read failure, if any.
        source: Option<std::io::Error>,
    },
    /// The working folder has no final name component, or that name is not UTF-8.
    FolderUnknown {
        /// The folder.
        path: PathBuf,
    },
    /// sc-observability refused a service, target, action or id name.
    InvalidName {
        /// Which name: `"service"`, `"target"`, `"action"`, `"correlation_id"`, `"version"` or `"sink"`.
        what: &'static str,
        /// The refused name.
        value: String,
        /// sc-observability's code and message.
        cause: ErrorDetail,
    },
    /// The session or general log file could not be prepared.
    OpenSink {
        /// The log file.
        path: PathBuf,
        /// sc-observability's failure.
        source: InitError,
    },
    /// The logger could not start.
    Start {
        /// `"general"`, `"session"` or `"memory"`.
        log: &'static str,
        /// sc-observability's failure.
        source: InitError,
    },
    /// A payload key is one of `SessionLogger::IDENTITY_KEYS` or `GeneralLogger::CONTEXT_KEYS`.
    ReservedField {
        /// The reserved key.
        key: String,
    },
    /// `Logger::log` or `Logger::try_log` refused the row.
    Rejected {
        /// `"general"`, `"session"` or `"memory"`.
        log: &'static str,
        /// sc-observability's failure.
        source: EventError,
    },
    /// The flush barrier failed.
    Flush {
        /// `"general"`, `"session"` or `"memory"`.
        log: &'static str,
        /// sc-observability's failure.
        source: FlushError,
    },
    /// `count` rows since the last flush were refused or not written; `last` is the latest failure.
    RowsLost {
        /// `"general"` or `"session"`.
        log: &'static str,
        /// Rows refused at write plus rows the writer could not write.
        count: u64,
        /// The latest failure, if known.
        last: Option<ErrorDetail>,
    },
}

impl LogError {
    /// `InvalidName` for `value`, keeping sc-observability's code and message.
    pub(crate) fn invalid_name(
        what: &'static str,
        value: &str,
        cause: &sc_observability_types::ValueValidationError,
    ) -> Self {
        Self::InvalidName {
            what,
            value: value.to_owned(),
            cause: ErrorDetail::foreign(cause.code().as_str(), cause),
        }
    }
}

/// The foreign id of an io error: `io.<ErrorKind>`, e.g. `io.PermissionDenied`.
fn io_id(error: &std::io::Error) -> String {
    format!("io.{:?}", error.kind())
}

impl ErrorId for LogError {
    fn id(&self) -> &'static str {
        match self {
            Self::InvalidSlug { .. } => "log.invalid_slug",
            Self::InvalidRepo { .. } => "log.invalid_repo",
            Self::GitUnavailable { .. } => "log.git_unavailable",
            Self::GitOutput { .. } => "log.git_output",
            Self::HostUnknown { .. } => "log.host_unknown",
            Self::FolderUnknown { .. } => "log.folder_unknown",
            Self::InvalidName { .. } => "log.invalid_name",
            Self::OpenSink { .. } => "log.open_sink",
            Self::Start { .. } => "log.start",
            Self::ReservedField { .. } => "log.reserved_field",
            Self::Rejected { .. } => "log.rejected",
            Self::Flush { .. } => "log.flush",
            Self::RowsLost { .. } => "log.rows_lost",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidSlug { field, value } => format!(
                "{field} {value:?} is not a slug: use only lowercase a-z, 0-9, '.', '_' and '-' (not empty, not '.' or '..')"
            ),
            Self::InvalidRepo { value, reason } => format!("repo {value:?} is not valid: {reason}"),
            Self::GitUnavailable { .. } => "git could not be started".to_owned(),
            Self::GitOutput { command } => {
                format!("git {command} printed output that is not UTF-8")
            }
            Self::HostUnknown { source: Some(_) } => "the host name could not be read".to_owned(),
            Self::HostUnknown { source: None } => "the host name is not UTF-8".to_owned(),
            Self::FolderUnknown { path } => format!(
                "the folder {} has no final name component in UTF-8",
                path.display()
            ),
            Self::InvalidName { what, value, .. } => {
                format!("sc-observability refused the {what} name {value:?}")
            }
            Self::OpenSink { path, .. } => {
                format!("the log file {} could not be prepared", path.display())
            }
            Self::Start { log, .. } => format!("the {log} logger could not start"),
            Self::ReservedField { key } => {
                format!("payload key {key:?} is reserved for the identity or context fields")
            }
            Self::Rejected { log, .. } => format!("the {log} logger refused the row"),
            Self::Flush { log, .. } => format!("the {log} logger could not flush"),
            Self::RowsLost { log, count, .. } => {
                format!("{count} {log} row(s) were refused or not written since the last flush")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::GitUnavailable { source }
            | Self::HostUnknown {
                source: Some(source),
            } => Some(ErrorDetail::foreign(io_id(source), source)),
            Self::InvalidName { cause, .. } => Some(cause.clone()),
            Self::OpenSink { source, .. } | Self::Start { source, .. } => Some(
                ErrorDetail::foreign(source.diagnostic().code.as_str(), source),
            ),
            Self::Rejected { source, .. } => Some(ErrorDetail::foreign(
                source.diagnostic().code.as_str(),
                source,
            )),
            Self::Flush { source, .. } => Some(ErrorDetail::foreign(
                source.diagnostic().code.as_str(),
                source,
            )),
            Self::RowsLost { last, .. } => last.clone(),
            Self::InvalidSlug { .. }
            | Self::InvalidRepo { .. }
            | Self::GitOutput { .. }
            | Self::HostUnknown { source: None }
            | Self::FolderUnknown { .. }
            | Self::ReservedField { .. } => None,
        }
    }
}

impl fmt::Display for LogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for LogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GitUnavailable { source }
            | Self::HostUnknown {
                source: Some(source),
            } => Some(source),
            Self::InvalidName { cause, .. } => Some(cause),
            Self::OpenSink { source, .. } | Self::Start { source, .. } => Some(source),
            Self::Rejected { source, .. } => Some(source),
            Self::Flush { source, .. } => Some(source),
            Self::RowsLost { last, .. } => last.as_ref().map(|last| last as &(dyn Error + 'static)),
            Self::InvalidSlug { .. }
            | Self::InvalidRepo { .. }
            | Self::GitOutput { .. }
            | Self::HostUnknown { source: None }
            | Self::FolderUnknown { .. }
            | Self::ReservedField { .. } => None,
        }
    }
}
