#![allow(clippy::unwrap_used)]

use sc_agent_log::Session;

#[test]
fn slugs_are_checked() {
    let root = std::env::temp_dir();
    assert!(Session::open(&root, "x".into(), "Bad Name", "judge").is_err());
    assert!(Session::open(&root, "x".into(), "ok", "a--b").is_err());
}
