//! Shared default log-root location.

use std::path::{Path, PathBuf};

/// Returns the default sc-agent log root under `home`.
#[must_use]
pub fn default_log_root(home: &Path) -> PathBuf {
    home.join(".sc-agent")
}
