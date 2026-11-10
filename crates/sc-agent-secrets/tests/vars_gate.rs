//! REQ-SAG-0401 gate: credential variable names are string literals only in sc-agent-secrets.
//!
//! Walks the workspace `crates/` tree and fails on any quoted [`ALL_VARS`]
//! name in a file outside `crates/sc-agent-secrets` (build output under
//! `target/` is skipped). Other crates name a credential through the
//! constants or [`env_var`](sc_agent_secrets::env_var).

use std::fs;
use std::path::{Path, PathBuf};

use sc_agent_secrets::ALL_VARS;

/// Every `(file, name)` where `name` from [`ALL_VARS`] appears as `"name"` under `root`,
/// skipping the `exempt` directory and any `target` directory.
fn quoted_names(root: &Path, exempt: &Path) -> std::io::Result<Vec<(PathBuf, &'static str)>> {
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
            for name in ALL_VARS {
                if text.contains(&format!("\"{name}\"")) {
                    found.push((path.clone(), *name));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

#[test]
fn credential_names_are_literals_only_in_sc_agent_secrets() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates = manifest.parent().unwrap();
    assert_eq!(crates.file_name().unwrap(), "crates");
    let found = quoted_names(crates, manifest).unwrap();
    assert!(
        found.is_empty(),
        "credential variable names outside crates/sc-agent-secrets: {found:?}"
    );
}

/// The gate is not vacuous: a planted literal in another crate is found, the exempt crate is not.
#[test]
fn the_gate_finds_a_planted_literal() {
    let root = tempfile::tempdir().unwrap();
    let exempt = root.path().join("sc-agent-secrets");
    let other = root.path().join("other").join("src");
    fs::create_dir_all(&exempt).unwrap();
    fs::create_dir_all(&other).unwrap();
    for name in ALL_VARS {
        fs::write(
            exempt.join("vars.rs"),
            format!("const A: &str = \"{name}\";"),
        )
        .unwrap();
        fs::write(other.join("lib.rs"), format!("let v = \"{name}\";")).unwrap();
        let found = quoted_names(root.path(), &exempt).unwrap();
        assert_eq!(found, [(other.join("lib.rs"), *name)]);
    }
}
