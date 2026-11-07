//! `command` input contract (REQ-SAG-0312, REQ-SAG-0313, REQ-SAG-0305, D11).
//!
//! Each case writes a `check.toml` whose command runs a fake producer from
//! `tests/fixtures/producers/`, loads it with `config::load_check_str`, and
//! runs it through `extract::extract_input`. The real `sc-treesitter`
//! end-to-end test lands in r-3b.

use std::path::{Path, PathBuf};
use std::process::Command;

use sc_agent_types::{ErrorCode, JevError, RepoRoot, Rev};
use sc_jev_check::config::{InputSpec, load_check_str};
use sc_jev_check::extract::{Extracted, extract_input};
use tempfile::TempDir;

fn python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}

fn producer(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/producers")
        .join(name)
        .display()
        .to_string()
}

/// A temp repo with `src/hook.py`.
fn repo() -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix("sc-jev-check-command-")
        .tempdir()
        .expect("temp");
    std::fs::create_dir(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/hook.py"), "if armed:\n    fire()\n").expect("hook");
    dir
}

/// `[inputs.live]` running `argv`, with an optional `path` line.
fn check_toml(argv: &[String], path: Option<&str>) -> String {
    let command = serde_json::to_string(argv).expect("argv");
    let path = path.map_or_else(String::new, |path| format!("path = \"{path}\"\n"));
    format!(
        "[questions]\npath = \"questions.json\"\n\n[inputs.live]\nkind = \"command\"\ncommand = {command}\n{path}"
    )
}

fn run(repo: &Path, toml: &str) -> Result<Extracted, JevError> {
    let spec = load_check_str(toml)?;
    extract_input(&RepoRoot::new(repo), &Rev::worktree(), &spec.inputs["live"])
}

#[test]
fn test_command_path_placeholder_from_check_toml() {
    let dir = repo();
    let argv = vec![
        python().into(),
        producer("emit_subjects.py"),
        "{path}".into(),
    ];
    let toml = check_toml(&argv, Some("src/hook.py"));
    let spec = load_check_str(&toml).expect("check.toml with command path");
    match &spec.inputs["live"] {
        InputSpec::Command { path, .. } => {
            assert_eq!(path.as_ref().expect("path field").as_str(), "src/hook.py");
        }
        other => panic!("expected a command input, got {other:?}"),
    }
    let extracted = run(dir.path(), &toml).expect("command input runs");
    let document = extracted.document.expect("subjects document");
    assert_eq!(document.source().path, "src/hook.py");
    assert_eq!(extracted.subjects[0].id(), "src/hook.py:1");
    assert_eq!(extracted.subjects[0].text(), "if armed:\n    fire()");
}

#[test]
fn test_command_placeholder_without_path_is_config_invalid() {
    let dir = repo();
    let marker = dir.path().join("ran");
    let argv = vec![
        python().into(),
        producer("emit_subjects.py"),
        "{path}".into(),
        marker.display().to_string(),
    ];
    let error = run(dir.path(), &check_toml(&argv, None)).expect_err("no path field");
    assert_eq!(error.code(), ErrorCode::ConfigInvalid);
    assert!(
        error.fix_hint().contains("path field"),
        "{}",
        error.fix_hint()
    );
    assert!(!marker.exists(), "the producer must not run");

    // A `path` that is not repo-relative is refused when check.toml is parsed.
    for bad in ["../outside.py", "/etc/hosts"] {
        let error = load_check_str(&check_toml(&argv, Some(bad))).expect_err(bad);
        assert_eq!(error.code(), ErrorCode::ConfigInvalid, "{bad}");
    }
    // A missing file is refused before the producer runs.
    let error = run(dir.path(), &check_toml(&argv, Some("src/absent.py"))).expect_err("absent");
    assert_eq!(error.code(), ErrorCode::InputMissing);
    assert!(!marker.exists(), "the producer must not run");
}

#[cfg(unix)]
#[test]
fn test_command_path_symlink_outside_root_refused() {
    let dir = repo();
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("secret.py"), "OUTSIDE-SECRET\n").expect("secret");
    std::os::unix::fs::symlink(
        outside.path().join("secret.py"),
        dir.path().join("src/leak.py"),
    )
    .expect("symlink");
    std::os::unix::fs::symlink(outside.path(), dir.path().join("linked")).expect("dir symlink");
    let marker = dir.path().join("ran");
    let argv = vec![
        python().into(),
        producer("emit_subjects.py"),
        "{path}".into(),
        marker.display().to_string(),
    ];
    for path in ["src/leak.py", "linked/secret.py"] {
        let error = run(dir.path(), &check_toml(&argv, Some(path))).expect_err(path);
        assert_eq!(error.code(), ErrorCode::InputInvalid, "{path}");
        assert!(
            error.fix_hint().contains("outside the repo root"),
            "{error}"
        );
        assert!(!error.to_string().contains("OUTSIDE-SECRET"));
    }
    let error = run(dir.path(), &check_toml(&argv, Some("src"))).expect_err("directory");
    assert_eq!(error.code(), ErrorCode::InputInvalid);
    assert!(!marker.exists(), "the producer must not run");
}

#[test]
fn test_command_stderr_error_response_reraised() {
    let dir = repo();
    let line = r#"{"error":"LANGUAGE_UNAVAILABLE","message":"Use python.","schema_version":1}"#;
    let argv = vec![
        python().into(),
        producer("fail_with_stderr.py"),
        line.into(),
    ];
    let error = run(dir.path(), &check_toml(&argv, None)).expect_err("producer failed");
    assert!(matches!(error, JevError::ProducerError { .. }), "{error:?}");
    assert_eq!(error.code(), ErrorCode::LanguageUnavailable);
    let program = Path::new(python())
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("stem");
    assert_eq!(error.fix_hint(), format!("{program}: Use python."));

    let producer_line = r#"{"error":"QUERY_NOT_FOUND","message":"No query branches.","schema_version":1,"details":{"producer":"sc-treesitter"}}"#;
    let argv = vec![
        python().into(),
        producer("fail_with_stderr.py"),
        producer_line.into(),
    ];
    let error = run(dir.path(), &check_toml(&argv, None)).expect_err("producer failed");
    assert_eq!(error.code(), ErrorCode::QueryNotFound);
}

#[test]
fn test_command_stderr_unknown_code_is_extraction_failed() {
    let dir = repo();
    let unknown = r#"{"error":"NOT_A_CODE","message":"x","schema_version":1}"#;
    let argv = vec![
        python().into(),
        producer("fail_with_stderr.py"),
        unknown.into(),
    ];
    let error = run(dir.path(), &check_toml(&argv, None)).expect_err("unknown id");
    assert_eq!(error.code(), ErrorCode::ExtractionFailed);
    assert!(error.to_string().contains("NOT_A_CODE"), "{error}");

    for line in ["plain text failure", r#"{"error": 1}"#] {
        let argv = vec![
            python().into(),
            producer("fail_with_stderr.py"),
            line.into(),
        ];
        let error = run(dir.path(), &check_toml(&argv, None)).expect_err(line);
        assert_eq!(error.code(), ErrorCode::ExtractionFailed, "{line}");
        assert!(error.to_string().contains("exit 2"), "{error}");
    }
}

const PROBE_KEY: &str = "SC_JEV_CHECK_COMMAND_PROBE_API_KEY";
const PROBE_VALUE: &str = "sentinel-command-target-4b1e";
const PROBE_DONE: &str = "command-env-probe-ran";

/// D11.2 / ADR-SAG-0020: re-runs this test binary with a sentinel
/// credential in its environment (`unsafe_code` is forbidden, so a test
/// cannot set the variable in its own process) and checks the producer did
/// not see it.
#[test]
fn test_command_child_env_is_allowlisted() {
    let exe = std::env::current_exe().expect("test binary");
    let output = Command::new(exe)
        .args([
            "--exact",
            "env_probe_child",
            "--include-ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PROBE_KEY, PROBE_VALUE)
        .env("OPENAI_API_KEY", PROBE_VALUE)
        .output()
        .expect("probe");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "probe failed: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains(PROBE_DONE), "probe did not run: {stdout}");
}

#[test]
#[ignore = "child half of test_command_child_env_is_allowlisted"]
fn env_probe_child() {
    if std::env::var_os(PROBE_KEY).is_none() {
        return;
    }
    let dir = repo();
    let argv = vec![python().into(), producer("print_env.py")];
    let extracted = run(dir.path(), &check_toml(&argv, None)).expect("env probe");
    let text = extracted.subjects[0].text();
    assert!(!text.contains(PROBE_VALUE), "sentinel leaked: {text}");
    assert!(!text.contains(PROBE_KEY), "key name leaked: {text}");
    assert!(!text.contains("OPENAI_API_KEY"), "key name leaked: {text}");
    let names: Vec<String> = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, _)| name.to_ascii_uppercase())
        .collect();
    let allowed = [
        "PATH",
        "HOME",
        "USERPROFILE",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "TMPDIR",
        "LANG",
        "TERM",
    ];
    for name in &names {
        assert!(
            allowed.contains(&name.as_str()) || name.starts_with("LC_") || is_interpreter_var(name),
            "{name} is outside the allowlist"
        );
    }
    println!("{PROBE_DONE}");
}

/// Variables the Python interpreter itself may add to `os.environ`.
fn is_interpreter_var(name: &str) -> bool {
    // macOS sets `__CF_USER_TEXT_ENCODING` for every process; Python on
    // macOS may add `__PYVENV_LAUNCHER__`. Neither comes from this process.
    name.starts_with("__")
}
