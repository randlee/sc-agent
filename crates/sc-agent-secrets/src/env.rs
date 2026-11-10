/// Reads named variables.
///
/// `None` means unset. `Some` means set, including an empty string. The trait
/// is an open extension point (ADR-SAG-0020) so tests can inject values
/// without changing the process environment. It is intentionally not sealed.
pub trait EnvSource: Send + Sync {
    /// Returns the variable named `name` when it is set.
    fn var(&self, name: &str) -> Option<String>;
}
/// Reads variables from the process environment.
///
/// A missing variable is `None`. A present empty string is `Some("")`, so
/// repo-root `.env` does not override it. Non-Unicode values are treated as
/// present and empty.
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
