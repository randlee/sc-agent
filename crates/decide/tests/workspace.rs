//! Workspace searches (REQ-SAG-0001 AC2, REQ-SAG-0007, REQ-SAG-0201 AC2) over `crates/`.

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
        let schema_hits: Vec<String> = rust_files(&crates_dir().join("sc-decision-schema/src"))
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
}
