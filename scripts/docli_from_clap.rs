//! Map a live [`clap::Command`] onto the sc-docli 0.2.0 `CliModel` JSON shape.
//!
//! This is the `docli::clap_model::from_clap` walk from sc-docli 0.2.0, kept in
//! the consumer so the published crates do not depend on `sc-docli`. The host
//! `docli` binary renders the JSON.

use std::path::{Path, PathBuf};

use clap::{Arg, Command};
use serde::Serialize;

/// Failure while reading arguments or writing a model file.
#[derive(Debug)]
pub struct CliModelError {
    message: String,
}

impl std::fmt::Display for CliModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CliModelError {}

impl CliModelError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// A command (or subcommand) in the neutral model.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CliModel {
    name: String,
    version: Option<String>,
    description: String,
    long_description: String,
    epilogue: String,
    usage: String,
    options: Vec<OptionSpec>,
    arguments: Vec<ArgumentSpec>,
    subcommands: Vec<CliModel>,
}

/// A flag or option.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct OptionSpec {
    name: String,
    long: Option<String>,
    short: Option<String>,
    help: String,
    long_help: String,
    value_name: Option<String>,
    required: bool,
    default_value: Option<String>,
    choices: Vec<String>,
    min_values: Option<usize>,
    max_values: Option<usize>,
}

/// A positional argument.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ArgumentSpec {
    name: String,
    help: String,
    required: bool,
    default_value: Option<String>,
    choices: Vec<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl CliModel {
    /// Command name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Nested subcommands, sorted by name.
    pub fn subcommands(&self) -> &[CliModel] {
        &self.subcommands
    }

    /// Flags and options, sorted by name. Auto-injected help and version are omitted.
    pub fn options(&self) -> &[OptionSpec] {
        &self.options
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl OptionSpec {
    /// Long form, including dashes.
    pub fn long(&self) -> Option<&str> {
        self.long.as_deref()
    }

    /// Canonical identifier.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Build a model from `command` (typically `Cli::command()`).
pub fn from_clap(command: &Command) -> CliModel {
    from_clap_path(command, command.get_name())
}

/// `--output <path>` from the process arguments.
pub fn output_path_from_args() -> Result<PathBuf, CliModelError> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("--output"), Some(path)) if args.next().is_none() && !path.is_empty() => {
            Ok(PathBuf::from(path))
        }
        _ => Err(CliModelError::new("usage: gen-cli-model --output <path>")),
    }
}

/// Write `model` as pretty JSON, creating parent directories when needed.
pub fn write_model(path: &Path, model: &CliModel) -> Result<(), CliModelError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| CliModelError::new(format!("create {}: {error}", parent.display())))?;
    }
    let mut bytes = serde_json::to_vec_pretty(model)
        .map_err(|error| CliModelError::new(format!("encode model: {error}")))?;
    bytes.push(b'\n');
    std::fs::write(path, bytes)
        .map_err(|error| CliModelError::new(format!("write {}: {error}", path.display())))
}

fn from_clap_path(command: &Command, full_name: &str) -> CliModel {
    let mut options = Vec::new();
    let mut arguments = Vec::new();

    for arg in command.get_arguments().filter(|arg| !is_auto_injected(arg)) {
        let long = arg.get_long().map(|name| format!("--{name}"));
        let short = arg.get_short().map(|name| format!("-{name}"));
        let help = arg.get_help().map(ToString::to_string).unwrap_or_default();
        let long_help = arg
            .get_long_help()
            .map(ToString::to_string)
            .unwrap_or_default();
        let value_name = arg
            .get_value_names()
            .and_then(|names| names.first().map(ToString::to_string));
        let default_value = arg
            .get_default_values()
            .first()
            .map(|value| value.to_string_lossy().into_owned());
        let choices = arg
            .get_possible_values()
            .into_iter()
            .map(|value| value.get_name().to_owned())
            .collect();
        let required = arg.is_required_set();
        let num_args = arg.get_num_args();

        if long.is_none() && short.is_none() {
            arguments.push(ArgumentSpec {
                name: arg.get_id().as_str().to_owned(),
                help,
                required,
                default_value,
                choices,
            });
        } else {
            options.push(OptionSpec {
                name: arg.get_id().as_str().to_owned(),
                long,
                short,
                help,
                long_help,
                value_name,
                required,
                default_value,
                choices,
                min_values: num_args.map(|range| range.min_values()),
                max_values: num_args.map(|range| range.max_values()),
            });
        }
    }

    options.sort_by(|left, right| left.name.cmp(&right.name));
    arguments.sort_by(|left, right| left.name.cmp(&right.name));

    let mut subcommands: Vec<CliModel> = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| {
            from_clap_path(
                subcommand,
                &format!("{full_name} {}", subcommand.get_name()),
            )
        })
        .collect();
    subcommands.sort_by(|left, right| left.name.cmp(&right.name));

    CliModel {
        name: command.get_name().to_owned(),
        version: command.get_version().map(ToString::to_string),
        description: command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        long_description: command
            .get_long_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        epilogue: command
            .get_after_help()
            .map(ToString::to_string)
            .unwrap_or_default(),
        usage: command
            .clone()
            .bin_name(full_name)
            .render_usage()
            .to_string(),
        options,
        arguments,
        subcommands,
    }
}

fn is_auto_injected(arg: &Arg) -> bool {
    matches!(arg.get_id().as_str(), "help" | "version")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{ArgAction, Command};

    #[test]
    fn maps_a_positional_a_flag_and_a_subcommand() {
        let command = Command::new("demo")
            .version("1.2.3")
            .about("Demo tool")
            .arg(Arg::new("file").help("Input file").required(true))
            .arg(
                Arg::new("verbose")
                    .long("verbose")
                    .short('v')
                    .help("Verbose")
                    .action(ArgAction::SetTrue),
            )
            .subcommand(Command::new("run").about("Run it"));

        let model = from_clap(&command);
        assert_eq!(model.name(), "demo");
        assert_eq!(model.version.as_deref(), Some("1.2.3"));
        assert_eq!(model.description, "Demo tool");
        assert!(model.usage.contains("demo"));

        let file = model
            .arguments
            .iter()
            .find(|argument| argument.name == "file")
            .expect("positional");
        assert!(file.required);
        assert_eq!(file.help, "Input file");

        let verbose = model
            .options()
            .iter()
            .find(|option| option.name() == "verbose")
            .expect("flag");
        assert_eq!(verbose.long(), Some("--verbose"));
        assert!(model.options().iter().all(|option| option.name() != "help"));
        assert!(
            model
                .options()
                .iter()
                .all(|option| option.name() != "version")
        );

        let names: Vec<&str> = model.subcommands().iter().map(CliModel::name).collect();
        assert_eq!(names, ["run"]);
    }
}
