//! Environment-variable sources.

/// Reads named variables.
///
/// `None` means unset. `Some` means set, including an empty string. The trait
/// is open so callers and tests can inject values without changing the
/// process environment.
pub trait EnvSource: Send + Sync {
    /// Returns the variable named `name` when it is set.
    fn var(&self, name: &str) -> Option<String>;
}

/// Reads variables from the process environment.
///
/// A missing variable is `None`; a present empty string is `Some("")`.
/// Resolution checks repo-root `.env` first, then the process environment,
/// then the keyring. A selected empty value is rejected without falling through.
/// Non-Unicode values are treated as present and empty.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        match std::env::var(name) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => Some(String::new()),
        }
    }
}
