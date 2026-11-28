//! File-output helpers for the CLI-model generator examples.
//!
//! The command-tree adapter is `docli::clap_model::from_clap` from the pinned
//! sc-docli development dependency, as recommended by its README.

use std::path::{Path, PathBuf};

use docli::schema::CliModel;

/// `--output <path>` from the process arguments.
pub fn output_path_from_args() -> Result<PathBuf, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("--output"), Some(path)) if args.next().is_none() && !path.is_empty() => {
            Ok(PathBuf::from(path))
        }
        _ => Err("usage: gen-cli-model --output <path>".into()),
    }
}

/// Write `model` as pretty JSON, creating parent directories when needed.
pub fn write_model(path: &Path, model: &CliModel) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut bytes =
        serde_json::to_vec_pretty(model).map_err(|error| format!("encode model: {error}"))?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}
