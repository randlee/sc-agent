//! Workspace test: Rust canonical bytes match the committed Python reference.

use std::path::PathBuf;
use std::process::Command;

use sc_agent_types::{Sha256Digest, canonical_json_bytes};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Matches the justfile: `python` on Windows, `python3` elsewhere.
fn python_program() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}

#[test]
fn jev_cross_impl_matches_python_reference() {
    let root = repo_root();
    let fixture = root.join("tests/fixtures/jev/cross_impl");
    let logical = std::fs::read_to_string(fixture.join("logical.json")).expect("logical.json");
    let value: serde_json::Value = serde_json::from_str(&logical).expect("parse logical json");
    let rust_bytes = canonical_json_bytes(&value).expect("canonicalize");

    let python = Command::new(python_program())
        .arg(fixture.join("canonicalize.py"))
        .output()
        .expect("run python reference");
    assert!(
        python.status.success(),
        "python reference failed: {}",
        String::from_utf8_lossy(&python.stderr)
    );

    let reference_bytes = std::fs::read(fixture.join("reference.bytes")).expect("reference.bytes");
    let reference_digest =
        std::fs::read_to_string(fixture.join("reference.sha256")).expect("reference.sha256");
    let rust_digest = Sha256Digest::of(&rust_bytes);

    assert_eq!(rust_bytes, python.stdout, "rust bytes differ from python");
    assert_eq!(
        rust_bytes, reference_bytes,
        "rust bytes differ from committed reference"
    );
    assert_eq!(rust_digest.as_str(), reference_digest.trim());
    println!("jev_cross_impl sha256={}", rust_digest.as_str());
}
