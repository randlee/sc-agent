//! [`DecidrError`], the error of every public function of the decidr library (REQ-SAG-0002).

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use sc_agent_config::RunnerName;
use sc_agent_types::{ErrorDetail, ErrorId, Provider};

use crate::ModelSelector;
use crate::input::LimitKind;

/// A decidr command could not run, or stopped before it finished.
///
/// A decision that fails inside `Decision::decide` or `decide_stream` is not a `DecidrError`:
/// it is the `DecisionError` of its `DecisionResult`, printed in that result's line.
///
/// `Display` is `"<id>: <message>"`. A wrapped error (a lower sc-agent error, or an io, clap,
/// serde_json or ctrlc error) is kept unchanged, boxed where it is large so `DecidrError` stays
/// small; it is reachable through [`Error::source`] and [`ErrorId::cause_detail`].
///
/// # Examples
///
/// ```
/// use decidr::DecidrError;
/// use sc_agent_types::ErrorId;
///
/// let error = DecidrError::InputNotFound { input: "missing.json".into() };
/// assert_eq!(error.id(), "decidr.input_not_found");
/// assert_eq!(error.to_string(), format!("decidr.input_not_found: {}", error.message()));
/// assert!(error.recovery().is_some());
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum DecidrError {
    /// A per-call timeout is non-finite or outside the request-timeout bounds.
    InvalidRequestTimeout {
        /// A lower-level duration-conversion failure, when present.
        source: Option<Box<ErrorDetail>>,
    },
    /// Integration evidence failed boundary validation.
    IntegrationEvidence {
        /// The invalid field or contract element.
        field: &'static str,
        /// The constraint to correct, without the rejected value.
        reason: &'static str,
        /// An underlying parsing failure, when present.
        source: Option<Box<ErrorDetail>>,
    },
    /// The command line does not parse; the clap error is the cause.
    Usage {
        /// clap's error, unchanged.
        source: Box<clap::Error>,
    },
    /// The global configuration could not be loaded or is out of bounds (REQ-SAG-0013).
    Config(Box<sc_agent_config::AgentConfigError>),
    /// `--provider` or `--model` does not parse.
    Types(Box<sc_agent_types::TypesError>),
    /// The provider's or runner's key is missing or malformed.
    Secrets(Box<sc_agent_secrets::SecretsError>),
    /// A slug is invalid (`log.invalid_slug`), or repo detection or a logger failed.
    Log(Box<sc_agent_log::LogError>),
    /// A local runner's backend could not be built.
    Local(Box<sc_decision_local::LocalError>),
    /// Building a System One backend, decoding the models list, or the local-runner load wait
    /// failed.
    Decision(Box<sc_decision_schema::DecisionError>),
    /// A models or doctor send failed.
    Http(Box<sc_agent_http::HttpError>),
    /// An LLM client could not be built or a provider model list could not be read.
    Llm(Box<sc_llm_provider::LlmError>),
    /// `flush_session_log` failed after the results were printed.
    Runner(Box<sc_decision_runner::RunnerError>),
    /// No provider in `Provider::ranking` for the model has a detected key, and no
    /// `--provider` was given.
    NoProviderKey {
        /// The model as selected.
        model: ModelSelector,
        /// The providers tried, in ranking order.
        tried: Vec<Provider>,
    },
    /// `--runner` (or `models(runner=...)`) names no configured local runner.
    UnknownRunner {
        /// The name given (it may not even be a valid runner name).
        name: String,
        /// The configured runner names.
        known: Vec<RunnerName>,
    },
    /// The input file does not exist.
    InputNotFound {
        /// The input as given.
        input: String,
    },
    /// The input could not be read (any io error other than NotFound).
    InputRead {
        /// The input as given.
        input: String,
        /// The io error, unchanged.
        source: std::io::Error,
    },
    /// A REQ-SAG-0014 input limit was exceeded; nothing was sent.
    LimitExceeded {
        /// Which limit.
        what: LimitKind,
        /// The JSONL line (1-based), or `None` for a single request.
        line: Option<u64>,
        /// The limit.
        max: u64,
    },
    /// A batch line is not UTF-8.
    LineNotUtf8 {
        /// The JSONL line (1-based).
        line: u64,
        /// The offset of the first invalid byte in the line.
        byte_offset: usize,
    },
    /// An image file could not be read.
    ImageRead {
        /// The image part name.
        name: String,
        /// The image file.
        path: PathBuf,
        /// The io error, unchanged.
        source: std::io::Error,
    },
    /// An image's extension (files) or magic bytes (in memory) is not png, jpg or jpeg.
    ImageFormat {
        /// The image part name.
        name: String,
        /// The image file; empty for an in-memory image.
        path: PathBuf,
    },
    /// A request does not match the Jev request schema, or `DecisionRequest::new` refused it
    /// (image parts, image limits); the cause is kept (REQ-SAG-0504).
    RequestInvalid {
        /// The JSONL line (1-based), or `None` for a single request.
        line: Option<u64>,
        /// `decision.request_parse`, `decision.image_parts` or `decision.limits`.
        source: Box<sc_decision_schema::DecisionError>,
    },
    /// An output value could not be encoded as JSON.
    Encode {
        /// serde_json's error, unchanged.
        source: serde_json::Error,
    },
    /// A stdout write failed.
    Write {
        /// The io error, unchanged.
        source: std::io::Error,
    },
    /// The interrupt handler could not be installed.
    Signal {
        /// ctrlc's error, unchanged.
        source: ctrlc::Error,
    },
}

/// Every wrapped lower error converts into its variant, boxed.
macro_rules! boxed_from {
    ($($error:ty => $variant:ident),* $(,)?) => {
        $(impl From<$error> for DecidrError {
            fn from(error: $error) -> Self {
                Self::$variant(Box::new(error))
            }
        })*
    };
}

boxed_from! {
    sc_agent_config::AgentConfigError => Config,
    sc_agent_types::TypesError => Types,
    sc_agent_secrets::SecretsError => Secrets,
    sc_agent_log::LogError => Log,
    sc_decision_local::LocalError => Local,
    sc_decision_schema::DecisionError => Decision,
    sc_agent_http::HttpError => Http,
    sc_llm_provider::LlmError => Llm,
    sc_decision_runner::RunnerError => Runner,
}

impl From<serde_json::Error> for DecidrError {
    fn from(source: serde_json::Error) -> Self {
        Self::Encode { source }
    }
}

impl From<clap::Error> for DecidrError {
    fn from(source: clap::Error) -> Self {
        Self::Usage {
            source: Box::new(source),
        }
    }
}

impl DecidrError {
    /// `RequestInvalid` at `line` for `source`.
    #[must_use]
    pub fn request_invalid(line: Option<u64>, source: sc_decision_schema::DecisionError) -> Self {
        Self::RequestInvalid {
            line,
            source: Box::new(source),
        }
    }

    /// An io error reading `input`: `InputNotFound` for NotFound, else `InputRead`.
    #[must_use]
    pub fn input(input: &str, source: std::io::Error) -> Self {
        if source.kind() == std::io::ErrorKind::NotFound {
            Self::InputNotFound {
                input: input.to_owned(),
            }
        } else {
            Self::InputRead {
                input: input.to_owned(),
                source,
            }
        }
    }
}

/// `" at line N"`, or nothing for a single request.
fn at_line(line: Option<u64>) -> String {
    line.map(|line| format!(" at line {line}"))
        .unwrap_or_default()
}

/// A foreign error (io, clap, serde_json, ctrlc) as a cause.
fn foreign(id: &str, error: &(dyn Error + 'static)) -> Option<ErrorDetail> {
    Some(ErrorDetail::foreign(id, error))
}

impl ErrorId for DecidrError {
    fn id(&self) -> &'static str {
        match self {
            Self::InvalidRequestTimeout { .. } => "decidr.invalid_request_timeout",
            Self::IntegrationEvidence { .. } => "decidr.integration_evidence",
            Self::Usage { .. } => "decidr.usage",
            Self::Config(_) => "decidr.config",
            Self::Types(_) => "decidr.types",
            Self::Secrets(_) => "decidr.secrets",
            Self::Log(_) => "decidr.log",
            Self::Local(_) => "decidr.local",
            Self::Decision(_) => "decidr.decision",
            Self::Http(_) => "decidr.http",
            Self::Llm(_) => "decidr.llm",
            Self::Runner(_) => "decidr.runner",
            Self::NoProviderKey { .. } => "decidr.no_provider_key",
            Self::UnknownRunner { .. } => "decidr.unknown_runner",
            Self::InputNotFound { .. } => "decidr.input_not_found",
            Self::InputRead { .. } => "decidr.input_read",
            Self::LimitExceeded { .. } => "decidr.limit_exceeded",
            Self::LineNotUtf8 { .. } => "decidr.line_not_utf8",
            Self::ImageRead { .. } => "decidr.image_read",
            Self::ImageFormat { .. } => "decidr.image_format",
            Self::RequestInvalid { .. } => "decidr.request_invalid",
            Self::Encode { .. } => "decidr.encode",
            Self::Write { .. } => "decidr.write",
            Self::Signal { .. } => "decidr.signal",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidRequestTimeout { .. } => format!(
                "timeout_secs must be finite and within {}..={} seconds",
                sc_agent_config::RUNNER_REQUEST_TIMEOUT_SECS.start(),
                sc_agent_config::RUNNER_REQUEST_TIMEOUT_SECS.end(),
            ),
            Self::IntegrationEvidence { field, reason, .. } => {
                format!("integration evidence {field}: {reason}")
            }
            Self::Usage { source } => format!(
                "invalid command line ({})",
                source.kind().as_str().unwrap_or("usage error")
            ),
            Self::Config(error) => format!("the configuration is not usable ({})", error.id()),
            Self::Types(error) => format!("a value does not parse ({})", error.id()),
            Self::Secrets(error) => format!("a key could not be resolved ({})", error.id()),
            Self::Log(error) => format!("the session identity or a log failed ({})", error.id()),
            Self::Local(error) => {
                format!(
                    "the local runner backend could not be built ({})",
                    error.id()
                )
            }
            Self::Decision(error) => format!("the decision backend failed ({})", error.id()),
            Self::Http(error) => format!("the send failed ({})", error.id()),
            Self::Llm(error) => format!("the LLM provider failed ({})", error.id()),
            Self::Runner(error) => format!("the session log failed ({})", error.id()),
            Self::NoProviderKey { model, tried } => {
                let tried: Vec<&str> = tried.iter().map(|provider| provider.as_str()).collect();
                format!(
                    "no provider key found for model {:?}; tried {}",
                    model.to_string(),
                    tried.join(", ")
                )
            }
            Self::UnknownRunner { name, known } => format!(
                "{name:?} is not a configured local runner (configured: {})",
                if known.is_empty() {
                    "none".to_owned()
                } else {
                    known
                        .iter()
                        .map(RunnerName::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            Self::InputNotFound { input } => format!("input {input:?} does not exist"),
            Self::InputRead { input, source } => {
                format!("input {input:?} could not be read ({})", source.kind())
            }
            Self::LimitExceeded { what, line, max } => {
                format!("the `{what}` limit of {max} was exceeded{}", at_line(*line))
            }
            Self::LineNotUtf8 { line, byte_offset } => {
                format!("line {line} is not UTF-8 (invalid byte at offset {byte_offset})")
            }
            Self::ImageRead { name, path, source } => format!(
                "image {name:?} ({}) could not be read ({})",
                path.display(),
                source.kind()
            ),
            Self::ImageFormat { name, path } => {
                if path.as_os_str().is_empty() {
                    format!("image {name:?} is not png or jpeg")
                } else {
                    format!(
                        "image {name:?} ({}) is not .png, .jpg or .jpeg",
                        path.display()
                    )
                }
            }
            Self::RequestInvalid { line, source } => {
                format!(
                    "the request{} is invalid: {}",
                    at_line(*line),
                    source.message()
                )
            }
            Self::Encode { .. } => "the output could not be encoded as JSON".to_owned(),
            Self::Write { source } => format!("stdout write failed ({})", source.kind()),
            Self::Signal { .. } => "the interrupt handler could not be installed".to_owned(),
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::InvalidRequestTimeout { source } | Self::IntegrationEvidence { source, .. } => {
                source.as_deref().cloned()
            }
            Self::Usage { source } => foreign("clap", &**source),
            Self::Config(error) => Some(error.detail()),
            Self::Types(error) => Some(error.detail()),
            Self::Secrets(error) => Some(error.detail()),
            Self::Log(error) => Some(error.detail()),
            Self::Local(error) => Some(error.detail()),
            Self::Decision(error) | Self::RequestInvalid { source: error, .. } => {
                Some(error.detail())
            }
            Self::Http(error) => Some(error.detail()),
            Self::Llm(error) => Some(error.detail()),
            Self::Runner(error) => Some(error.detail()),
            Self::InputRead { source, .. }
            | Self::ImageRead { source, .. }
            | Self::Write { source } => foreign("io", source),
            Self::Encode { source } => foreign("serde_json", source),
            Self::Signal { source } => foreign("ctrlc", source),
            Self::NoProviderKey { .. }
            | Self::UnknownRunner { .. }
            | Self::InputNotFound { .. }
            | Self::LimitExceeded { .. }
            | Self::LineNotUtf8 { .. }
            | Self::ImageFormat { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::InvalidRequestTimeout { .. } => {
                Some("omit timeout_secs or pass finite seconds within the reported bounds")
            }
            Self::IntegrationEvidence { .. } => Some(
                "rerun every required integration case against checksum-verified candidate assets",
            ),
            Self::Usage { .. } => Some("See decidr --help."),
            Self::Config(error) => error.recovery(),
            Self::Types(error) => error.recovery(),
            Self::Secrets(error) => error.recovery(),
            Self::Log(error) => error.recovery(),
            Self::Local(error) => error.recovery(),
            Self::Decision(error) | Self::RequestInvalid { source: error, .. } => error.recovery(),
            Self::Http(error) => error.recovery(),
            Self::Llm(error) => error.recovery(),
            Self::Runner(error) => error.recovery(),
            Self::NoProviderKey { .. } => {
                Some("set one of the listed providers' key variables, or pass --provider")
            }
            Self::UnknownRunner { .. } => {
                Some("name a key of `local_runners` in the global configuration")
            }
            Self::InputNotFound { .. } => Some("pass an existing request file, or - for stdin"),
            Self::InputRead { .. } => {
                Some("check that the input can be read by this user and send again")
            }
            Self::LimitExceeded { .. } => Some(
                "keep one request under 8 MiB, a batch under 10,000 lines and 256 MiB, and each image under 20 MiB",
            ),
            Self::LineNotUtf8 { .. } => Some("write the batch as UTF-8 JSONL"),
            Self::ImageRead { .. } => Some("pass an image file that exists and can be read"),
            Self::ImageFormat { .. } => Some("pass a .png, .jpg or .jpeg image"),
            Self::Encode { .. } => {
                Some("report this as a bug: decidr's output always encodes as JSON")
            }
            Self::Write { .. } => {
                Some("keep stdout open until decidr exits (a closed pipe stops the batch)")
            }
            Self::Signal { .. } => {
                Some("run decidr in a process with no other interrupt handler installed")
            }
        }
    }
}

impl fmt::Display for DecidrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for DecidrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRequestTimeout { source } | Self::IntegrationEvidence { source, .. } => {
                source.as_ref().map(|error| &**error as &dyn Error)
            }
            Self::Usage { source } => Some(&**source),
            Self::Config(error) => Some(&**error),
            Self::Types(error) => Some(&**error),
            Self::Secrets(error) => Some(&**error),
            Self::Log(error) => Some(&**error),
            Self::Local(error) => Some(&**error),
            Self::Decision(error) | Self::RequestInvalid { source: error, .. } => Some(&**error),
            Self::Http(error) => Some(&**error),
            Self::Llm(error) => Some(&**error),
            Self::Runner(error) => Some(&**error),
            Self::InputRead { source, .. }
            | Self::ImageRead { source, .. }
            | Self::Write { source } => Some(source),
            Self::Encode { source } => Some(source),
            Self::Signal { source } => Some(source),
            Self::NoProviderKey { .. }
            | Self::UnknownRunner { .. }
            | Self::InputNotFound { .. }
            | Self::LimitExceeded { .. }
            | Self::LineNotUtf8 { .. }
            | Self::ImageFormat { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::ShortName;

    fn samples() -> Vec<DecidrError> {
        let io = || std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        vec![
            DecidrError::NoProviderKey {
                model: ModelSelector::Short(ShortName::Jev),
                tried: vec![Provider::TypeSafe],
            },
            DecidrError::from(sc_agent_types::TypesError::UnknownProvider { text: "x".into() }),
            DecidrError::LimitExceeded {
                what: LimitKind::RequestBytes,
                line: Some(3),
                max: 8,
            },
            DecidrError::InputRead {
                input: "f".into(),
                source: io(),
            },
            DecidrError::Write { source: io() },
            DecidrError::from(sc_decision_schema::DecisionError::Limits {
                what: sc_decision_schema::LimitKind::Images,
                max: 20,
            }),
            DecidrError::from(sc_agent_secrets::SecretsError::KeyMissing { var: "V".into() }),
        ]
    }

    /// `detail()` is built from id, message, recovery and cause_detail (the ErrorId contract).
    #[test]
    fn detail_matches_the_four_parts() {
        for error in samples() {
            let detail = error.detail();
            assert_eq!(detail.id, error.id());
            assert_eq!(detail.message, error.message());
            assert_eq!(detail.recovery.as_deref(), error.recovery());
            assert_eq!(detail.cause.map(|cause| *cause), error.cause_detail());
            assert_eq!(
                error.to_string(),
                format!("{}: {}", error.id(), error.message())
            );
        }
    }

    /// RBP-F001: every sample has a recovery; wrapped variants forward the wrapped one's.
    #[test]
    fn every_sample_has_a_recovery_and_wrapped_ones_forward_it() {
        for error in samples() {
            assert!(error.recovery().is_some(), "{} has no recovery", error.id());
        }
        let limits = sc_decision_schema::DecisionError::Limits {
            what: sc_decision_schema::LimitKind::Images,
            max: 20,
        };
        let expected = limits.recovery();
        assert_eq!(DecidrError::from(limits).recovery(), expected);
    }

    /// RBP-F002: io sources are kept, with their kind, through `source()` and `cause_detail()`.
    #[test]
    fn io_sources_are_kept() {
        let error = DecidrError::Write {
            source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe closed"),
        };
        let source = error.source().expect("source");
        let io = source.downcast_ref::<std::io::Error>().expect("io error");
        assert_eq!(io.kind(), std::io::ErrorKind::BrokenPipe);
        let cause = error.cause_detail().expect("cause");
        assert_eq!(cause.id, "io");
        assert_eq!(cause.message, "pipe closed");
        assert!(!error.message().contains("BrokenPipe"), "no Debug strings");
    }

    /// RBP-F009: the boxed sources keep `DecidrError` small.
    #[test]
    fn decide_error_stays_small() {
        assert!(
            std::mem::size_of::<DecidrError>() <= 64,
            "DecidrError is {} bytes",
            std::mem::size_of::<DecidrError>()
        );
    }

    #[test]
    fn no_provider_key_has_a_recovery() {
        let error = DecidrError::NoProviderKey {
            model: ModelSelector::Short(ShortName::Jev),
            tried: vec![Provider::TypeSafe],
        };
        assert_eq!(error.id(), "decidr.no_provider_key");
        assert!(
            error
                .recovery()
                .is_some_and(|text| text.contains("--provider"))
        );
        assert!(error.message().contains("typesafe"));
        assert!(error.message().contains("jev"));
    }
}
