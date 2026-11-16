//! Python binding error IDs and categories.

use sc_agent_types::{ErrorCategory, ErrorDetail, ErrorId};

#[derive(Debug)]
pub(crate) enum PyBindingError {
    WorkerPanicked { message: String },
    InvalidOption { option: String, bound: String },
}

sc_agent_types::error_catalog! { PyBindingError {
    PyBindingError::WorkerPanicked { .. } => ("sc_agent_py.worker_panicked", Some(ErrorCategory::Other)),
    PyBindingError::InvalidOption { .. } => ("sc_agent_py.invalid_option", Some(ErrorCategory::Validation)),
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
        }
    }
    fn cause_detail(&self) -> Option<ErrorDetail> {
        None
    }
    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::InvalidOption { .. } => Some("Correct the named option and retry."),
            Self::WorkerPanicked { .. } => None,
        }
    }
}
