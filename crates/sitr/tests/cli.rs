//! `sitr` end-to-end tests (feature `cli`): each runs the built binary with
//! `--log-root <tempdir>/.sc-agent`, so the general log lands in the tempdir. No test sets
//! HOME or USERPROFILE (cross-platform guidelines; closes agt-bkj).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sitr")
}

fn temp_home() -> std::io::Result<tempfile::TempDir> {
    let home = tempfile::tempdir()?;
    let mut command = Command::new("git");
    command
        .arg("init")
        .arg("-q")
        .current_dir(home.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = sc_agent_testkit::ChildGuard::spawn(&mut command)
        .wait_with_output(sc_agent_testkit::HANG_GUARD);
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(home)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn canonical_path_with_missing_tail(path: &Path) -> std::io::Result<PathBuf> {
    let mut existing = path;
    let mut suffix = Vec::new();
    while !existing.exists() {
        let component = existing.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing path component")
        })?;
        suffix.push(component.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing path ancestor")
        })?;
    }
    let mut canonical = std::fs::canonicalize(existing)?;
    for component in suffix.iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary());
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("SC_PROJECT")
        .env_remove("SITR_LOG_ROOT")
        .env_remove("DECIDR_LOG_ROOT")
        .env_remove("DECIDR_WORKERS")
        .env_remove("SC_AGENT__WORKERS");
    command
}

fn run(home: &Path, args: &[&str]) -> std::io::Result<Output> {
    run_with_env(home, args, &[])
}

fn run_with_env(
    home: &Path,
    args: &[&str],
    env_overrides: &[(&str, OsString)],
) -> std::io::Result<Output> {
    let mut command = command(home, args);
    for (key, value) in env_overrides {
        command.env(key, value);
    }
    Ok(sc_agent_testkit::ChildGuard::spawn(&mut command)
        .wait_with_output(sc_agent_testkit::HANG_GUARD))
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

/// AC: REQ-SAG-0303 sitr listings.
#[cfg(all(feature = "grammar-python", feature = "grammar-rust"))]
#[test]
fn cli_languages_and_queries_list_pinned_assets() {
    let home = temp_home().expect("temp home and git worktree");
    let languages = run(home.path(), &["languages"]).expect("spawn sitr");
    success(&languages);
    assert_eq!(languages.stdout, b"[\"python\",\"rust\"]\n");
    let listed: Vec<String> =
        serde_json::from_slice(trim_newline(&languages.stdout)).expect("languages");
    assert_eq!(listed, vec!["python".to_owned(), "rust".to_owned()]);

    let queries = run(home.path(), &["queries", "--language", "python"]).expect("spawn sitr");
    success(&queries);
    let value: serde_json::Value =
        serde_json::from_slice(trim_newline(&queries.stdout)).expect("queries");
    assert_eq!(
        value,
        serde_json::json!([{"language": "python", "query": "branches"}])
    );
}

/// AC: sitr query.
#[cfg(feature = "grammar-rust")]
#[test]
fn query_prints_captures_matching_the_golden() {
    let home = temp_home().expect("temp home and git worktree");
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
    .expect("spawn sitr");
    success(&output);
    assert!(!home.path().join(".sc-agent").join("sessions").exists());
    let golden = std::fs::read(fixture("match.rs.json")).expect("golden");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&golden)
    );
    assert!(output.stderr.is_empty());
}

/// AC: sitr query.
#[test]
fn unknown_language_and_missing_positional_fail_with_ids() {
    let home = temp_home().expect("temp home and git worktree");
    let file = fixture("match.rs");
    let path = file.to_str().expect("utf8 path");
    let unknown =
        error_json(&run(home.path(), &["query", "cobol", "x", path]).expect("spawn sitr"))
            .expect("failure contract");
    assert_eq!(unknown["id"], "treesitter.unknown_language");
    assert_eq!(unknown["message"], "unknown language \"cobol\"");
    assert!(
        unknown["recovery"]
            .as_str()
            .is_some_and(|hint| hint.contains("sitr languages"))
    );

    let usage = error_json(&run(home.path(), &["query", "rust"]).expect("spawn sitr"))
        .expect("failure contract");
    assert_eq!(usage["id"], "treesitter.usage");

    let missing = error_json(
        &run(home.path(), &["query", "rust", "match-arms", "absent.rs"]).expect("spawn sitr"),
    )
    .expect("failure contract");
    assert_eq!(missing["id"], "treesitter.file_not_found");
}

/// AC: REQ-SAG-0002 lowest-layer sitr.
#[cfg(feature = "grammar-python")]
#[test]
fn non_utf8_file_reaches_stderr_as_not_utf8() {
    let home = temp_home().expect("temp home and git worktree");
    let file = fixture("not-utf8.py");
    let path = file.to_str().expect("utf8 path");
    let value =
        error_json(&run(home.path(), &["query", "python", "branches", path]).expect("spawn sitr"))
            .expect("failure contract");
    assert_eq!(value["id"], "treesitter.not_utf8");
    assert_eq!(
        value["message"],
        format!("source in {path} is not UTF-8 at byte 6")
    );
}

/// AC: REQ-SAG-0003 sitr.log.
#[test]
fn each_run_writes_one_sitr_command_event_to_its_own_log() {
    let home = temp_home().expect("temp home and git worktree");
    success(&run(home.path(), &["languages"]).expect("spawn sitr"));
    let log = home
        .path()
        .join(".sc-agent")
        .join("logs")
        .join("sitr.log.jsonl");
    let text = std::fs::read_to_string(&log).expect("sitr.log.jsonl");
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("jsonl"))
        .collect();
    let commands: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["action"] == "sitr.command")
        .collect();
    assert_eq!(commands.len(), 1, "{text}");
    let event = commands[0];
    assert_eq!(event["service"], "sitr");
    assert_eq!(event["fields"]["outcome"], "ok");
    assert_eq!(event["fields"]["subcommand"], "languages");

    let failed = run(home.path(), &["query", "cobol", "x", "y"]).expect("spawn sitr");
    assert_eq!(failed.status.code(), Some(1));
    let text = std::fs::read_to_string(&log).expect("sitr.log.jsonl");
    let last: serde_json::Value = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .rfind(|event| event["action"] == "sitr.command")
        .expect("second event");
    assert_eq!(last["fields"]["outcome"], "error");
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
    success(&run(home.path(), &["query", "rust", "match-arms", path]).expect("spawn sitr"));
    let text = std::fs::read_to_string(&log).expect("sitr.log.jsonl");
    let last: serde_json::Value = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .rfind(|event| event["action"] == "sitr.command")
        .expect("third event");
    assert_eq!(last["fields"]["outcome"], "ok");
    assert_eq!(last["fields"]["language"], "rust");
    assert_eq!(last["fields"]["query"], "match-arms");
    assert_eq!(last["fields"]["file"], path);
    let size = std::fs::metadata(&file).expect("fixture").len();
    assert_eq!(last["fields"]["bytes"], size);
    // A separate file: nothing but sitr's own log lives in the logs directory.
    let files: Vec<_> = std::fs::read_dir(home.path().join(".sc-agent").join("logs"))
        .expect("logs dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(files, ["sitr.log.jsonl"]);
}

fn snapshot_config_tree(root: &Path) -> std::io::Result<Vec<(PathBuf, Vec<u8>)>> {
    fn visit(root: &Path, dir: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) -> std::io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else {
                files.push((
                    path.strip_prefix(root)
                        .map_err(|error| {
                            std::io::Error::new(std::io::ErrorKind::InvalidInput, error)
                        })?
                        .into(),
                    std::fs::read(&path)?,
                ));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

#[test]
fn doctor_matches_shared_rows_lists_four_files_and_does_not_write() -> std::io::Result<()> {
    let home = temp_home().expect("temp home and git worktree");
    let canonical_home = std::fs::canonicalize(home.path()).expect("canonical temp home");
    let shared = canonical_home
        .join(".sc")
        .join("sc-agent")
        .join("sc-agent.json");
    std::fs::create_dir_all(shared.parent().expect("parent")).expect("create local config");
    std::fs::write(&shared, br#"{"project":"phase-a"}"#).expect("write local config");
    let before = snapshot_config_tree(home.path()).expect("snapshot temp worktree");

    let output = run(home.path(), &["doctor"]).expect("spawn sitr doctor");
    success(&output);
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(trim_newline(&output.stdout)).expect("doctor JSON");
    let settings = report["settings"].as_array().expect("settings");
    assert_eq!(settings.len(), 2);
    assert_eq!(settings[0]["setting"], "project");
    assert_eq!(settings[0]["value"], "phase-a");
    let actual_shared_source = canonical_path_with_missing_tail(Path::new(
        settings[0]["source"].as_str().expect("source path"),
    ))?;
    assert_eq!(
        actual_shared_source,
        canonical_path_with_missing_tail(&shared)?
    );
    assert_eq!(
        settings[1],
        serde_json::json!({
            "setting": "log_root",
            "value": home.path().join(".sc-agent").display().to_string(),
            "source": "default",
        })
    );
    let files = report["files"].as_array().expect("files");
    assert_eq!(files.len(), 4);
    assert_eq!(
        files.iter().filter(|file| file["exists"] == true).count(),
        1
    );
    let actual_paths = files
        .iter()
        .map(|file| {
            canonical_path_with_missing_tail(Path::new(file["path"].as_str().expect("path")))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let expected_paths = [
        canonical_home
            .join(".sc")
            .join("sc-agent")
            .join("sitr.json"),
        shared.clone(),
        home.path().join(".sc-agent").join("sitr.json"),
        home.path().join(".sc-agent").join("sc-agent.json"),
    ]
    .iter()
    .map(|path| canonical_path_with_missing_tail(path))
    .collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(actual_paths, expected_paths);
    assert_eq!(
        snapshot_config_tree(home.path()).expect("snapshot temp worktree"),
        before
    );
    assert!(
        !home
            .path()
            .join(".sc-agent")
            .join("logs")
            .join("sitr.log.jsonl")
            .exists()
    );
    Ok(())
}

#[test]
fn doctor_uses_shared_project_and_sitr_prefix_only() {
    let home = temp_home().expect("temp home and git worktree");
    let baseline = run_with_env(
        home.path(),
        &["doctor"],
        &[("SC_PROJECT", OsString::from("phase-d"))],
    )
    .expect("spawn sitr doctor without decidr env");
    let output = run_with_env(
        home.path(),
        &["doctor"],
        &[
            ("SC_PROJECT", OsString::from("phase-d")),
            ("DECIDR_WORKERS", OsString::from("3")),
            (
                "DECIDR_LOG_ROOT",
                home.path().join("other").into_os_string(),
            ),
        ],
    )
    .expect("spawn sitr doctor");
    success(&output);
    assert_eq!(output.stdout, baseline.stdout);
    assert_eq!(output.stderr, baseline.stderr);
    let report: serde_json::Value =
        serde_json::from_slice(trim_newline(&output.stdout)).expect("doctor JSON");
    assert_eq!(report["settings"][0]["value"], "phase-d");
    assert_eq!(report["settings"][0]["source"], "SC_PROJECT");
    assert_eq!(report["settings"][1]["source"], "default");

    let sitr_root = home.path().join("sitr-root");
    let output = run_with_env(
        home.path(),
        &["doctor"],
        &[("SITR_LOG_ROOT", sitr_root.clone().into_os_string())],
    )
    .expect("spawn sitr doctor");
    success(&output);
    let report: serde_json::Value =
        serde_json::from_slice(trim_newline(&output.stdout)).expect("doctor JSON");
    assert_eq!(
        report["settings"][1]["value"],
        sitr_root.display().to_string()
    );
    assert_eq!(report["settings"][1]["source"], "SITR_LOG_ROOT");
    success(
        &run_with_env(
            home.path(),
            &["languages"],
            &[("SITR_LOG_ROOT", sitr_root.clone().into_os_string())],
        )
        .expect("spawn sitr with SITR_LOG_ROOT"),
    );
    assert!(sitr_root.join("logs").join("sitr.log.jsonl").is_file());
}

#[test]
fn decidr_environment_does_not_change_sitr_config_or_log_destination() {
    let home = temp_home().expect("temp home and git worktree");
    let baseline = run(home.path(), &["doctor"]).expect("spawn sitr doctor");
    success(&baseline);

    let other_root = home.path().join("other-root");
    let report = run_with_env(
        home.path(),
        &["doctor"],
        &[
            ("DECIDR_WORKERS", OsString::from("3")),
            ("DECIDR_LOG_ROOT", other_root.clone().into_os_string()),
        ],
    )
    .expect("spawn sitr doctor with decidr environment");
    success(&report);
    assert_eq!(report.stdout, baseline.stdout);
    assert_eq!(report.stderr, baseline.stderr);

    success(
        &run_with_env(
            home.path(),
            &["languages"],
            &[("DECIDR_LOG_ROOT", other_root.clone().into_os_string())],
        )
        .expect("spawn sitr with DECIDR_LOG_ROOT"),
    );
    assert!(home.path().join(".sc-agent/logs/sitr.log.jsonl").is_file());
    assert!(!other_root.exists());
}

#[test]
fn decidr_config_files_do_not_change_sitr_doctor_rows_or_files() {
    let home = temp_home().expect("temp home and git worktree");
    let baseline = run(home.path(), &["doctor"]).expect("spawn sitr doctor");
    success(&baseline);

    let local = home.path().join(".sc/sc-agent");
    let global = home.path().join(".sc-agent");
    std::fs::create_dir_all(&local).expect("create local config dir");
    std::fs::create_dir_all(&global).expect("create global config dir");
    std::fs::write(
        local.join("decidr.json"),
        br#"{"project":"ignored-local","log_root":"local-root"}"#,
    )
    .expect("write local decidr config");
    std::fs::write(
        global.join("decidr.json"),
        br#"{"project":"ignored-global","log_root":"global-root"}"#,
    )
    .expect("write global decidr config");

    let report = run(home.path(), &["doctor"]).expect("spawn sitr doctor with decidr files");
    success(&report);
    assert_eq!(report.stdout, baseline.stdout);
    assert_eq!(report.stderr, baseline.stderr);
}

#[test]
fn one_default_run_creates_only_the_general_log_and_no_sessions() {
    let home = temp_home().expect("temp home and git worktree");
    success(&run(home.path(), &["languages"]).expect("spawn sitr"));

    let logs = home.path().join(".sc-agent").join("logs");
    let mut names = std::fs::read_dir(&logs)
        .expect("logs directory")
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()
        .expect("read log entries");
    names.sort();
    assert_eq!(names, vec![OsString::from("sitr.log.jsonl")]);
    assert!(logs.join("sitr.log.jsonl").is_file());
    assert!(!home.path().join(".sc-agent").join("sessions").exists());
}

#[test]
fn doctor_resolves_all_four_config_files_in_layer_order() -> std::io::Result<()> {
    let home = temp_home().expect("temp home and git worktree");
    let local = home.path().join(".sc").join("sc-agent");
    let global = home.path().join(".sc-agent");
    std::fs::create_dir_all(&local).expect("create local config");
    std::fs::create_dir_all(&global).expect("create global config");
    let layers = [
        (local.join("sitr.json"), "local-sitr"),
        (local.join("sc-agent.json"), "local-shared"),
        (global.join("sitr.json"), "global-sitr"),
        (global.join("sc-agent.json"), "global-shared"),
    ];
    for (file, root) in &layers {
        std::fs::write(file, serde_json::json!({"log_root": root}).to_string())
            .expect("write config layer");
    }
    for (index, (file, root)) in layers.iter().enumerate() {
        let output = run(home.path(), &["doctor"]).expect("spawn sitr doctor");
        success(&output);
        let report: serde_json::Value =
            serde_json::from_slice(trim_newline(&output.stdout)).expect("doctor JSON");
        assert_eq!(report["settings"][1]["value"], *root);
        let source = canonical_path_with_missing_tail(file)?;
        let actual_source = canonical_path_with_missing_tail(Path::new(
            report["settings"][1]["source"]
                .as_str()
                .expect("source path"),
        ))?;
        assert_eq!(actual_source, source,);
        if index + 1 < layers.len() {
            std::fs::remove_file(file).expect("remove winning config layer");
        }
    }
    Ok(())
}

#[test]
fn shared_global_log_root_and_flag_override_choose_the_log_path() {
    let home = temp_home().expect("temp home and git worktree");
    let configured = home.path().join("configured");
    let shared = home.path().join(".sc-agent").join("sc-agent.json");
    std::fs::create_dir_all(shared.parent().expect("parent")).expect("create global config");
    std::fs::write(
        &shared,
        serde_json::json!({"log_root": configured}).to_string(),
    )
    .expect("write global config");
    success(&run(home.path(), &["languages"]).expect("spawn sitr"));
    assert!(
        home.path()
            .join("configured")
            .join("logs")
            .join("sitr.log.jsonl")
            .exists()
    );

    let flag_root = home.path().join("flag-root");
    success(
        &run(
            home.path(),
            &["--log-root", flag_root.to_str().expect("path"), "languages"],
        )
        .expect("spawn sitr"),
    );
    assert!(flag_root.join("logs").join("sitr.log.jsonl").exists());
}
