//! Temp directories for crate tests.

use std::fs;
use std::path::Path;

use sc_agent_types::RepoRoot;

/// Isolated repo root removed when the value drops.
pub(crate) struct TempRepo {
    dir: tempfile::TempDir,
}

impl TempRepo {
    /// Creates an empty directory and, when `dotenv` is set, writes `.env`.
    pub(crate) fn new(dotenv: Option<&str>) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        if let Some(body) = dotenv {
            fs::write(dir.path().join(".env"), body).expect("write .env");
        }
        Self { dir }
    }

    /// Returns the directory as a [`RepoRoot`].
    pub(crate) fn root(&self) -> RepoRoot {
        RepoRoot::new(self.dir.path())
    }

    /// Returns the directory path.
    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }
}
