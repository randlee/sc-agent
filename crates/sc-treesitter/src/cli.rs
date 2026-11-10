use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Args, Parser, Subcommand};
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
        Err(error) => Err(value_parser_error(&error).unwrap_or_else(|| usage_error(&error))),
    }
}
/// `--captures` value parser: comma-separated, trimmed, no empty names.
fn parse_captures(value: &str) -> Result<Captures, JevError> {
    if value.is_empty() {
        return Err(JevError::input_invalid_field(
            "--captures",
            "--captures requires a comma-separated allow-list",
        ));
    }
    value
        .split(',')
        .map(|part| {
            let name = part.trim();
            if name.is_empty() {
                Err(JevError::input_invalid_field(
                    "--captures",
                    "--captures contains an empty name",
                ))
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


    #[test]
    fn captures_are_trimmed_and_ordered() {
        let parsed = parse_captures(" b ,a").expect("captures");
        assert_eq!(parsed.names().collect::<Vec<_>>(), ["b", "a"]);
        assert!(parse_captures("").is_err());
        assert!(parse_captures("a,,b").is_err());
    }
}
