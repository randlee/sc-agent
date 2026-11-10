use std::path::Path;
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sc-treesitter")
}
fn run(cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(binary());
    isolate_git(&mut command).args(args).current_dir(cwd);
    spawn_with_deadline(&mut command)
}
fn error_json(output: &Output) -> serde_json::Value {
    assert_eq!(output.status.code(), Some(2), "status {:?}", output.status);
    assert!(output.stdout.is_empty(), "stdout must be empty on failure");
    serde_json::from_slice(trim_newline(&output.stderr)).expect("stderr json")
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
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
