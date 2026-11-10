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

    /// AC: REQ-SAG-0201 AC2. Outside sc-decision-schema no serde type is named like a Jev API
    /// type (`SystemOne*`, `Question*`, `Asset*`, `ModelMetadata*`, `NoulCriteria*`).
    ///
    /// The literal `rg 'struct (SystemOne|...)'` also matches `SystemOneBackend`
    /// (sc-decision-systemone), a sans-IO backend with no serde derive; this check looks at
    /// the derive, so it does not count it.
    #[test]
    fn only_sc_decision_schema_maps_the_jev_api() {
        let prefixes = [
            "SystemOne",
            "Question",
            "Asset",
            "ModelMetadata",
            "NoulCriteria",
        ];
        for path in rust_files(&crates_dir()) {
            if path
                .components()
                .any(|part| part.as_os_str() == "sc-decision-schema")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            let lines: Vec<&str> = text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let Some(name) = line
                    .trim_start()
                    .strip_prefix("pub struct ")
                    .or_else(|| line.trim_start().strip_prefix("struct "))
                else {
                    continue;
                };
                if !prefixes.iter().any(|prefix| name.starts_with(prefix)) {
                    continue;
                }
                let derives = lines[index.saturating_sub(6)..index]
                    .iter()
                    .any(|above| above.contains("Serialize") || above.contains("Deserialize"));
                assert!(
                    !derives,
                    "{}:{} maps the Jev API: {line}",
                    path.display(),
                    index + 1
                );
            }
        }
    }
}
