//! CLI process checks, including the committed branches fixture.
//!
//! Argv edge cases (help, version, hyphen values, duplicates) live in
//! `tests/argv.rs`; the ADR-SAG-0029 (c) error-code assertions stay here.
//!
//! Subprocesses run through `support::spawn_with_deadline`; git runs with
//! the global config disabled. Grammar-specific tests are gated on the
//! grammar's Cargo feature.

mod support;

use std::path::Path;
#[cfg(feature = "python")]
use std::path::PathBuf;
use std::process::{Command, Output};

use support::{commit_all, git, isolate_git, spawn_with_deadline};
use tempfile::TempDir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sc-treesitter")
}

#[cfg(feature = "python")]
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-impl/branches")
}

fn run(cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(binary());
    isolate_git(&mut command).args(args).current_dir(cwd);
    spawn_with_deadline(&mut command)
}

fn temp_repo(object_format: Option<&str>) -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix("sc-treesitter-")
        .tempdir()
        .expect("temp dir");
    match object_format {
        Some(format) => git(
            dir.path(),
            &["init", "-b", "main", &format!("--object-format={format}")],
        ),
        None => git(dir.path(), &["init", "-b", "main"]),
    };
    dir
}

fn error_json(output: &Output) -> serde_json::Value {
    assert_eq!(output.status.code(), Some(2), "status {:?}", output.status);
    assert!(output.stdout.is_empty(), "stdout must be empty on failure");
    serde_json::from_slice(trim_newline(&output.stderr)).expect("stderr json")
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

#[cfg(feature = "python")]
fn extract_args<'a>(rev: &'a str, path: &'a str) -> Vec<&'a str> {
    vec![
        "extract",
        "--language",
        "python",
        "--query",
        "branches",
        "--rev",
        rev,
        path,
    ]
}

#[cfg(feature = "python")]
#[test]
fn test_fixture_extract_stdout_matches_subjects_json() {
    let hook = std::fs::read(fixture_root().join("hook.py")).expect("hook.py");
    let expected = std::fs::read(fixture_root().join("subjects.json")).expect("subjects.json");
    let relative = "tests/fixtures/cross-impl/branches/hook.py";
    let dir = temp_repo(None);
    let file = dir.path().join(relative);
    std::fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
    std::fs::write(&file, &hook).expect("write hook");
    commit_all(dir.path(), "fixture");

    let output = run(dir.path(), &extract_args("HEAD", relative));
    assert!(
        output.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
}

#[test]
fn cli_unknown_language_is_jev_error_on_stderr() {
    let dir = TempDir::new().expect("temp");
    let output = run(
        dir.path(),
        &[
            "extract",
            "--language",
            "cobol",
            "--query",
            "branches",
            "missing.py",
        ],
    );
    let value = error_json(&output);
    assert_eq!(value["error"], "LANGUAGE_UNAVAILABLE");
    assert!(
        value["message"]
            .as_str()
            .expect("message")
            .contains("cobol")
    );
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value.as_object().expect("object").len(), 3);
}

#[cfg(feature = "python")]
#[test]
fn cli_missing_input_is_typed() {
    let dir = temp_repo(None);
    commit_all(dir.path(), "empty");
    let value = error_json(&run(dir.path(), &extract_args("HEAD", "missing.py")));
    assert_eq!(value["error"], "INPUT_MISSING");
    assert!(
        value["message"]
            .as_str()
            .expect("message")
            .contains("missing.py")
    );
}

#[cfg(feature = "python")]
#[test]
fn cli_unknown_revision_is_input_invalid() {
    let dir = temp_repo(None);
    std::fs::write(dir.path().join("a.py"), "x = 1\n").expect("write");
    commit_all(dir.path(), "one");
    let value = error_json(&run(dir.path(), &extract_args("no-such-rev", "a.py")));
    assert_eq!(value["error"], "INPUT_INVALID");
}

#[cfg(feature = "python")]
#[test]
fn cli_tree_path_is_not_a_file() {
    let dir = temp_repo(None);
    std::fs::create_dir_all(dir.path().join("pkg")).expect("mkdir");
    std::fs::write(dir.path().join("pkg/a.py"), "x = 1\n").expect("write");
    commit_all(dir.path(), "one");
    let value = error_json(&run(dir.path(), &extract_args("HEAD", "pkg")));
    assert_eq!(value["error"], "INPUT_INVALID");
}

/// `--rev=--output=<file>` used to reach `git show` as an option and create
/// the file. It must now fail in argument parsing, before any git call.
#[test]
fn cli_option_injection_rev_is_rejected_before_git() {
    let dir = temp_repo(None);
    std::fs::write(dir.path().join("a.py"), "x = 1\n").expect("write");
    commit_all(dir.path(), "one");
    let target = dir.path().join("pwned.txt");
    let target = &target;
    let injected = format!("--rev=--output={}", target.display());
    for args in [
        vec![
            "extract",
            "--language",
            "python",
            "--query",
            "branches",
            injected.as_str(),
            "a.py",
        ],
        vec![
            "extract",
            "--language",
            "python",
            "--query",
            "branches",
            "--rev=-p",
            "a.py",
        ],
    ] {
        let value = error_json(&run(dir.path(), &args));
        assert_eq!(value["error"], "INPUT_INVALID", "{args:?}");
        // Old code passed `{rev}:{path}` to git, so the file would appear as
        // `pwned.txt:a.py`; check every name with the prefix.
        let leaked: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("pwned"))
            .collect();
        assert!(leaked.is_empty(), "git must never see the injected option");
    }
}

#[cfg(feature = "python")]
fn identifier_repo(object_format: Option<&str>) -> TempDir {
    let dir = temp_repo(object_format);
    let hook = std::fs::read(fixture_root().join("hook.py")).expect("hook.py");
    std::fs::write(dir.path().join("hook.py"), &hook).expect("write");
    commit_all(dir.path(), "hook");
    dir
}

#[cfg(feature = "python")]
fn blob_sha(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    value["source"]["blob_sha"]
        .as_str()
        .expect("blob_sha")
        .to_owned()
}

/// The sha comes from the same object id the bytes were read from, so it is
/// always the blob id of the committed content, even after HEAD moves on.
#[cfg(feature = "python")]
#[test]
fn cli_rev_read_sha_is_the_blob_of_the_bytes() {
    let dir = identifier_repo(None);
    let first = git(dir.path(), &["rev-parse", "HEAD"]);
    let first = String::from_utf8_lossy(trim_newline(&first.stdout)).into_owned();
    let expected = git(dir.path(), &["rev-parse", &format!("{first}:hook.py")]);
    std::fs::write(dir.path().join("hook.py"), "moved = 1\n").expect("rewrite");
    commit_all(dir.path(), "move HEAD");
    let sha = blob_sha(&run(dir.path(), &extract_args(&first, "hook.py")));
    assert_eq!(sha, String::from_utf8_lossy(trim_newline(&expected.stdout)));
    let hashed = git(dir.path(), &["hash-object", "--", "hook.py"]);
    assert_ne!(sha, String::from_utf8_lossy(trim_newline(&hashed.stdout)));
}

#[cfg(feature = "python")]
#[test]
fn cli_sha256_repository_is_accepted() {
    let dir = identifier_repo(Some("sha256"));
    let sha = blob_sha(&run(dir.path(), &extract_args("HEAD", "hook.py")));
    assert_eq!(sha.len(), 64, "{sha}");
    let worktree = blob_sha(&run(dir.path(), &extract_args("WORKTREE", "hook.py")));
    assert_eq!(worktree, sha);
}

/// Reads stop at 8 MiB (RSH-007): the worktree file by length, the blob by
/// `git cat-file -s` before it is buffered.
#[cfg(feature = "python")]
#[test]
fn cli_oversize_inputs_are_input_too_large() {
    let dir = temp_repo(None);
    let big = vec![b'#'; 8 * 1024 * 1024 + 1];
    std::fs::write(dir.path().join("big.py"), &big).expect("write");
    commit_all(dir.path(), "big");
    for rev in ["HEAD", "WORKTREE"] {
        let value = error_json(&run(dir.path(), &extract_args(rev, "big.py")));
        assert_eq!(value["error"], "INPUT_TOO_LARGE", "{rev}");
    }
}

/// A `WORKTREE` read hashes the returned bytes, not a filesystem path, so it
/// works with Windows verbatim (`\\?\`) canonical paths and skips clean
/// filters. The file is committed with CRLF bytes under `-text`; switching
/// the attribute to `text` would make a filtered hash normalise to LF.
#[cfg(feature = "python")]
#[test]
fn cli_worktree_sha_is_the_blob_of_the_raw_bytes() {
    let dir = temp_repo(None);
    std::fs::write(dir.path().join(".gitattributes"), "crlf.py -text\n").expect("attrs");
    std::fs::write(
        dir.path().join("crlf.py"),
        "def f(x):\r\n    return Refusal(x)\r\n",
    )
    .expect("write");
    commit_all(dir.path(), "crlf");
    let head = blob_sha(&run(dir.path(), &extract_args("HEAD", "crlf.py")));
    let worktree = blob_sha(&run(dir.path(), &extract_args("WORKTREE", "crlf.py")));
    assert_eq!(worktree, head);
    std::fs::write(dir.path().join(".gitattributes"), "crlf.py text\n").expect("attrs");
    let filtered = git(dir.path(), &["hash-object", "--", "crlf.py"]);
    assert_ne!(
        String::from_utf8_lossy(trim_newline(&filtered.stdout)),
        head,
        "fixture must make a filtered hash differ"
    );
    let unfiltered = blob_sha(&run(dir.path(), &extract_args("WORKTREE", "crlf.py")));
    assert_eq!(unfiltered, head, "clean filters must not apply");
}

/// An absolute path through a symlinked directory (macOS `/tmp`) names a
/// file inside the repository and must be accepted.
#[cfg(all(unix, feature = "python"))]
#[test]
fn cli_absolute_path_through_symlink_is_inside_repo() {
    let dir = identifier_repo(None);
    let links = TempDir::new().expect("links");
    let link = links.path().join("repo-link");
    std::os::unix::fs::symlink(dir.path(), &link).expect("symlink");
    let absolute = link.join("hook.py");
    let absolute = absolute.to_str().expect("utf8 path");
    for rev in ["HEAD", "WORKTREE"] {
        let output = run(dir.path(), &extract_args(rev, absolute));
        assert!(
            output.status.success(),
            "{rev}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
        assert_eq!(value["source"]["path"], "hook.py");
    }
    // Absent from the working tree: the parent is canonicalized instead.
    std::fs::remove_file(dir.path().join("hook.py")).expect("remove");
    let output = run(dir.path(), &extract_args("HEAD", absolute));
    assert!(
        output.status.success(),
        "deleted file: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(all(unix, feature = "python"))]
#[test]
fn cli_paths_outside_the_repo_are_rejected() {
    let dir = identifier_repo(None);
    let outside = TempDir::new().expect("outside");
    let secret = outside.path().join("secret.py");
    std::fs::write(&secret, "x = 1\n").expect("secret");
    std::os::unix::fs::symlink(&secret, dir.path().join("escape.py")).expect("symlink");
    let secret = secret.to_str().expect("utf8 path");
    for (rev, path) in [
        ("WORKTREE", "escape.py"),
        ("WORKTREE", secret),
        ("HEAD", secret),
        ("HEAD", "../x.py"),
        ("HEAD", "a\\b.py"),
        ("HEAD", "a:b.py"),
    ] {
        let value = error_json(&run(dir.path(), &extract_args(rev, path)));
        assert_eq!(value["error"], "INPUT_INVALID", "{rev} {path}");
    }
}

#[test]
fn cli_missing_positional_is_input_invalid() {
    let dir = TempDir::new().expect("temp");
    let value = error_json(&run(
        dir.path(),
        &["extract", "--language", "python", "--query", "branches"],
    ));
    assert_eq!(value["error"], "INPUT_INVALID");
    assert!(
        value["message"]
            .as_str()
            .expect("message")
            .contains("source path")
    );
}

#[cfg(all(feature = "python", feature = "rust"))]
#[test]
fn cli_languages_and_queries_list_pinned_assets() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
