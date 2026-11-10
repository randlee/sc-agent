//! [`DecideError`], the error of every public function of the decide library (REQ-SAG-0002).

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use sc_agent_types::{ErrorDetail, ErrorId, Provider};

/// A decide command could not run, or stopped before it finished.
///
/// A decision that fails inside `Decision::decide` or `decide_stream` is not a `DecideError`:
/// it is the `DecisionError` of its `DecisionResult`, printed in that result's line.
///
/// `Display` is `"<id>: <message>"`; a wrapped lower error is kept unchanged and is reachable
/// through [`Error::source`] and [`ErrorId::cause_detail`].
///
/// # Examples
///
/// ```
/// use decide::DecideError;
/// use sc_agent_types::ErrorId;
///
/// let error = DecideError::InputNotFound { input: "missing.json".into() };
/// assert_eq!(error.id(), "decide.input_not_found");
/// assert_eq!(error.to_string(), format!("decide.input_not_found: {}", error.message()));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum DecideError {
    /// A clap error; `kind` is the Debug text of `clap::error::ErrorKind` (display only),
    /// `detail` clap's text.
    Usage {
        /// The clap error kind (display only).
        kind: String,
        /// clap's rendered text.
        detail: String,
    },
    /// The global configuration could not be loaded or is out of bounds (REQ-SAG-0013).
    Config(sc_agent_config::AgentConfigError),
    /// `--provider` or `--model` does not parse.
    Types(sc_agent_types::TypesError),
    /// The provider's or runner's key is missing or malformed.
    Secrets(sc_agent_secrets::SecretsError),
    /// A slug is invalid (`log.invalid_slug`), or repo detection or a logger open failed.
    Log(Box<sc_agent_log::LogError>),
    /// A local runner's backend could not be built.
    Local(Box<sc_decision_local::LocalError>),
    /// Building a System One backend, decoding the models list, or the local-runner load wait
    /// failed.
    Decision(Box<sc_decision_schema::DecisionError>),
    /// A models or doctor send failed.
    Http(sc_agent_http::HttpError),
    /// An LLM client could not be built or a provider model list could not be read.
    Llm(sc_llm_provider::LlmError),
    /// `flush_session_log` failed after the results were printed.
    Runner(Box<sc_decision_runner::RunnerError>),
    /// No provider in `Provider::ranking` for the model has a detected key, and no
    /// `--provider` was given.
    NoProviderKey {
        /// The model as given.
        model: String,
        /// The providers tried, in ranking order.
        tried: Vec<Provider>,
    },
    /// `--runner` (or `models(runner=...)`) names no configured local runner.
    UnknownRunner {
        /// The name given.
        name: String,
        /// The configured runner names.
        known: Vec<String>,
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
        /// The Debug text of the `io::ErrorKind` (display only).
        kind: String,
        /// The io error's Display text.
        detail: String,
    },
    /// A REQ-SAG-0014 input limit was exceeded; nothing was sent.
    LimitExceeded {
        /// `"request_bytes"`, `"batch_lines"`, `"batch_bytes"` or `"image_bytes"`.
        what: &'static str,
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
        /// The Debug text of the `io::ErrorKind` (display only).
        kind: String,
        /// The io error's Display text.
        detail: String,
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
        /// serde_json's message.
        detail: String,
    },
    /// A stdout write failed.
    Write {
        /// The Debug text of the `io::ErrorKind` (display only).
        kind: String,
        /// The io error's Display text.
        detail: String,
    },
    /// The interrupt handler could not be installed.
    Signal {
        /// ctrlc's error text.
        detail: String,
    },
}

impl DecideError {
    /// An io error reading `input`: `InputNotFound` for NotFound, else `InputRead`.
    pub(crate) fn input(input: &str, error: &std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::NotFound {
            Self::InputNotFound {
                input: input.to_owned(),
            }
        } else {
            Self::InputRead {
                input: input.to_owned(),
                kind: format!("{:?}", error.kind()),
                detail: error.to_string(),
            }
        }
    }

    /// A JSON encoding failure.
    pub(crate) fn encode(error: &serde_json::Error) -> Self {
        Self::Encode {
            detail: error.to_string(),
        }
    }
}

/// Boxed variants keep `DecideError` small (clippy `result_large_err`); the wrapped error is
/// unchanged inside its box.
macro_rules! boxed_from {
    ($($error:ty => $variant:ident),* $(,)?) => {
        $(impl From<$error> for DecideError {
            fn from(error: $error) -> Self {
                Self::$variant(Box::new(error))
            }
        })*
    };
}

boxed_from! {
    sc_agent_log::LogError => Log,
    sc_decision_local::LocalError => Local,
    sc_decision_schema::DecisionError => Decision,
    sc_decision_runner::RunnerError => Runner,
}

impl DecideError {
    /// `RequestInvalid` at `line` for `source`.
    #[must_use]
    pub fn request_invalid(line: Option<u64>, source: sc_decision_schema::DecisionError) -> Self {
        Self::RequestInvalid {
            line,
            source: Box::new(source),
        }
    }
}

/// `" (line N)"`, or nothing for a single request.
fn at_line(line: Option<u64>) -> String {
    line.map(|line| format!(" at line {line}"))
        .unwrap_or_default()
}

impl ErrorId for DecideError {
    fn id(&self) -> &'static str {
        match self {
            Self::Usage { .. } => "decide.usage",
            Self::Config(_) => "decide.config",
            Self::Types(_) => "decide.types",
            Self::Secrets(_) => "decide.secrets",
            Self::Log(_) => "decide.log",
            Self::Local(_) => "decide.local",
            Self::Decision(_) => "decide.decision",
            Self::Http(_) => "decide.http",
            Self::Llm(_) => "decide.llm",
            Self::Runner(_) => "decide.runner",
            Self::NoProviderKey { .. } => "decide.no_provider_key",
            Self::UnknownRunner { .. } => "decide.unknown_runner",
            Self::InputNotFound { .. } => "decide.input_not_found",
            Self::InputRead { .. } => "decide.input_read",
            Self::LimitExceeded { .. } => "decide.limit_exceeded",
            Self::LineNotUtf8 { .. } => "decide.line_not_utf8",
            Self::ImageRead { .. } => "decide.image_read",
            Self::ImageFormat { .. } => "decide.image_format",
            Self::RequestInvalid { .. } => "decide.request_invalid",
            Self::Encode { .. } => "decide.encode",
            Self::Write { .. } => "decide.write",
            Self::Signal { .. } => "decide.signal",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Usage { kind, detail } => {
                format!("invalid command line ({kind}): {}", detail.trim_end())
            }
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
                    "no provider key found for model {model:?}; tried {}",
                    tried.join(", ")
                )
            }
            Self::UnknownRunner { name, known } => format!(
                "{name:?} is not a configured local runner (configured: {})",
                if known.is_empty() {
                    "none".to_owned()
                } else {
                    known.join(", ")
                }
            ),
            Self::InputNotFound { input } => format!("input {input:?} does not exist"),
            Self::InputRead {
                input,
                kind,
                detail,
            } => format!("input {input:?} could not be read ({kind}): {detail}"),
            Self::LimitExceeded { what, line, max } => {
                format!("the `{what}` limit of {max} was exceeded{}", at_line(*line))
            }
            Self::LineNotUtf8 { line, byte_offset } => {
                format!("line {line} is not UTF-8 (invalid byte at offset {byte_offset})")
            }
            Self::ImageRead {
                name,
                path,
                kind,
                detail,
            } => format!(
                "image {name:?} ({}) could not be read ({kind}): {detail}",
                path.display()
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
            Self::Encode { detail } => format!("the output could not be encoded: {detail}"),
            Self::Write { kind, detail } => format!("stdout write failed ({kind}): {detail}"),
            Self::Signal { detail } => {
                format!("the interrupt handler could not be installed: {detail}")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
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
            Self::Usage { .. }
            | Self::NoProviderKey { .. }
            | Self::UnknownRunner { .. }
            | Self::InputNotFound { .. }
            | Self::InputRead { .. }
            | Self::LimitExceeded { .. }
            | Self::LineNotUtf8 { .. }
            | Self::ImageRead { .. }
            | Self::ImageFormat { .. }
            | Self::Encode { .. }
            | Self::Write { .. }
            | Self::Signal { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::Usage { .. } => Some("See decide --help."),
            Self::NoProviderKey { .. } => {
                Some("set one of the listed providers' key variables, or pass --provider")
            }
            Self::UnknownRunner { .. } => {
                Some("name a key of `local_runners` in the global configuration")
            }
            Self::LimitExceeded { .. } => Some(
                "keep one request under 8 MiB, a batch under 10,000 lines and 256 MiB, and each image under 20 MiB",
            ),
            Self::ImageFormat { .. } => Some("pass a .png, .jpg or .jpeg image"),
            Self::RequestInvalid { source, .. } => source.recovery(),
            _ => None,
        }
    }
}

impl fmt::Display for DecideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for DecideError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Types(error) => Some(error),
            Self::Secrets(error) => Some(error),
            Self::Log(error) => Some(&**error),
            Self::Local(error) => Some(&**error),
            Self::Decision(error) | Self::RequestInvalid { source: error, .. } => Some(&**error),
            Self::Http(error) => Some(error),
            Self::Llm(error) => Some(error),
            Self::Runner(error) => Some(&**error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `detail()` is built from id, message, recovery and cause_detail (the ErrorId contract).
    #[test]
    fn detail_matches_the_four_parts() {
        let errors = [
            DecideError::NoProviderKey {
                model: "jev".into(),
                tried: vec![Provider::TypeSafe],
            },
            DecideError::Types(sc_agent_types::TypesError::UnknownProvider { text: "x".into() }),
            DecideError::LimitExceeded {
                what: "request_bytes",
                line: Some(3),
                max: 8,
            },
        ];
        for error in errors {
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

    #[test]
    fn no_provider_key_has_a_recovery() {
        let error = DecideError::NoProviderKey {
            model: "jev".into(),
            tried: vec![Provider::TypeSafe],
        };
        assert_eq!(error.id(), "decide.no_provider_key");
        assert!(
            error
                .recovery()
                .is_some_and(|text| text.contains("--provider"))
        );
        assert!(error.message().contains("typesafe"));
    }
}
