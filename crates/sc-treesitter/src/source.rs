//! Reading a source file for a tree-sitter query.

use std::path::Path;

use crate::Error;

/// The largest source [`crate::run_query`] accepts: 8 MiB.
///
/// The bound also keeps every row below 8 Mi, so 1-based line numbers never overflow.
pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Reads the file at `path`.
///
/// # Errors
///
/// [`Error::FileNotFound`] when the file does not exist; [`Error::FileRead`] for any other I/O
/// error.
pub fn read_source(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => Error::FileNotFound {
            path: path.to_path_buf(),
        },
        kind => Error::FileRead {
            path: path.to_path_buf(),
            kind,
            detail: error.to_string(),
        },
    })
}
