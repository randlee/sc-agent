//! `tsq` argument parsing.
//!
//! Every clap error other than the help and version displays becomes
//! [`sc_treesitter::Error::Usage`] (id `treesitter.usage`).

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "tsq",
    bin_name = "tsq",
    version,
    about = "Run predefined tree-sitter queries."
)]
pub(crate) struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// `tsq` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Print the loaded grammars as a JSON array of language ids.
    Languages,
    /// Print the predefined queries as a JSON array of {language, query}.
    Queries(QueriesArgs),
    /// Run a predefined query on a file and print its captures as JSON.
    Query(QueryArgs),
}

impl Command {
    /// The subcommand name, for the general log.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Languages => "languages",
            Self::Queries(_) => "queries",
            Self::Query(_) => "query",
        }
    }
}

/// Arguments of `tsq queries`.
#[derive(Debug, Args)]
pub(crate) struct QueriesArgs {
    /// Only list queries for this language.
    #[arg(long, value_name = "L", allow_hyphen_values = true)]
    pub(crate) language: Option<String>,
}

/// Arguments of `tsq query`.
#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    /// Grammar id (see `tsq languages`).
    #[arg(value_name = "LANGUAGE")]
    pub(crate) language: String,
    /// Predefined query name (see `tsq queries`).
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Source file to query.
    #[arg(value_name = "FILE")]
    pub(crate) file: std::path::PathBuf,
    /// Comma-separated allow-list of capture names.
    #[arg(
        long,
        value_name = "NAME,NAME",
        allow_hyphen_values = true,
        value_parser = parse_captures
    )]
    pub(crate) captures: Option<Captures>,
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
/// [`sc_treesitter::Error::Usage`] for every clap error other than the help and version
/// displays; `kind` is the clap error kind and `detail` clap's rendered text.
pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, sc_treesitter::Error>
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
        Err(error) => Err(sc_treesitter::Error::Usage {
            kind: format!("{:?}", error.kind()),
            detail: error.render().to_string(),
        }),
    }
}

/// `--captures` value parser: comma-separated, trimmed, no empty names.
fn parse_captures(value: &str) -> Result<Captures, String> {
    if value.is_empty() {
        return Err("--captures requires a comma-separated allow-list".to_owned());
    }
    value
        .split(',')
        .map(|part| {
            let name = part.trim();
            if name.is_empty() {
                Err("--captures contains an empty name".to_owned())
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

    /// AC: tsq query.
    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    /// AC: tsq query.
    #[test]
    fn captures_are_trimmed_and_ordered() {
        let parsed = parse_captures(" b ,a").expect("captures");
        assert_eq!(parsed.names().collect::<Vec<_>>(), ["b", "a"]);
        assert!(parse_captures("").is_err());
        assert!(parse_captures("a,,b").is_err());
    }

    #[test]
    fn bad_captures_and_missing_positional_are_usage_errors() {
        let error = parse([
            "tsq",
            "query",
            "rust",
            "match-arms",
            "f.rs",
            "--captures",
            "a,,b",
        ])
        .expect_err("empty capture name");
        assert_eq!(error.id(), "treesitter.usage");
        assert!(error.message().contains("empty name"), "{error}");

        let error = parse(["tsq", "query", "rust"]).expect_err("missing positionals");
        assert!(matches!(
            error,
            sc_treesitter::Error::Usage { ref kind, .. } if kind == "MissingRequiredArgument"
        ));
    }

    #[test]
    fn help_and_version_are_printed() {
        assert!(
            matches!(parse(["tsq", "--help"]), Ok(Parsed::Print(text)) if text.contains("tsq"))
        );
        assert!(matches!(parse(["tsq", "--version"]), Ok(Parsed::Print(_))));
    }
}
