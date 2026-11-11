//! The error type of sc-agent-secrets.

use std::fmt;

use sc_agent_types::{ErrorDetail, ErrorId};

/// Recovery text for [`SecretsError::KeyMissing`].
const KEY_MISSING_RECOVERY: &str = "set the variable in the process environment, the repo-root .env file or the platform keyring (service = variable name)";

/// Recovery text for [`SecretsError::KeyMalformed`].
const KEY_MALFORMED_RECOVERY: &str = "store the key again as visible ASCII only: no spaces, line breaks, control characters or non-ASCII characters";

/// Recovery text for [`SecretsError::DotenvRead`].
const DOTENV_READ_RECOVERY: &str = "make the repo-root .env a readable regular file of at most 1 MiB (check its permissions), or set the variable in the process environment";

/// Recovery text for [`SecretsError::KeyringRead`].
const KEYRING_READ_RECOVERY: &str = "unlock or start the platform keystore for this OS user, or set the variable in the process environment or the repo-root .env file";

/// Recovery text for [`SecretsError::KeyringTimeout`].
const KEYRING_TIMEOUT_RECOVERY: &str = "unlock the platform keystore for this OS user (a retry blocks again until it answers), or set the variable in the process environment or the repo-root .env file";

/// A key could not be resolved.
///
/// No variant holds key bytes. `cause` and `kind` are this layer's summary
/// (display-only text, not a stable format to match on); the lower-layer error
/// is kept whole as `source` (its id and message, REQ-SAG-0002 0.2.0) and is
/// reachable through [`ErrorId::cause_detail`] and `Error::source`. Neither an
/// io error nor a keyring error renders the stored value, so nothing in them
/// needs redacting. [`Display`](fmt::Display) writes `"<id>: <message>"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecretsError {
    /// The variable is unset, empty or blank in every source.
    KeyMissing {
        /// The variable (and keyring service) that was looked up.
        var: String,
    },
    /// The key has whitespace, a control character or a non-visible-ASCII character.
    KeyMalformed {
        /// The variable that held the key.
        var: String,
        /// Which rule the key broke.
        reason: &'static str,
    },
    /// The repo-root `.env` exists and cannot be read, is not a regular file or is too large.
    ///
    /// `cause` labels the failure in this layer's words; `source` is the io
    /// error whole (`io.<ErrorKind>` and its message), `None` for a refusal of
    /// this layer's own (not a regular file, too large).
    DotenvRead {
        /// The I/O error kind (display-only).
        kind: String,
        /// This layer's label of the failure.
        cause: &'static str,
        /// The io error, id and message whole.
        source: Option<ErrorDetail>,
    },
    /// The platform keyring could not be read.
    ///
    /// `cause` labels the failure in this layer's words; `source` is the
    /// keyring error whole (`keyring.<kind>` and its message, which never
    /// renders the stored value), `None` when the read thread itself failed.
    KeyringRead {
        /// The keyring error kind (display-only).
        kind: &'static str,
        /// This layer's label of the failure.
        cause: &'static str,
        /// The keyring error, id and message whole.
        source: Option<ErrorDetail>,
    },
    /// The keyring did not answer within 15 s, or an earlier read is still stuck.
    KeyringTimeout,
}

impl ErrorId for SecretsError {
    fn id(&self) -> &'static str {
        match self {
            Self::KeyMissing { .. } => "secrets.key_missing",
            Self::KeyMalformed { .. } => "secrets.key_malformed",
            Self::DotenvRead { .. } => "secrets.dotenv_read",
            Self::KeyringRead { .. } => "secrets.keyring_read",
            Self::KeyringTimeout => "secrets.keyring_timeout",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::KeyMissing { var } => format!(
                "no key found for {var}: it is unset or blank in the process environment, the repo-root .env file and the platform keyring"
            ),
            Self::KeyMalformed { var, reason } => {
                format!("the key in {var} is malformed: {reason}")
            }
            Self::DotenvRead { kind, cause, .. } => {
                format!("the repo-root .env file could not be read ({cause}; {kind})")
            }
            Self::KeyringRead { kind, cause, .. } => {
                format!("the platform keyring could not be read ({cause}; {kind})")
            }
            Self::KeyringTimeout => {
                "the platform keyring did not answer within 15 s, or an earlier read is still stuck"
                    .to_owned()
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::DotenvRead { source, .. } | Self::KeyringRead { source, .. } => source.clone(),
            Self::KeyMissing { .. } | Self::KeyMalformed { .. } | Self::KeyringTimeout => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::KeyMissing { .. } => Some(KEY_MISSING_RECOVERY),
            Self::KeyMalformed { .. } => Some(KEY_MALFORMED_RECOVERY),
            Self::DotenvRead { .. } => Some(DOTENV_READ_RECOVERY),
            Self::KeyringRead { .. } => Some(KEYRING_READ_RECOVERY),
            Self::KeyringTimeout => Some(KEYRING_TIMEOUT_RECOVERY),
        }
    }
}

impl fmt::Display for SecretsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl std::error::Error for SecretsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DotenvRead { source, .. } | Self::KeyringRead { source, .. } => source
                .as_ref()
                .map(|source| source as &(dyn std::error::Error + 'static)),
            Self::KeyMissing { .. } | Self::KeyMalformed { .. } | Self::KeyringTimeout => None,
        }
    }
}
