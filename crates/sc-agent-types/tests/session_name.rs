//! Portable session-name policy, independent of filesystem and host OS.
use sc_agent_types::{MAX_SEGMENT_BYTES, is_path_safe, is_valid_slug};

#[test]
fn session_slugs_are_lowercase_portable_directory_names() {
    for text in [
        "phase-b",
        "rust-developer",
        "a.b_c-1",
        "0",
        "console",
        "com10",
    ] {
        assert!(is_valid_slug(text), "{text}");
    }
    for text in [
        "", ".", "..", "phase.", "Phase B", "Rust", "a/b", "a\\b", "é", "con", "nul", "aux",
        "com1", "lpt9", "con.txt",
    ] {
        assert!(!is_valid_slug(text), "{text}");
    }
    assert!(is_valid_slug(&"a".repeat(MAX_SEGMENT_BYTES)));
    assert!(!is_valid_slug(&"a".repeat(MAX_SEGMENT_BYTES + 1)));
}

#[test]
fn repository_segments_keep_their_broader_character_set() {
    for text in ["Repository", "github.com", "é", "my repo"] {
        assert!(is_path_safe(text), "{text}");
    }
    for text in [
        "", ".", "..", "a/b", "a\\b", "a:", "a?", "a.", "a ", "a\n", "CON", "COM1.txt",
    ] {
        assert!(!is_path_safe(text), "{text:?}");
    }
}
