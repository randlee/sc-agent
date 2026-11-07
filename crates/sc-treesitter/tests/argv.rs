//! Argv edge cases of the clap parser (ARCHP-011, CR-016 port from a-10).
//!
//! Every case runs the built binary. Parse failures must be one
//! `ScAgentErrorResponse` line on stderr with exit 2 and `INPUT_INVALID`
//! (ADR-SAG-0029 (c)); `--help` and `--version` print to stdout and exit 0.

mod support;

use std::path::Path;
use std::process::{Command, Output};

use support::{commit_all, git, isolate_git, spawn_with_deadline};
use tempfile::TempDir;

fn run(cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sc-treesitter"));
    isolate_git(&mut command).args(args).current_dir(cwd);
    spawn_with_deadline(&mut command)
}

fn error_code(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(2), "status {:?}", output.status);
    assert!(output.stdout.is_empty(), "stdout must be empty on failure");
    let stderr = output.stderr.strip_suffix(b"\n").unwrap_or(&output.stderr);
    assert!(!stderr.contains(&b'\n'), "stderr must be one line");
    let value: serde_json::Value = serde_json::from_slice(stderr).expect("stderr json");
    assert_eq!(value["schema_version"], 1);
    value["error"].as_str().expect("error code").to_owned()
}

fn stdout_text(output: &Output) -> String {
    assert!(
        output.status.success(),
        "status {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "stderr must be empty on success");
    String::from_utf8(output.stdout.clone()).expect("utf8 stdout")
}

fn repo_with_file() -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix("sc-treesitter-argv-")
        .tempdir()
        .expect("temp dir");
    git(dir.path(), &["init", "-b", "main"]);
    std::fs::write(
        dir.path().join("a.py"),
        "def f(x):\n    match x:\n        case 1:\n            return Refusal(x)\n",
    )
    .expect("write");
    commit_all(dir.path(), "one");
    dir
}

fn extract_with<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["extract", "--language", "python", "--query", "branches"];
    args.extend_from_slice(extra);
    args
}

#[test]
fn test_version_flag_exits_zero() {
    let dir = TempDir::new().expect("temp");
    let expected = format!("sc-treesitter {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["--version", "-V"] {
        assert_eq!(stdout_text(&run(dir.path(), &[flag])), expected, "{flag}");
    }
}

/// `--rev=--output=FILE` (and the two-token form) must fail in argument
/// parsing: git never runs, so no file appears.
#[test]
fn test_rev_option_injection_rejected() {
    let dir = repo_with_file();
    let target = dir.path().join("pwned.txt");
    let joined = format!("--rev=--output={}", target.display());
    let separate = format!("--output={}", target.display());
    for args in [
        extract_with(&[joined.as_str(), "a.py"]),
        extract_with(&["--rev", separate.as_str(), "a.py"]),
        extract_with(&["--rev=-p", "a.py"]),
        extract_with(&["--rev", "-p", "a.py"]),
    ] {
        assert_eq!(
            error_code(&run(dir.path(), &args)),
            "INPUT_INVALID",
            "{args:?}"
        );
        let leaked = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with("pwned"));
        assert!(!leaked, "git must never see the injected option: {args:?}");
    }
}

#[test]
fn test_help_prints_usage_and_exits_zero() {
    let dir = TempDir::new().expect("temp");
    for args in [
        vec!["--help"],
        vec!["-h"],
        vec!["help"],
        vec!["extract", "--help"],
        vec!["help", "extract"],
    ] {
        let text = stdout_text(&run(dir.path(), &args));
        assert!(text.contains("sc-treesitter extract"), "{args:?}: {text}");
        assert!(text.contains("Usage: sc-treesitter "), "{args:?}: {text}");
        assert!(!text.contains("sc-treesitter.exe"), "{args:?}: {text}");
    }
}

/// A value flag takes the next token even when it starts with `-`, so `-h`
/// as a value is not swallowed as `--help`.
#[test]
fn test_hyphen_values_are_values_not_flags() {
    let dir = TempDir::new().expect("temp");
    for args in [
        vec!["extract", "--language", "-h", "--query", "branches", "a.py"],
        vec!["extract", "--language=-h", "--query", "branches", "a.py"],
        vec!["queries", "--language", "-h"],
    ] {
        assert_eq!(
            error_code(&run(dir.path(), &args)),
            "LANGUAGE_UNAVAILABLE",
            "{args:?}"
        );
    }
}

#[test]
fn test_argument_errors_are_input_invalid() {
    let dir = TempDir::new().expect("temp");
    let cases: Vec<Vec<&str>> = vec![
        vec![],
        vec!["bogus"],
        vec!["--bogus"],
        vec!["languages", "extra"],
        vec!["queries", "--language"],
        vec!["queries", "--language", "python", "--language", "rust"],
        extract_with(&[]),
        extract_with(&["a.py", "b.py"]),
        extract_with(&["--bogus", "a.py"]),
        extract_with(&["--rev", "HEAD", "--rev", "HEAD", "a.py"]),
        extract_with(&["--rev=", "a.py"]),
        extract_with(&["--rev", "HEAD main", "a.py"]),
        extract_with(&["--captures=", "a.py"]),
        extract_with(&["--captures", "a,,b", "a.py"]),
        extract_with(&["--captures", "a", "--captures", "b", "a.py"]),
        extract_with(&["--language", "rust", "a.py"]),
        vec!["extract", "--query", "branches", "a.py"],
        vec!["extract", "--language", "python", "a.py"],
        vec!["extract", "--language"],
    ];
    for args in cases {
        assert_eq!(
            error_code(&run(dir.path(), &args)),
            "INPUT_INVALID",
            "{args:?}"
        );
    }
}

#[test]
fn test_missing_source_path_names_it() {
    let dir = TempDir::new().expect("temp");
    let output = run(dir.path(), &extract_with(&[]));
    assert_eq!(error_code(&output), "INPUT_INVALID");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("source path"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `--` ends the options, so a source path may start with `-`; it is then
/// read (and here missing) like any other path.
#[cfg(feature = "python")]
#[test]
fn test_double_dash_allows_hyphen_path() {
    let dir = repo_with_file();
    let output = run(dir.path(), &extract_with(&["--", "-missing.py"]));
    assert_eq!(error_code(&output), "INPUT_MISSING");
}

#[cfg(feature = "python")]
#[test]
fn test_equals_and_separate_forms_agree() {
    let dir = repo_with_file();
    let separate = run(
        dir.path(),
        &extract_with(&["--rev", "HEAD", "--captures", "refusal", "a.py"]),
    );
    let joined = run(
        dir.path(),
        &extract_with(&["--rev=HEAD", "--captures=refusal", "a.py"]),
    );
    let separate = stdout_text(&separate);
    assert_eq!(separate, stdout_text(&joined));
    assert!(
        separate.contains("refusal") && !separate.contains("\"arm\""),
        "{separate}"
    );
}
