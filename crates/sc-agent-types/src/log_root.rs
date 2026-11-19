//! Shared default log-root location.

use std::path::{Path, PathBuf};

/// Returns the default sc-agent log root under `home`.
#[must_use]
pub fn default_log_root(home: &Path) -> PathBuf {
    home.join(".sc-agent")
}

/// The log root of a CLI: its `<CLI>_LOG_ROOT` test override when set and non-empty, else
/// [`default_log_root`] under `home`; `None` when neither is known.
#[must_use]
pub fn cli_log_root(
    override_root: Option<&std::ffi::OsStr>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    match override_root.filter(|root| !root.is_empty()) {
        Some(root) => Some(PathBuf::from(root)),
        None => home.map(|home| default_log_root(&home)),
    }
}

/// Resolves the process home, rejecting an explicitly empty HOME (USERPROFILE on Windows).
/// An unset variable may use the platform's standard home lookup.
#[must_use]
pub fn resolve_home() -> Option<PathBuf> {
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    resolve_home_with(std::env::var_os(home_var).as_deref(), || {
        // The platform lookup supplies the documented fallback when the variable is unset.
        #[allow(deprecated)]
        std::env::home_dir()
    })
}

/// Applies the home rule to controlled inputs, including an injected platform lookup.
#[must_use]
pub fn resolve_home_with(
    home: Option<&std::ffi::OsStr>,
    lookup: impl FnOnce() -> Option<PathBuf>,
) -> Option<PathBuf> {
    if home.is_some_and(std::ffi::OsStr::is_empty) {
        return None;
    }
    lookup().filter(|path| !path.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn home_rule_uses_only_the_injected_lookup() {
        assert_eq!(
            resolve_home_with(Some(OsStr::new("")), || panic!(
                "empty home must not fall back"
            )),
            None
        );
        assert_eq!(resolve_home_with(None, || None), None);
        assert_eq!(resolve_home_with(None, || Some(PathBuf::new())), None);
        let home = PathBuf::from("controlled-home");
        assert_eq!(resolve_home_with(None, || Some(home.clone())), Some(home));
    }
}
