//! Repo-root `.env` parsing.
//!
//! The parser never writes the process environment. A missing file yields no
//! variables. Callers decide whether a parsed value may override a variable
//! that is already set.
//!
//! # Dialect
//!
//! One `KEY=value` per line, optionally prefixed by `export `. Blank lines
//! and lines starting with `#` are skipped, as are lines without `=` or
//! with a key that is not `[A-Za-z_][A-Za-z0-9_]*`. A later line wins.
//!
//! - Unquoted values are trimmed. A `#` preceded by whitespace starts a
//!   comment that runs to the end of the line (`KEY=v # note` is `v`); a `#`
//!   with no whitespace before it is part of the value (`KEY=v#1` is `v#1`).
//! - `'single'` quotes keep the text verbatim.
//! - `"double"` quotes unescape `\n`, `\r`, `\t`, and `\<char>`.
//! - A quoted value may be followed by whitespace and a `#` comment.
//! - There is no variable expansion and no multi-line value.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::error::SecretsError;

/// Largest `.env` read: 1 MiB.
///
/// A credential file is a few hundred bytes; the cap only stops a repository
/// from making every key lookup read (and hold) an arbitrarily large file.
pub(crate) const MAX_DOTENV_BYTES: u64 = 1024 * 1024;

/// Reads `<repo_root>/.env`; a missing file yields no variables.
///
/// The path must be a regular file (after following symlinks) of at most
/// [`MAX_DOTENV_BYTES`]; the read itself is bounded too, so a file that grows
/// after the size check is still refused.
///
/// # Errors
///
/// Returns [`SecretsError::DotenvRead`] when the file exists and cannot be
/// read, is not a regular file, or is larger than [`MAX_DOTENV_BYTES`]. The
/// error holds a sanitized label, never the OS message.
pub(crate) fn load_dotenv(repo_root: &Path) -> Result<BTreeMap<String, String>, SecretsError> {
    let path = repo_root.join(".env");
    // Check the kind before opening: Windows refuses to open a directory
    // (PermissionDenied), which would hide the real reason.
    match fs::metadata(&path) {
        Ok(metadata) if !metadata.is_file() => return Err(not_a_file()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(dotenv_read_error(&error)),
    }
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        // Removed between the two calls: as if it was never there.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(dotenv_read_error(&error)),
    };
    // Backstop: the opened file is checked again (it may have been replaced).
    let metadata = file.metadata().map_err(|error| dotenv_read_error(&error))?;
    if !metadata.is_file() {
        return Err(not_a_file());
    }
    if metadata.len() > MAX_DOTENV_BYTES {
        return Err(too_large());
    }
    let mut text = String::new();
    file.take(MAX_DOTENV_BYTES.saturating_add(1))
        .read_to_string(&mut text)
        .map_err(|error| dotenv_read_error(&error))?;
    if u64::try_from(text.len()).map_or(true, |len| len > MAX_DOTENV_BYTES) {
        return Err(too_large());
    }
    Ok(parse_dotenv(&text))
}

fn not_a_file() -> SecretsError {
    SecretsError::DotenvRead {
        kind: "NotAFile".to_owned(),
        cause: "path is not a regular file",
    }
}

fn too_large() -> SecretsError {
    SecretsError::DotenvRead {
        kind: "FileTooLarge".to_owned(),
        cause: "file is larger than 1 MiB",
    }
}

/// The sanitized error for an unreadable `.env`.
fn dotenv_read_error(error: &io::Error) -> SecretsError {
    SecretsError::DotenvRead {
        kind: format!("{:?}", error.kind()),
        cause: sanitized_io_cause(error),
    }
}

/// Labels an I/O failure without the OS message.
///
/// `std::io::Error`'s display text can echo path or payload bytes. Callers
/// pass this label as the sanitized cause.
fn sanitized_io_cause(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "permission denied",
        io::ErrorKind::IsADirectory => "path is a directory",
        io::ErrorKind::InvalidData => "contents are not valid text",
        io::ErrorKind::NotFound => "not found",
        _ => "read failed",
    }
}

pub(crate) fn parse_dotenv(text: &str) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = strip_export(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_env_name(key) {
            continue;
        }
        vars.insert(key.to_owned(), parse_value(value));
    }
    vars
}

fn strip_export(line: &str) -> &str {
    line.strip_prefix("export ")
        .or_else(|| line.strip_prefix("export\t"))
        .map_or(line, str::trim_start)
}

fn is_env_name(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Decodes one raw value (everything after the first `=`).
fn parse_value(raw: &str) -> String {
    let value = raw.trim();
    if let Some(quoted) = quoted_value(value) {
        return quoted;
    }
    strip_inline_comment(raw).trim().to_owned()
}

/// Returns the decoded text when `value` is one quoted string, optionally
/// followed by whitespace and a `#` comment.
fn quoted_value(value: &str) -> Option<String> {
    let quote = value
        .chars()
        .next()
        .filter(|ch| *ch == '"' || *ch == '\'')?;
    let body = value.get(quote.len_utf8()..)?;
    let mut escaped = false;
    let close = body.char_indices().find_map(|(index, ch)| {
        if quote == '"' && escaped {
            escaped = false;
            return None;
        }
        if quote == '"' && ch == '\\' {
            escaped = true;
            return None;
        }
        (ch == quote).then_some(index)
    })?;
    let rest = body.get(close + quote.len_utf8()..)?.trim_start();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return None;
    }
    let inner = body.get(..close)?;
    Some(if quote == '"' {
        unescape_double(inner)
    } else {
        inner.to_owned()
    })
}

/// Cuts an unquoted value at the first `#` that follows whitespace.
fn strip_inline_comment(raw: &str) -> &str {
    let mut previous_is_space = false;
    for (index, ch) in raw.char_indices() {
        if ch == '#' && previous_is_space {
            // `index` is the byte offset of a `#`, so it is a char boundary.
            return raw.get(..index).unwrap_or(raw);
        }
        previous_is_space = ch.is_whitespace();
    }
    raw
}

fn unescape_double(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_export_quotes_comments_and_keeps_last_value() {
        let text = "\u{feff}# comment\n\nexport TYPESAFE_API_KEY=first\nTYPESAFE_API_KEY=\"sec ond\"\nOPENAI_API_KEY='keep#me'\nANTHROPIC_API_KEY=\"a\\nb\"\nnot a pair\n1BAD=no\n";
        let vars = parse_dotenv(text);
        assert_eq!(
            vars.get("TYPESAFE_API_KEY").map(String::as_str),
            Some("sec ond")
        );
        assert_eq!(
            vars.get("OPENAI_API_KEY").map(String::as_str),
            Some("keep#me")
        );
        assert_eq!(
            vars.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some("a\nb")
        );
        assert!(!vars.contains_key("1BAD"));
    }

    #[test]
    fn unquoted_trailing_comment_is_stripped() {
        let text = concat!(
            "KEY=v # c\n",
            "QUOTED=\"v # not comment\"\n",
            "GLUED=v#notcomment\n",
            "TAB=v\t# c\n",
            "ONLY= # nothing\n",
            "HASHFIRST=#kept\n",
            "SINGLE='a # b' # c\n",
            "DOUBLE=\"x\\\"y\" # c\n",
            "TRAILING=\"x\" junk\n",
        );
        let vars = parse_dotenv(text);
        let get = |key: &str| vars.get(key).map(String::as_str);
        assert_eq!(get("KEY"), Some("v"));
        assert_eq!(get("QUOTED"), Some("v # not comment"));
        assert_eq!(get("GLUED"), Some("v#notcomment"));
        assert_eq!(get("TAB"), Some("v"));
        assert_eq!(get("ONLY"), Some(""));
        assert_eq!(get("HASHFIRST"), Some("#kept"));
        assert_eq!(get("SINGLE"), Some("a # b"));
        assert_eq!(get("DOUBLE"), Some("x\"y"));
        assert_eq!(get("TRAILING"), Some("\"x\" junk"));
    }

    #[test]
    fn missing_file_is_empty() {
        let repo = tempfile::tempdir().unwrap();
        let loaded = load_dotenv(repo.path()).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn a_directory_or_an_oversized_file_is_refused() {
        use sc_agent_types::ErrorId;

        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join(".env")).unwrap();
        let error = load_dotenv(repo.path()).unwrap_err();
        assert_eq!(error.id(), "secrets.dotenv_read");
        assert!(error.message().contains("not a regular file"), "{error}");

        let repo = tempfile::tempdir().unwrap();
        let at_cap = "#".repeat(usize::try_from(MAX_DOTENV_BYTES).unwrap());
        fs::write(repo.path().join(".env"), format!("{at_cap}\n")).unwrap();
        let error = load_dotenv(repo.path()).unwrap_err();
        assert!(error.message().contains("larger than 1 MiB"), "{error}");

        fs::write(repo.path().join(".env"), &at_cap).unwrap();
        assert!(
            load_dotenv(repo.path()).unwrap().is_empty(),
            "exactly 1 MiB loads"
        );
    }

    #[test]
    fn unreadable_cause_omits_raw_error_text() {
        use sc_agent_types::ErrorId;

        let secret = "super-secret-value";
        let error = io::Error::new(io::ErrorKind::PermissionDenied, secret);
        let rendered = dotenv_read_error(&error);
        assert_eq!(rendered.id(), "secrets.dotenv_read");
        let text = format!("{rendered} {rendered:?} {}", rendered.message());
        assert!(text.contains("permission denied"), "{text}");
        assert!(!text.contains(secret), "{text}");
    }
}
