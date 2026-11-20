//! Python binding error IDs and categories.

use sc_agent_types::{ErrorCategory, ErrorDetail, ErrorId};

#[derive(Debug)]
pub(crate) enum PyBindingError {
    WorkerPanicked { message: String },
    InvalidOption { option: String, bound: String },
    InvalidResult { message: String },
    EncodeResult { message: String },
}

sc_agent_types::error_catalog! { PyBindingError {
    PyBindingError::WorkerPanicked { .. } => ("sc_agent_py.worker_panicked", Some(ErrorCategory::Other)),
    PyBindingError::InvalidOption { .. } => ("sc_agent_py.invalid_option", Some(ErrorCategory::Validation)),
    PyBindingError::InvalidResult { .. } => ("sc_agent_py.invalid_result", Some(ErrorCategory::Validation)),
    PyBindingError::EncodeResult { .. } => ("sc_agent_py.encode_result", Some(ErrorCategory::Other)),
} }

impl std::fmt::Display for PyBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.id(), self.message())
    }
}

impl std::error::Error for PyBindingError {}

impl ErrorId for PyBindingError {
    fn id(&self) -> &'static str {
        self.catalog_row().0
    }
    fn base_category(&self) -> Option<ErrorCategory> {
        self.catalog_row().1
    }
    fn message(&self) -> String {
        match self {
            Self::WorkerPanicked { message } => message.clone(),
            Self::InvalidOption { option, bound } => format!("{option} {bound}"),
            Self::InvalidResult { message } | Self::EncodeResult { message } => message.clone(),
        }
    }
    fn cause_detail(&self) -> Option<ErrorDetail> {
        None
    }
    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::InvalidOption { .. } => Some("Correct the named option and retry."),
            Self::WorkerPanicked { .. } => None,
            Self::InvalidResult { .. } => Some("Correct the result JSON and retry."),
            Self::EncodeResult { .. } => None,
        }
    }
}
