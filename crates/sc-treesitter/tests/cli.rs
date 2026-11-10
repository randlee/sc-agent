//! `tsq` end-to-end tests (feature `cli`): each runs the built binary with `HOME` set to a
//! temporary directory, so the general log lands there.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_tsq")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary());
    command
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("SC_LOG_ROOT");
    command
}

fn run(home: &Path, args: &[&str]) -> std::io::Result<Output> {
    command(home, args).output()
}

/// The failure contract: exit 1, empty stdout, one stderr line `{"id", "message", "recovery"?}`.
fn error_json(output: &Output) -> Result<serde_json::Value, String> {
    if output.status.code() != Some(1) {
        return Err(format!("status {:?}", output.status));
    }
    if !output.stdout.is_empty() {
        return Err("stdout must be empty on failure".to_owned());
    }
    let stderr = trim_newline(&output.stderr);
    if stderr.contains(&b'\n') {
        return Err("stderr must be one line".to_owned());
    }
    let value: serde_json::Value =
        serde_json::from_slice(stderr).map_err(|error| format!("stderr json: {error}"))?;
    let shape_ok = value.as_object().is_some_and(|object| {
        let recovery = object.get("recovery");
        object.len() == 2 + usize::from(recovery.is_some())
            && object.get("id").is_some_and(serde_json::Value::is_string)
            && object
                .get("message")
                .is_some_and(serde_json::Value::is_string)
            && recovery.is_none_or(serde_json::Value::is_string)
    });
    if shape_ok {
        Ok(value)
    } else {
        Err(format!("not {{id, message, recovery?}}: {value}"))
    }
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// AC: REQ-SAG-0303 tsq listings.
#[cfg(all(feature = "grammar-python", feature = "grammar-rust"))]
#[test]
fn cli_languages_and_queries_list_pinned_assets() {
    let home = tempfile::tempdir().expect("home");
    let languages = run(home.path(), &["languages"]).expect("spawn tsq");
    success(&languages);
    assert_eq!(languages.stdout, b"[\"python\",\"rust\"]\n");
    let listed: Vec<String> =
        serde_json::from_slice(trim_newline(&languages.stdout)).expect("languages");
    assert_eq!(listed, vec!["python".to_owned(), "rust".to_owned()]);

    let queries = run(home.path(), &["queries", "--language", "python"]).expect("spawn tsq");
    success(&queries);
    let value: serde_json::Value =
        serde_json::from_slice(trim_newline(&queries.stdout)).expect("queries");
    assert_eq!(
        value,
        serde_json::json!([{"language": "python", "query": "branches"}])
    );
}

/// AC: tsq query.
#[cfg(feature = "grammar-rust")]
#[test]
fn query_prints_captures_matching_the_golden() {
    let home = tempfile::tempdir().expect("home");
    let file = fixture("match.rs");
    let output = run(
        home.path(),
        &[
            "query",
            "rust",
            "match-arms",
            file.to_str().expect("utf8 path"),
        ],
    )
    .expect("spawn tsq");
    success(&output);
    let golden = std::fs::read(fixture("match.rs.json")).expect("golden");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&golden)
    );
    assert!(output.stderr.is_empty());
}

/// AC: tsq query.
#[test]
fn unknown_language_and_missing_positional_fail_with_ids() {
    let home = tempfile::tempdir().expect("home");
    let file = fixture("match.rs");
    let path = file.to_str().expect("utf8 path");
    let unknown = error_json(&run(home.path(), &["query", "cobol", "x", path]).expect("spawn tsq"))
        .expect("failure contract");
    assert_eq!(unknown["id"], "treesitter.unknown_language");
    assert_eq!(unknown["message"], "unknown language \"cobol\"");
    assert!(
        unknown["recovery"]
            .as_str()
            .is_some_and(|hint| hint.contains("tsq languages"))
    );

    let usage = error_json(&run(home.path(), &["query", "rust"]).expect("spawn tsq"))
        .expect("failure contract");
    assert_eq!(usage["id"], "treesitter.usage");

    let missing = error_json(
        &run(home.path(), &["query", "rust", "match-arms", "absent.rs"]).expect("spawn tsq"),
    )
    .expect("failure contract");
    assert_eq!(missing["id"], "treesitter.file_not_found");
}

/// AC: REQ-SAG-0002 lowest-layer tsq.
#[cfg(feature = "grammar-python")]
#[test]
fn non_utf8_file_reaches_stderr_as_not_utf8() {
    let home = tempfile::tempdir().expect("home");
    let file = fixture("not-utf8.py");
    let path = file.to_str().expect("utf8 path");
    let value =
        error_json(&run(home.path(), &["query", "python", "branches", path]).expect("spawn tsq"))
            .expect("failure contract");
    assert_eq!(value["id"], "treesitter.not_utf8");
    assert_eq!(
        value["message"],
        format!("source in {path} is not UTF-8 at byte 6")
    );
}

/// AC: REQ-SAG-0002 lowest-layer tsq.
#[test]
fn closed_stdout_is_a_write_error() {
    let home = tempfile::tempdir().expect("home");
    // The read end is dropped before the child starts, so every stdout write is EPIPE.
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let output = command(home.path(), &["languages"])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("spawn tsq");
    assert_eq!(output.status.code(), Some(1), "status {:?}", output.status);
    let value: serde_json::Value =
        serde_json::from_slice(trim_newline(&output.stderr)).expect("stderr json");
    assert_eq!(value["id"], "treesitter.write");
    assert!(
        value["message"]
            .as_str()
            .is_some_and(|message| message.contains("broken pipe")),
        "{value}"
    );
}

/// AC: REQ-SAG-0003 tsq.log.
#[test]
fn each_run_writes_one_tsq_command_event_to_its_own_log() {
    let home = tempfile::tempdir().expect("home");
    success(&run(home.path(), &["languages"]).expect("spawn tsq"));
    let log = home
        .path()
        .join(".sc-agent")
        .join("logs")
        .join("tsq.log.jsonl");
    let text = std::fs::read_to_string(&log).expect("tsq.log.jsonl");
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("jsonl"))
        .collect();
    let commands: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["action"] == "tsq.command")
        .collect();
    assert_eq!(commands.len(), 1, "{text}");
    let event = commands[0];
    assert_eq!(event["service"], "tsq");
    assert_eq!(event["outcome"], "ok");
    assert_eq!(event["fields"]["subcommand"], "languages");

    let failed = run(home.path(), &["query", "cobol", "x", "y"]).expect("spawn tsq");
    assert_eq!(failed.status.code(), Some(1));
    let text = std::fs::read_to_string(&log).expect("tsq.log.jsonl");
    let last: serde_json::Value = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .rfind(|event| event["action"] == "tsq.command")
        .expect("second event");
    assert_eq!(last["outcome"], "error");
    assert_eq!(last["fields"]["subcommand"], "query");
    assert_eq!(last["fields"]["error_id"], "treesitter.unknown_language");
    assert_eq!(last["fields"]["language"], "cobol");
    assert_eq!(last["fields"]["query"], "x");
    assert_eq!(last["fields"]["file"], "y");
    assert!(last["fields"]["bytes"].is_null(), "no file was read");
    assert!(last["fields"]["duration_ms"].is_u64());

    // A successful query records the source size.
    let file = fixture("match.rs");
    let path = file.to_str().expect("utf8 path");
    success(&run(home.path(), &["query", "rust", "match-arms", path]).expect("spawn tsq"));
    let text = std::fs::read_to_string(&log).expect("tsq.log.jsonl");
    let last: serde_json::Value = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .rfind(|event| event["action"] == "tsq.command")
        .expect("third event");
    assert_eq!(last["outcome"], "ok");
    assert_eq!(last["fields"]["language"], "rust");
    assert_eq!(last["fields"]["query"], "match-arms");
    assert_eq!(last["fields"]["file"], path);
    let size = std::fs::metadata(&file).expect("fixture").len();
    assert_eq!(last["fields"]["bytes"], size);
    // A separate file: nothing but tsq's own log lives in the logs directory.
    let files: Vec<_> = std::fs::read_dir(home.path().join(".sc-agent").join("logs"))
        .expect("logs dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(files, ["tsq.log.jsonl"]);
}
