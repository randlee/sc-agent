//! Reading a source file under the crate's size limit.

use std::io::Read;
use std::path::Path;

use crate::Error;

/// The largest source [`crate::run_query`] and [`read_source`] accept: 8 MiB.
///
/// The bound also keeps every row below 8 Mi, so 1-based line numbers never overflow.
pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Reads the file at `path`, refusing files over [`MAX_SOURCE_BYTES`].
///
/// At most `MAX_SOURCE_BYTES + 1` bytes are read, so a file that grows past the limit after its
/// size check is still never loaded whole.
///
/// # Errors
///
/// [`Error::FileNotFound`] when the file does not exist; [`Error::FileRead`] for any other
/// I/O error, or when `path` is not a regular file (kind `IsADirectory` for a directory,
/// `InvalidInput` otherwise); [`Error::InputTooLarge`] when the file is over the limit, checked
/// from its metadata before any read.
pub fn read_source(path: &Path) -> Result<Vec<u8>, Error> {
    let read_error = |error: std::io::Error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::FileNotFound {
                path: path.to_path_buf(),
            }
        } else {
            Error::FileRead {
                path: path.to_path_buf(),
                kind: error.kind(),
                detail: error.to_string(),
            }
        }
    };
    let file = std::fs::File::open(path).map_err(read_error)?;
    let metadata = file.metadata().map_err(read_error)?;
    if !metadata.is_file() {
        let kind = if metadata.is_dir() {
            std::io::ErrorKind::IsADirectory
        } else {
            std::io::ErrorKind::InvalidInput
        };
        return Err(Error::FileRead {
            path: path.to_path_buf(),
            kind,
            detail: "not a regular file".to_owned(),
        });
    }
    let limit = MAX_SOURCE_BYTES as u64;
    let too_large = |bytes: u64| Error::InputTooLarge {
        path: Some(path.to_path_buf()),
        bytes,
        limit,
    };
    if metadata.len() > limit {
        return Err(too_large(metadata.len()));
    }
    // Backstop: the file may grow between `metadata` and the read.
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(too_large(bytes.len() as u64));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC: REQ-SAG-0002 errors.
    #[test]
    fn missing_file_and_directory_are_typed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("absent.py");
        assert_eq!(
            read_source(&missing),
            Err(Error::FileNotFound {
                path: missing.clone()
            })
        );
        let error = read_source(dir.path()).expect_err("a directory is not a file");
        assert_eq!(error.id(), "treesitter.file_read");
        // Windows refuses to open a directory at all (PermissionDenied from `File::open`);
        // Unix opens it and the regular-file check rejects it.
        #[cfg(unix)]
        assert_eq!(
            error,
            Error::FileRead {
                path: dir.path().to_path_buf(),
                kind: std::io::ErrorKind::IsADirectory,
                detail: "not a regular file".to_owned(),
            }
        );
        assert!(matches!(error, Error::FileRead { ref path, .. } if path == dir.path()));
    }

    #[test]
    fn small_file_reads_and_large_file_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let small = dir.path().join("small.py");
        std::fs::write(&small, b"x = 1\n").expect("write");
        assert_eq!(read_source(&small).expect("read"), b"x = 1\n");

        let large = dir.path().join("large.py");
        std::fs::write(&large, vec![b'#'; MAX_SOURCE_BYTES + 3]).expect("write");
        assert_eq!(
            read_source(&large),
            Err(Error::InputTooLarge {
                path: Some(large.clone()),
                bytes: (MAX_SOURCE_BYTES + 3) as u64,
                limit: MAX_SOURCE_BYTES as u64,
            })
        );
    }
}
