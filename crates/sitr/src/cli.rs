//! `sitr` argument parsing.
//!
//! Every clap error other than the help and version displays becomes
//! [`sc_treesitter::Error::Usage`] (id `treesitter.usage`).

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "sitr",
    bin_name = "sitr",
    version,
    about = "Run predefined tree-sitter queries."
)]
pub(crate) struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// `sitr` commands.
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

/// Arguments of `sitr queries`.
#[derive(Debug, Args)]
pub(crate) struct QueriesArgs {
    /// Only list queries for this language.
    #[arg(long, value_name = "L", allow_hyphen_values = true)]
    pub(crate) language: Option<String>,
}

/// Arguments of `sitr query`.
#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    /// Grammar id (see `sitr languages`).
    #[arg(value_name = "LANGUAGE")]
    pub(crate) language: String,
    /// Predefined query name (see `sitr queries`).
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Source file to query.
    #[arg(value_name = "FILE")]
    pub(crate) file: std::path::PathBuf,
    /// Comma-separated allow-list of capture names.
    #[arg(long, value_name = "NAME,NAME", allow_hyphen_values = true)]
    pub(crate) captures: Option<String>,
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
