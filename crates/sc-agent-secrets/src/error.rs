//! The error type of sc-agent-secrets.

use std::fmt;

use sc_agent_types::{ErrorDetail, ErrorId};

/// Recovery text for [`SecretsError::KeyMissing`].
const KEY_MISSING_RECOVERY: &str = "set the variable in the process environment, the repo-root .env file or the platform keyring (service = variable name)";

/// Recovery text for [`SecretsError::KeyMalformed`].
const KEY_MALFORMED_RECOVERY: &str = "remove whitespace and control characters from the key";

/// A key could not be resolved.
///
/// No variant holds key bytes: causes are fixed, sanitized labels, and `kind`
/// fields are display-only text (an `io::ErrorKind` or keyring error kind),
/// not a stable format to match on. [`Display`](fmt::Display) writes
/// `"<id>: <message>"`.
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
    /// The repo-root `.env` exists and cannot be read.
    DotenvRead {
        /// The I/O error kind (display-only).
        kind: String,
        /// A sanitized label of the failure.
        cause: &'static str,
    },
    /// The platform keyring could not be read.
    KeyringRead {
        /// The keyring error kind (display-only).
        kind: String,
        /// A sanitized label of the failure (never the keyring's own text).
        cause: &'static str,
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
            Self::DotenvRead { kind, cause } => {
                format!("the repo-root .env file could not be read ({cause}; {kind})")
            }
            Self::KeyringRead { kind, cause } => {
                format!("the platform keyring could not be read ({cause}; {kind})")
            }
            Self::KeyringTimeout => {
                "the platform keyring did not answer within 15 s, or an earlier read is still stuck"
                    .to_owned()
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        None
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::KeyMissing { .. } => Some(KEY_MISSING_RECOVERY),
            Self::KeyMalformed { .. } => Some(KEY_MALFORMED_RECOVERY),
            Self::DotenvRead { .. } | Self::KeyringRead { .. } | Self::KeyringTimeout => None,
        }
    }
}

impl fmt::Display for SecretsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl std::error::Error for SecretsError {}
