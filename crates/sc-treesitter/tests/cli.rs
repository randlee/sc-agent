//! CLI process checks, including the committed branches fixture.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sc-treesitter")
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-impl/branches")
}

fn run(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("spawn sc-treesitter")
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "sc-treesitter")
        .env("GIT_AUTHOR_EMAIL", "sc-treesitter@example.com")
        .env("GIT_COMMITTER_NAME", "sc-treesitter")
        .env("GIT_COMMITTER_EMAIL", "sc-treesitter@example.com")
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn test_fixture_extract_stdout_matches_subjects_json() {
    let hook = std::fs::read(fixture_root().join("hook.py")).expect("hook.py");
    let expected = std::fs::read(fixture_root().join("subjects.json")).expect("subjects.json");
    let relative = "tests/fixtures/cross-impl/branches/hook.py";
    let dir = std::env::temp_dir().join(format!("sc-treesitter-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(relative).parent().expect("parent")).expect("mkdir");
    std::fs::write(dir.join(relative), &hook).expect("write hook");
    git(&dir, &["init", "-b", "main"]);
    git(&dir, &["add", "--", relative]);
    git(
        &dir,
        &["-c", "commit.gpgsign=false", "commit", "-m", "fixture"],
    );

    let output = run(
        &dir,
        &[
            "extract",
            "--language",
            "python",
            "--query",
            "branches",
            "--rev",
            "HEAD",
            relative,
        ],
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
}

#[test]
fn cli_unknown_language_is_jev_error_on_stderr() {
    let output = run(
        &std::env::temp_dir(),
        &[
            "extract",
            "--language",
            "cobol",
            "--query",
            "branches",
            "missing.py",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let value: serde_json::Value =
        serde_json::from_slice(trim_newline(&output.stderr)).expect("stderr json");
    assert_eq!(value["error"], "LANGUAGE_UNAVAILABLE");
    assert!(
        value["message"]
            .as_str()
            .expect("message")
            .contains("cobol")
    );
    assert_eq!(value.as_object().expect("object").len(), 2);
}

#[test]
fn cli_missing_input_is_typed() {
    let dir = std::env::temp_dir().join(format!("sc-treesitter-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    git(&dir, &["init", "-b", "main"]);
    git(
        &dir,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "empty",
        ],
    );
    let output = run(
        &dir,
        &[
            "extract",
            "--language",
            "python",
            "--query",
            "branches",
            "--rev",
            "HEAD",
            "missing.py",
        ],
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(2), "status {:?}", output.status);
    let value: serde_json::Value =
        serde_json::from_slice(trim_newline(&output.stderr)).expect("json");
    assert_eq!(value["error"], "INPUT_MISSING");
    assert!(
        value["message"]
            .as_str()
            .expect("message")
            .contains("missing.py")
    );
}

#[test]
fn cli_languages_and_queries_list_pinned_assets() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let languages = run(&root, &["languages"]);
    assert!(
        languages.status.success(),
        "{}",
        String::from_utf8_lossy(&languages.stderr)
    );
    let listed: Vec<String> =
        serde_json::from_slice(trim_newline(&languages.stdout)).expect("languages");
    assert_eq!(listed, vec!["python".to_owned(), "rust".to_owned()]);

    let queries = run(&root, &["queries", "--language", "python"]);
    assert!(
        queries.status.success(),
        "{}",
        String::from_utf8_lossy(&queries.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(trim_newline(&queries.stdout)).expect("queries");
    assert_eq!(value[0]["language"], "python");
    assert_eq!(value[0]["query"], "branches");
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}
