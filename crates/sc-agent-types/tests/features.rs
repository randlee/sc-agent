//! Module seams are cargo features (REQ-SAG-0107, ADR-SAG-0027).
//!
//! `cargo tree -p sc-agent-types --no-default-features -e normal` is pinned
//! against `tests/fixtures/no-default-features-deps.txt` (crate names only,
//! so a patch bump does not churn it; `--target all`, so the list does not
//! depend on the host platform). `cargo tree` runs `--offline` through the
//! workspace's bounded runner, so it needs a warm registry cache (any prior
//! `cargo build` or `cargo test` of the workspace provides one). `sha2` is a core dependency because
//! `Sha256Digest::of` is in core `ids`.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

#[path = "../../../tests/support/deadline.rs"]
mod deadline;

fn tree(args: &[&str]) -> BTreeSet<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "tree",
            "-p",
            "sc-agent-types",
            "-e",
            "normal",
            "--prefix",
            "none",
        ])
        .args(["--locked", "--offline", "--target", "all"])
        .args(args);
    let output = deadline::spawn_with_deadline(&mut command, None);
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn test_no_default_features_dep_set() {
    let pinned: BTreeSet<String> = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/no-default-features-deps.txt"),
    )
    .expect("pinned list")
    .lines()
    .map(str::to_owned)
    .collect();
    let actual = tree(&["--no-default-features"]);
    assert_eq!(actual, pinned);
    assert!(actual.contains("sha2"));
}

#[test]
fn test_no_forbidden_crate_in_any_normal_tree() {
    for args in [
        &[][..],
        &["--no-default-features"][..],
        &["--all-features"][..],
    ] {
        let names = tree(args);
        for forbidden in [
            "sc-observability",
            "sc-observability-types",
            "ureq",
            "pyo3",
            "tree-sitter",
        ] {
            assert!(!names.contains(forbidden), "{forbidden} with {args:?}");
        }
    }
}
