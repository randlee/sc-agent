//! Command-line grammar of the `sc-treesitter` binary (clap derive, ARCHP-011).
//!
//! [`parse`] is the only entry point. `--help` and `--version` print to
//! stdout and exit 0; every other parse failure is a [`JevError`] with code
//! `INPUT_INVALID` (ADR-SAG-0029 (c): an argument error, including a missing
//! source path and a revision starting with `-`). Value flags accept values
//! that start with `-` (`allow_hyphen_values`), so `--language -h` names a
//! language rather than asking for help, and `--rev -p` reaches
//! [`Rev::parse`], which rejects it before any git call.

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Args, Parser, Subcommand};
use sc_agent_types::{JevError, Rev};

/// Usage notes appended to `--help`.
const NOTES: &str = "\
Synopsis:
  sc-treesitter extract --language L --query Q [--captures name,name] [--rev HEAD] <path>
  sc-treesitter languages
  sc-treesitter queries [--language L]
  sc-treesitter --version

<path> is repo-relative, or absolute inside the repository. --rev HEAD reads the
blob with `git cat-file`. --rev WORKTREE reads the working tree. A revision may
not start with `-`. --captures is an allow-list. Execution always uses matches
mode. Success writes JSON to stdout; failure writes one error object to stderr
and exits 2.";

/// Value name of the positional source path; [`message`] reports it as
/// "a source path".
const PATH_VALUE_NAME: &str = "PATH";

// `name` and `bin_name` are fixed so usage and `--version` text never depend
// on argv[0] (Windows runs `sc-treesitter.exe`, CI-R3C-001). A doc comment
// here would replace `about` in `--help`.
#[derive(Debug, Parser)]
#[command(
    name = "sc-treesitter",
    bin_name = "sc-treesitter",
    version,
    about = "Tree-sitter producer for sc/subjects/1 documents.",
    after_help = NOTES
)]
pub(crate) struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// `sc-treesitter` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Parse one source blob and print its sc/subjects/1 document.
    Extract(ExtractArgs),
    /// Print the compiled-in language ids as a JSON array.
    Languages,
    /// Print the bundled queries as a JSON array.
    Queries(QueriesArgs),
}

/// Arguments of `sc-treesitter extract`.
#[derive(Debug, Args)]
pub(crate) struct ExtractArgs {
    /// Grammar id (see `sc-treesitter languages`).
    #[arg(long, value_name = "L", allow_hyphen_values = true)]
    pub(crate) language: String,
    /// Bundled query name (see `sc-treesitter queries`).
    #[arg(long, value_name = "Q", allow_hyphen_values = true)]
    pub(crate) query: String,
    /// Comma-separated allow-list of capture names.
    #[arg(
        long,
        value_name = "NAME,NAME",
        allow_hyphen_values = true,
        value_parser = parse_captures
    )]
    pub(crate) captures: Option<Captures>,
    /// Git revision to read, or WORKTREE for the working tree.
    #[arg(
        long,
        value_name = "REV",
        default_value = "HEAD",
        allow_hyphen_values = true,
        value_parser = parse_rev
    )]
    pub(crate) rev: Rev,
    /// Source path: repo-relative, or absolute inside the repository.
    #[arg(value_name = PATH_VALUE_NAME)]
    pub(crate) path: String,
}

/// Arguments of `sc-treesitter queries`.
#[derive(Debug, Args)]
pub(crate) struct QueriesArgs {
    /// Only list queries for this language.
    #[arg(long, value_name = "L", allow_hyphen_values = true)]
    pub(crate) language: Option<String>,
}

/// Non-empty `--captures` allow-list, in the order given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Captures(Vec<String>);

impl Captures {
    /// The capture names.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

/// Outcome of [`parse`].
#[derive(Debug)]
pub(crate) enum Parsed {
    /// Run this command.
    Run(Cli),
    /// Print this text to stdout and exit 0 (`--help`, `--version`).
    Print(String),
}

/// Parses `args` (including the program name, as from [`std::env::args_os`]).
///
/// # Errors
///
/// [`JevError::InputInvalid`] for every clap error other than the help and
/// version displays; a value parser's own [`JevError`] (always
/// `INPUT_INVALID` here) is returned as is.
pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, JevError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => Ok(Parsed::Run(cli)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            Ok(Parsed::Print(error.render().to_string()))
        }
        Err(error) => {
            Err(value_parser_error(&error)
                .unwrap_or_else(|| JevError::input_invalid(message(&error))))
        }
    }
}

/// The [`JevError`] a value parser ([`parse_rev`], [`parse_captures`])
/// returned, so its own code and message reach stderr unwrapped.
fn value_parser_error(error: &clap::Error) -> Option<JevError> {
    std::error::Error::source(error)?
        .downcast_ref::<JevError>()
        .cloned()
}

/// One-line, colour-free description of a clap error.
fn message(error: &clap::Error) -> String {
    if error.kind() == ErrorKind::MissingRequiredArgument
        && let Some(ContextValue::Strings(missing)) = error.get(ContextKind::InvalidArg)
    {
        let placeholder = format!("<{PATH_VALUE_NAME}>");
        let names: Vec<&str> = missing
            .iter()
            .map(|name| {
                if *name == placeholder {
                    "a source path"
                } else {
                    name.as_str()
                }
            })
            .collect();
        return format!("missing required argument: {}", names.join(", "));
    }
    if error.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        || error.kind() == ErrorKind::MissingSubcommand
    {
        return "a command is required: extract, languages or queries (see --help)".to_owned();
    }
    let rendered = error.render().to_string();
    let first = rendered.lines().next().unwrap_or_default();
    first.strip_prefix("error: ").unwrap_or(first).to_owned()
}

/// `--rev` value parser: [`Rev::parse`].
fn parse_rev(value: &str) -> Result<Rev, JevError> {
    Rev::parse(value)
}

/// `--captures` value parser: comma-separated, trimmed, no empty names.
fn parse_captures(value: &str) -> Result<Captures, JevError> {
    if value.is_empty() {
        return Err(JevError::input_invalid(
            "--captures requires a comma-separated allow-list",
        ));
    }
    value
        .split(',')
        .map(|part| {
            let name = part.trim();
            if name.is_empty() {
                Err(JevError::input_invalid("--captures contains an empty name"))
            } else {
                Ok(name.to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Captures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    /// Usage text names `sc-treesitter`, never argv\[0\] such as
    /// `sc-treesitter.exe` or a full path.
    #[test]
    fn usage_ignores_argv0() {
        for argv0 in ["sc-treesitter.exe", "/opt/bin/other-name"] {
            let Ok(Parsed::Print(text)) = parse([argv0, "extract", "--help"]) else {
                panic!("{argv0}: extract --help must print");
            };
            assert!(
                text.contains("Usage: sc-treesitter extract "),
                "{argv0}: {text}"
            );
            let Ok(Parsed::Print(text)) = parse([argv0, "--version"]) else {
                panic!("{argv0}: --version must print");
            };
            assert_eq!(
                text,
                format!("sc-treesitter {}\n", env!("CARGO_PKG_VERSION"))
            );
        }
    }

    #[test]
    fn captures_are_trimmed_and_ordered() {
        let parsed = parse_captures(" b ,a").expect("captures");
        assert_eq!(parsed.names().collect::<Vec<_>>(), ["b", "a"]);
        assert!(parse_captures("").is_err());
        assert!(parse_captures("a,,b").is_err());
    }
}
