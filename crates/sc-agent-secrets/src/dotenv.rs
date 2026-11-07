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
use std::io;
use std::path::Path;

use sc_agent_types::JevError;

pub(crate) const DOTENV_UNREADABLE: &str = "repo-root .env could not be read";

pub(crate) const DOTENV_RECOVERY: &str = "Make the repo-root .env file readable by this user, or remove it, then retry. Do not paste secret values into logs or this message.";

pub(crate) fn load_dotenv(repo_root: &Path) -> Result<BTreeMap<String, String>, JevError> {
    let path = repo_root.join(".env");
    match fs::read_to_string(&path) {
        Ok(text) => Ok(parse_dotenv(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(JevError::credential_store_invalid(
            DOTENV_UNREADABLE,
            sanitized_io_cause(&error),
            DOTENV_RECOVERY,
        )),
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
    let body = &value[1..];
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
    let rest = body[close + 1..].trim_start();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return None;
    }
    let inner = &body[..close];
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
            return &raw[..index];
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
        let repo = crate::test_support::TempRepo::new(None);
        let loaded = load_dotenv(repo.path()).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn unreadable_cause_omits_raw_error_text() {
        let secret = "super-secret-value";
        let error = io::Error::new(io::ErrorKind::PermissionDenied, secret);
        let rendered = JevError::credential_store_invalid(
            DOTENV_UNREADABLE,
            super::sanitized_io_cause(&error),
            DOTENV_RECOVERY,
        );
        let text = format!("{rendered} {rendered:?} {}", rendered.fix_hint());
        assert!(text.contains("cause: permission denied"), "{text}");
        assert!(text.contains(DOTENV_RECOVERY), "{text}");
        assert!(!text.contains(secret), "{text}");
        assert!(!rendered.fix_hint().contains("check.toml"));
    }
}
