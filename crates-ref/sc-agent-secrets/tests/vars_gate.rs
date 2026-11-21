//! REQ-SAG-0401 gate: credential variable names are string literals only in sc-agent-secrets.
//!
//! Walks the workspace `crates/` tree and fails on any [`ALL_VARS`] name used
//! as text in a file outside `crates/sc-agent-secrets` (build output under
//! `target/` is skipped). Other crates name a credential through the
//! constants (`sc_agent_secrets::OPENAI_API_KEY`) or
//! [`env_var`](sc_agent_secrets::env_var).
//!
//! - In `.rs` files every string literal (`"..."`, `r#"..."#`, `b"..."`) is
//!   searched, so a name inside a longer string (`"export NAME=x"`, escaped
//!   JSON `"{\"k\":\"NAME\"}"`) is caught. Comments and code (a
//!   `sc_agent_secrets::NAME` path, a `use sc_agent_secrets::{NAME}` list)
//!   are not literals and pass.
//! - In every other text file (Python, shell, JSON, TOML, Markdown) any
//!   whole-word occurrence is caught unless it is written as a `::NAME` path.

use std::fs;
use std::path::{Path, PathBuf};

use sc_agent_secrets::ALL_VARS;

/// Every `(file, name)` under `root` that uses a credential name as text,
/// skipping the `exempt` directory and any `target` directory.
fn credential_literals(
    root: &Path,
    exempt: &Path,
) -> std::io::Result<Vec<(PathBuf, &'static str)>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path == exempt || path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            // Non-text files cannot hold a source literal.
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let is_rust = path.extension().is_some_and(|ext| ext == "rs");
            for name in ALL_VARS {
                let hit = if is_rust {
                    rust_string_literals(&text)
                        .iter()
                        .any(|literal| has_word(literal, name, false))
                } else {
                    has_word(&text, name, true)
                };
                if hit {
                    found.push((path.clone(), *name));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Whether `name` occurs in `text` as a whole word (neighbours not `[A-Za-z0-9_]`).
///
/// With `allow_paths`, an occurrence written as `::name` is not counted.
fn has_word(text: &str, name: &str, allow_paths: bool) -> bool {
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    text.match_indices(name).any(|(start, _)| {
        let before = text.get(..start).unwrap_or_default();
        let after = text.get(start + name.len()..).unwrap_or_default();
        let bounded = !before.bytes().next_back().is_some_and(is_word)
            && !after.bytes().next().is_some_and(is_word);
        bounded && !(allow_paths && before.ends_with("::"))
    })
}

/// The contents of every string literal in Rust source `text` (escapes kept as written).
///
/// A small lexer: it skips `//` and `/* */` comments and char literals
/// (`'"'`, `'\''`), tells lifetimes apart from char literals, and reads
/// normal, byte and raw (`r#"..."#`) strings.
fn rust_string_literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let at = |index: usize| chars.get(index).copied();
    let mut literals = Vec::new();
    let mut index = 0;
    while let Some(ch) = at(index) {
        match ch {
            '/' if at(index + 1) == Some('/') => {
                while at(index).is_some_and(|c| c != '\n') {
                    index += 1;
                }
            }
            '/' if at(index + 1) == Some('*') => {
                index += 2;
                while at(index).is_some() && !(at(index) == Some('*') && at(index + 1) == Some('/'))
                {
                    index += 1;
                }
                index += 2;
            }
            '\'' => {
                // A char literal is 'x' or '\..'; anything else is a lifetime.
                if at(index + 1) == Some('\\') {
                    index += 2;
                    while at(index).is_some_and(|c| c != '\'') {
                        index += 1;
                    }
                    index += 1;
                } else if at(index + 2) == Some('\'') {
                    index += 3;
                } else {
                    index += 1;
                }
            }
            'r' if matches!(at(index + 1), Some('"' | '#'))
                && !index
                    .checked_sub(1)
                    .and_then(at)
                    .is_some_and(|c| c.is_alphanumeric() || c == '_') =>
            {
                let mut cursor = index + 1;
                let mut hashes = 0;
                while at(cursor) == Some('#') {
                    hashes += 1;
                    cursor += 1;
                }
                if at(cursor) != Some('"') {
                    index += 1;
                    continue;
                }
                cursor += 1;
                let start = cursor;
                let closes = |pos: usize| {
                    at(pos) == Some('"') && (1..=hashes).all(|offset| at(pos + offset) == Some('#'))
                };
                while at(cursor).is_some() && !closes(cursor) {
                    cursor += 1;
                }
                literals.push(
                    chars
                        .get(start..cursor)
                        .unwrap_or_default()
                        .iter()
                        .collect(),
                );
                index = cursor + 1 + hashes;
            }
            '"' => {
                let start = index + 1;
                let mut cursor = start;
                while let Some(c) = at(cursor) {
                    match c {
                        '\\' => cursor += 2,
                        '"' => break,
                        _ => cursor += 1,
                    }
                }
                literals.push(
                    chars
                        .get(start..cursor.min(chars.len()))
                        .unwrap_or_default()
                        .iter()
                        .collect(),
                );
                index = cursor + 1;
            }
            _ => index += 1,
        }
    }
    literals
}

#[test]
fn credential_names_are_literals_only_in_sc_agent_secrets() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates = manifest.parent().unwrap();
    assert_eq!(crates.file_name().unwrap(), "crates");
    let found = credential_literals(crates, manifest).unwrap();
    assert!(
        found.is_empty(),
        "credential variable names outside crates/sc-agent-secrets: {found:?}"
    );
}

/// The gate is not vacuous: every planted form is found; paths, comments and the exempt crate pass.
#[test]
fn the_gate_finds_planted_literals_and_passes_paths() {
    for name in ALL_VARS {
        let flagged_rust = [
            format!(r#"let v = "{name}";"#),
            format!(r#"let v = "export {name}=x";"#),
            format!(r#"let v = "{{\"key_env\":\"{name}\"}}";"#),
            format!(r##"let v = r#"{name}"#;"##),
            format!(r#"let v = b"{name}";"#),
            format!(r#"let q = '"'; let v = "{name}";"#),
            format!(r#"fn f<'a>(x: &'a str) {{ g("{name}") }}"#),
        ];
        let passing_rust = [
            format!("let v = sc_agent_secrets::{name};"),
            format!("use sc_agent_secrets::{{{name}, env_var}};"),
            format!("// reads {name}\nlet v = 1;"),
            format!("/* \"{name}\" */ let v = 1;"),
            format!(r#"let v = "{name}_SUFFIX";"#),
        ];
        let flagged_text = [
            ("py", format!("key = os.environ['{name}']")),
            ("sh", format!("export {name}=x")),
            ("json", format!("{{\"key_env\": \"{name}\"}}")),
            ("md", format!("set `{name}` first")),
        ];
        let passing_text = [("md", format!("use `sc_agent_secrets::{name}`"))];

        for (source, expect_hit) in flagged_rust
            .iter()
            .map(|text| (("rs", text.clone()), true))
            .chain(
                passing_rust
                    .iter()
                    .map(|text| (("rs", text.clone()), false)),
            )
            .chain(flagged_text.iter().map(|pair| (pair.clone(), true)))
            .chain(passing_text.iter().map(|pair| (pair.clone(), false)))
        {
            let (ext, text) = source;
            let root = tempfile::tempdir().unwrap();
            let exempt = root.path().join("sc-agent-secrets");
            let other = root.path().join("other").join("src");
            fs::create_dir_all(&exempt).unwrap();
            fs::create_dir_all(&other).unwrap();
            fs::write(exempt.join(format!("vars.{ext}")), &text).unwrap();
            let file = other.join(format!("lib.{ext}"));
            fs::write(&file, &text).unwrap();
            let found = credential_literals(root.path(), &exempt).unwrap();
            if expect_hit {
                assert_eq!(found, [(file, *name)], "should be caught: {text}");
            } else {
                assert!(found.is_empty(), "should pass: {text} -> {found:?}");
            }
        }
    }
}
