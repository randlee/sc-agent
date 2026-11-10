//! Workspace searches (REQ-SAG-0001 AC2, REQ-SAG-0007, REQ-SAG-0201 AC2) over `crates/`, and
//! the determinism scan of every test (`tests_are_deterministic`).

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn crates_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/")
            .to_path_buf()
    }

    fn files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("dir").flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if name != "target" && name != ".venv" && name != "__pycache__" {
                    files(&path, extension, out);
                }
            } else if path.extension().is_some_and(|ext| ext == extension) {
                out.push(path);
            }
        }
    }

    fn rust_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        files(dir, "rs", &mut out);
        out
    }

    /// Every text file under `dir` (UTF-8 files; build output and caches skipped), as `rg` sees it.
    fn text_files(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).expect("dir").flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if !matches!(
                    name.to_str(),
                    Some("target" | ".venv" | "__pycache__" | ".git")
                ) {
                    text_files(&path, out);
                }
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                out.push((path, text));
            }
        }
    }

    /// AC: REQ-SAG-0001 AC2. The AC's literal workspace search finds nothing: no text file
    /// anywhere under `crates/` contains any of the three needles. The needles are built at
    /// run time so this file does not match itself.
    #[test]
    fn no_uuid_text_anywhere_in_crates() {
        let crate_name = ["uu", "id"].concat();
        let needles = [
            ["Uu", "id"].concat(),
            format!("{crate_name}::"),
            format!("{crate_name} ="),
        ];
        let mut files = Vec::new();
        text_files(&crates_dir(), &mut files);
        assert!(
            files.len() > 100,
            "the search saw the workspace ({} files)",
            files.len()
        );
        for (path, text) in files {
            for needle in &needles {
                assert!(
                    !text.contains(needle.as_str()),
                    "{} contains {needle}",
                    path.display()
                );
            }
        }
    }

    /// AC: REQ-SAG-0007. No rounding call and no precision format in any Rust file of the
    /// workspace (tests included, which is stricter than the AC). Needles are built at run
    /// time so this file does not match itself.
    #[test]
    fn no_rounding_anywhere() {
        for path in rust_files(&crates_dir()) {
            let text = std::fs::read_to_string(&path).expect("read");
            let round_call = ["round", "("].concat();
            assert!(!text.contains(&round_call), "{} rounds", path.display());
            let bytes = text.as_bytes();
            for (index, window) in bytes.windows(3).enumerate() {
                if window[0] == b'{' && window[1] == b':' && window[2] == b'.' {
                    let digit = bytes.get(index + 3).is_some_and(u8::is_ascii_digit);
                    assert!(!digit, "{} has a precision format", path.display());
                }
            }
        }
    }

    /// Field names distinctive of the Jev wire format (SystemOneRequest, SystemOneResponse,
    /// Question, Answer, Asset, ModelMetadata); generic words such as `model` or `state` are
    /// left out so a config key or an LLM envelope does not count.
    const JEV_FIELDS: [&str; 10] = [
        "questions",
        "answers",
        "assets",
        "noul",
        "criteria",
        "probabilities",
        "legend",
        "data_base64",
        "release_date",
        "instructions",
    ];

    /// Every serde-derived struct or enum in `text` that has a Jev wire field (by its Rust
    /// name or its `#[serde(rename = "...")]`), as `(type line, fields)`.
    fn jev_types(text: &str) -> Vec<(String, Vec<String>)> {
        let lines: Vec<&str> = text.lines().collect();
        let mut found = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if !(line.contains("derive(")
                && (line.contains("Serialize") || line.contains("Deserialize")))
            {
                continue;
            }
            let Some(start) = (index + 1..lines.len()).find(|&at| {
                let trimmed = lines[at].trim_start();
                let trimmed = trimmed
                    .strip_prefix("pub(crate) ")
                    .or_else(|| trimmed.strip_prefix("pub "))
                    .unwrap_or(trimmed);
                trimmed.starts_with("struct ") || trimmed.starts_with("enum ")
            }) else {
                continue;
            };
            let mut depth = 0_i64;
            let mut fields = Vec::new();
            for (at, line) in lines.iter().enumerate().skip(start) {
                depth += line.matches('{').count() as i64 - line.matches('}').count() as i64;
                let trimmed = line.trim_start();
                let trimmed = trimmed
                    .strip_prefix("pub(crate) ")
                    .or_else(|| trimmed.strip_prefix("pub "))
                    .unwrap_or(trimmed);
                let name: String = trimmed
                    .trim_start_matches("r#")
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                if trimmed
                    .get(name.len()..)
                    .is_some_and(|rest| rest.trim_start().starts_with(':'))
                    && JEV_FIELDS.contains(&name.as_str())
                {
                    fields.push(name);
                }
                if let Some(rename) = trimmed
                    .split("rename = \"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    && JEV_FIELDS.contains(&rename)
                {
                    fields.push(rename.to_owned());
                }
                if at > start && depth <= 0 || (depth == 0 && line.trim_end().ends_with(';')) {
                    break;
                }
            }
            if !fields.is_empty() {
                found.push((lines[start].trim().to_owned(), fields));
            }
        }
        found
    }

    /// AC: REQ-SAG-0201 AC2. Outside sc-decision-schema no serde-derived type carries a Jev
    /// wire field (checked by field name and serde rename, not by type name). The scanner is
    /// proven on sc-decision-schema itself, where it must find the request and response
    /// types, so a broken scanner cannot pass vacuously.
    #[test]
    fn only_sc_decision_schema_maps_the_jev_api() {
        let schema_hits: Vec<String> =
            rust_files(&crates_dir().join("sc-decision-schema").join("src"))
                .iter()
                .flat_map(|path| jev_types(&std::fs::read_to_string(path).expect("read")))
                .map(|(line, _)| line)
                .collect();
        for expected in ["SystemOneRequest", "SystemOneResponse"] {
            assert!(
                schema_hits.iter().any(|line| line.contains(expected)),
                "the scanner must find {expected} in sc-decision-schema: {schema_hits:?}"
            );
        }
        for path in rust_files(&crates_dir()) {
            if path
                .components()
                .any(|part| part.as_os_str() == "sc-decision-schema")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            let hits = jev_types(&text);
            assert!(
                hits.is_empty(),
                "{} maps the Jev API: {hits:?}",
                path.display()
            );
        }
    }

    /// The test code of the workspace, as `(path, first line number, lines)`: every `.rs` file
    /// under `crates/*/tests/`, and every `#[cfg(test)]` module of `crates/*/src/` (an inline
    /// `mod name { .. }`, a `mod name;` file, or a file that starts with `#![cfg(test)]`).
    /// `crates/sc-agent-testkit` is skipped: it is where the bounded forms live.
    fn test_code(crates: &Path) -> Vec<(PathBuf, usize, Vec<String>)> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(crates).expect("crates").flatten() {
            let crate_dir = entry.path();
            if !crate_dir.is_dir() || entry.file_name() == "sc-agent-testkit" {
                continue;
            }
            let tests = crate_dir.join("tests");
            if tests.is_dir() {
                for path in rust_files(&tests) {
                    let text = std::fs::read_to_string(&path).expect("read");
                    found.push((path, 1, text.lines().map(str::to_owned).collect()));
                }
            }
            let src = crate_dir.join("src");
            if src.is_dir() {
                for path in rust_files(&src) {
                    let text = std::fs::read_to_string(&path).expect("read");
                    found.extend(cfg_test_regions(&path, &text));
                }
            }
        }
        found
    }

    /// The `#[cfg(test)]` modules of one source file (see [`test_code`]).
    fn cfg_test_regions(path: &Path, text: &str) -> Vec<(PathBuf, usize, Vec<String>)> {
        let lines: Vec<&str> = text.lines().collect();
        let owned = |range: &[&str]| range.iter().map(|line| (*line).to_owned()).collect();
        if lines
            .iter()
            .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with("//"))
            .is_some_and(|line| line.trim() == "#![cfg(test)]")
        {
            return vec![(path.to_path_buf(), 1, owned(&lines))];
        }
        let mut regions = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            if lines[index].trim() != "#[cfg(test)]" {
                index += 1;
                continue;
            }
            // Skip further attributes (one line or several) to the item.
            let mut item = index + 1;
            let mut depth = 0_i64;
            while let Some(line) = lines.get(item) {
                let trimmed = line.trim_start();
                if depth == 0 && !trimmed.starts_with("#[") {
                    break;
                }
                depth += line.matches('(').count() as i64 - line.matches(')').count() as i64;
                item += 1;
            }
            let Some(line) = lines.get(item) else { break };
            let declaration = line
                .trim_start()
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub ");
            if let Some(rest) = declaration.strip_prefix("mod ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if rest[name.len()..].trim_start().starts_with(';') {
                    if let Some(file) = module_file(path, &name) {
                        let text = std::fs::read_to_string(&file).expect("read");
                        regions.push((file, 1, text.lines().map(str::to_owned).collect()));
                    }
                } else {
                    let mut depth = 0_i64;
                    let mut end = item;
                    for (at, line) in lines.iter().enumerate().skip(item) {
                        depth +=
                            line.matches('{').count() as i64 - line.matches('}').count() as i64;
                        end = at;
                        if depth <= 0 && at > item {
                            break;
                        }
                    }
                    regions.push((path.to_path_buf(), item + 1, owned(&lines[item..=end])));
                    index = end;
                }
            }
            index += 1;
        }
        regions
    }

    /// The file of `mod name;` declared in `path`: `<dir>/name.rs` for `lib.rs`, `main.rs` and
    /// `mod.rs`, else `<dir>/<stem>/name.rs`.
    fn module_file(path: &Path, name: &str) -> Option<PathBuf> {
        let dir = path.parent()?;
        let stem = path.file_stem()?.to_str()?;
        let base = if matches!(stem, "lib" | "main" | "mod") {
            dir.to_path_buf()
        } else {
            dir.join(stem)
        };
        [
            base.join(format!("{name}.rs")),
            base.join(name).join("mod.rs"),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file())
    }

    /// A banned form found in test code.
    #[derive(Debug, PartialEq, Eq)]
    struct Finding {
        path: String,
        line: usize,
        rule: &'static str,
        text: String,
    }

    /// The forms a test may not use: each needle with why, built at run time so this file
    /// does not match itself. Every one is a real clock read or a wait with no deadline; the
    /// bounded forms are in sc-agent-testkit (and the manual clock in sc-agent-types).
    fn rules() -> Vec<(String, &'static str)> {
        vec![
            (["Instant", "::now"].concat(), "reads the real clock"),
            ([".elapsed", "()"].concat(), "measures real time"),
            (["SystemTime", "::now"].concat(), "reads the wall clock"),
            (
                [".recv", "()"].concat(),
                "receives with no deadline (use recv_within)",
            ),
            (
                [".join", "()"].concat(),
                "joins with no deadline (use join_within)",
            ),
            (
                [".accept", "()"].concat(),
                "accepts with no deadline (use serve_once)",
            ),
            (
                [".wait", "("].concat(),
                "a Condvar/Barrier/Child wait with no deadline",
            ),
            (
                [".wait_while", "("].concat(),
                "a Condvar wait with no deadline",
            ),
            (["yield", "_now"].concat(), "spins on the scheduler"),
        ]
    }

    /// The repeat-race bound names: a `for _ in 0..` loop whose bound names one is a
    /// probabilistic stress loop.
    const LOOP_NAMES: [&str; 3] = ["RACES", "WAITS", "ITER"];

    /// Every banned form in `lines` (a file or a test module starting at `first`), minus the
    /// allowlist. Comment lines are skipped: they describe, they do not run.
    fn scan(path: &str, first: usize, lines: &[String], serves_once: bool) -> Vec<Finding> {
        let rules = rules();
        let loop_head = ["for _ in ", "0.."].concat();
        let bind_any = ["bind(\"127.0.0.1", ":0\")"].concat();
        let mut findings = Vec::new();
        for (offset, line) in lines.iter().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            let mut hit = |rule: &'static str| {
                findings.push(Finding {
                    path: path.to_owned(),
                    line: first + offset,
                    rule,
                    text: code.to_owned(),
                });
            };
            for (needle, why) in &rules {
                if code.contains(needle.as_str()) {
                    hit(why);
                }
            }
            if let Some(at) = code.find(&loop_head) {
                let bound: String = code[at + loop_head.len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if LOOP_NAMES.iter().any(|name| bound.contains(name)) {
                    hit("a repeat-race loop (drive the interleaving once instead)");
                }
            }
            if code.contains(&bind_any) && !serves_once {
                hit("binds a loopback port outside serve_once (a freed port can be reused)");
            }
        }
        findings
            .into_iter()
            .filter(|finding| {
                !ALLOWED.iter().any(|(path, needle, _)| {
                    finding.path.ends_with(path) && finding.text.contains(needle)
                })
            })
            .collect()
    }

    /// Exceptions to `tests_are_deterministic`, as `(path suffix, text on the line, why)`.
    /// Kept empty: every test waits through a bounded primitive and reads only a manual clock.
    const ALLOWED: [(&str, &str, &str); 0] = [];

    /// Owner rule: no flaky test, by design. No test reads real time, waits without a deadline,
    /// spins, loops a race, or binds a port outside `serve_once`, in `crates/*/tests/` or in any
    /// `#[cfg(test)]` module (sc-agent-testkit, which holds the bounded forms, is skipped). The
    /// scanner is proven on a positive control that has every banned form, and the scan must
    /// see the whole test code of the workspace, so it cannot pass vacuously.
    ///
    /// Consumer: CI (`cargo test --workspace --all-features`), enforcing the owner's
    /// zero-tolerance rule on flaky tests. Defect class: the 13 nondeterministic tests found at
    /// the phase-A end review (real-time bounds, unbounded waits, repeat-race loops, freed
    /// ports). Delete it if Rust gains a lint that enforces the same, or the owner drops the
    /// rule.
    #[test]
    fn tests_are_deterministic() {
        let control: Vec<String> = [
            ["let t = Instant", "::now();"].concat(),
            ["let e = t.elapsed", "();"].concat(),
            ["let w = SystemTime", "::now();"].concat(),
            ["let v = rx.recv", "();"].concat(),
            ["handle.join", "().unwrap();"].concat(),
            ["let (s, _) = listener.accept", "().unwrap();"].concat(),
            ["let g = condvar.wait", "(g).unwrap();"].concat(),
            ["let g = condvar.wait_while", "(g, |_| true).unwrap();"].concat(),
            ["std::thread::yield", "_now();"].concat(),
            ["for _ in ", "0..RACES {"].concat(),
            ["let l = TcpListener::bind(\"127.0.0.1", ":0\").unwrap();"].concat(),
            ["// a comment: Instant", "::now() is only described here"].concat(),
            "let v = rx.recv_timeout(HANG_GUARD);".to_owned(),
        ]
        .into_iter()
        .collect();
        let caught = scan("control.rs", 1, &control, false);
        let lines: Vec<usize> = caught.iter().map(|finding| finding.line).collect();
        assert_eq!(
            lines,
            (1..=11).collect::<Vec<_>>(),
            "the scanner catches each banned form once and skips comments and bounded forms: \
             {caught:#?}"
        );
        assert!(
            scan("control.rs", 1, &control[10..11], true).is_empty(),
            "a bind beside serve_once is allowed"
        );
        let inline = "fn real() {}\n#[cfg(test)]\n#[cfg(unix)]\nmod tests {\n    fn t() {\n        x.JOIN;\n    }\n}\nfn after() { y.JOIN; }\n"
            .replace("JOIN", &["jo", "in()"].concat());
        let regions = cfg_test_regions(Path::new("src/control.rs"), &inline);
        assert_eq!(regions.len(), 1, "one cfg(test) module");
        assert_eq!(regions[0].1, 4, "it starts at its `mod` line");
        assert_eq!(regions[0].2.len(), 5, "it ends at its closing brace");

        let code = test_code(&crates_dir());
        let files: std::collections::BTreeSet<&PathBuf> =
            code.iter().map(|(path, _, _)| path).collect();
        let lines: usize = code.iter().map(|(_, _, lines)| lines.len()).sum();
        assert!(
            files.len() >= 90 && lines >= 20_000,
            "the scan saw the workspace's test code ({} files, {lines} lines)",
            files.len()
        );
        let mut findings = Vec::new();
        for (path, first, lines) in &code {
            let whole = std::fs::read_to_string(path).expect("read");
            let serves_once = whole.contains("serve_once");
            let shown = path
                .strip_prefix(crates_dir())
                .unwrap_or(path)
                .display()
                .to_string();
            findings.extend(scan(&shown, *first, lines, serves_once));
        }
        assert!(
            findings.is_empty(),
            "{} nondeterministic form(s) in tests:\n{}",
            findings.len(),
            findings
                .iter()
                .map(|finding| format!(
                    "  {}:{}: {}: {}",
                    finding.path, finding.line, finding.rule, finding.text
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
