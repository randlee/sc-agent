//! The `decide` command line: argument parsing only (the behaviour is in the library).

use std::path::PathBuf;

use clap::Parser;
use clap::error::ErrorKind;
use decide::DecideError;
use sc_agent_config::RunnerName;

/// `decide`: send TypeSafe (Jev) decision requests.
#[derive(Debug, clap::Parser)]
#[command(
    name = "decide",
    bin_name = "decide",
    version,
    about = "Send TypeSafe (Jev) decision requests."
)]
pub(crate) struct Cli {
    /// Configuration directory holding default.json (and local.json); default ~/.sc-agent. Logs go to the configured log_root, else ~/.sc-agent.
    #[arg(long, global = true, value_name = "PATH", value_parser = non_empty_path)]
    pub(crate) config_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The subcommands.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Send one request (JSON) or a batch (JSONL, --batch) through the Decision trait.
    Send(SendArgs),
    /// Print the TypeSafe models list (GET /v1/models) of TypeSafe or of a configured local runner.
    Models(ModelsArgs),
    /// List detected keys in use order; warn about provider models missing from the catalog and config models tying a built-in.
    Doctor,
}

/// `decide send`.
#[derive(Debug, clap::Args)]
pub(crate) struct SendArgs {
    /// Optional override (typesafe, anthropic, openai, openrouter, nous, together). Absent: chosen by ranking among detected keys (REQ-SAG-0103).
    #[arg(long, value_name = "PROVIDER", value_parser = non_empty_text)]
    pub(crate) provider: Option<String>,
    /// Short name (jev, haiku, luna, ...), a model id, or a configured local runner name.
    #[arg(long, value_name = "NAME", allow_hyphen_values = true, value_parser = non_empty_text)]
    pub(crate) model: String,
    /// Project slug for the session rows.
    #[arg(long, value_name = "SLUG", allow_hyphen_values = true, value_parser = non_empty_text)]
    pub(crate) project: String,
    /// Agent-role slug for the session rows (e.g. plan-anchor, judge-jev, sanity).
    #[arg(long, value_name = "SLUG", allow_hyphen_values = true, value_parser = non_empty_text)]
    pub(crate) agent_role: String,
    /// Input is JSONL: one request per line (a bare Jev request or {"request", "images", "long_context"}).
    #[arg(long)]
    pub(crate) batch: bool,
    /// Image part NAME read from FILE (png, jpg, jpeg); repeatable; single-request mode only.
    #[arg(long = "image", value_name = "NAME=FILE", value_parser = parse_image_arg, conflicts_with = "batch")]
    pub(crate) images: Vec<(String, PathBuf)>,
    /// Send TypeSafe-Danger-Zone-Long-Context: true; single-request mode only.
    #[arg(long, conflicts_with = "batch")]
    pub(crate) long_context: bool,
    /// Request file, or - for stdin.
    #[arg(value_name = "FILE|-", default_value = "-", value_parser = non_empty_text)]
    pub(crate) input: String,
}

/// `decide models`.
#[derive(Debug, clap::Args)]
pub(crate) struct ModelsArgs {
    /// A configured local runner; TypeSafe when absent.
    #[arg(long, value_name = "NAME", value_parser = parse_runner_name)]
    pub(crate) runner: Option<RunnerName>,
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
/// [`DecideError::Usage`] for every clap error other than the help and version displays.
pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, DecideError>
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
        Err(error) => Err(DecideError::from(error)),
    }
}

/// A runner name, parsed once at the edge (`RunnerName`: non-empty, not a short name).
fn parse_runner_name(value: &str) -> Result<RunnerName, String> {
    RunnerName::try_from(value.to_owned())
        .map_err(|reason| format!("{reason}: {value:?}. See decide --help."))
}

fn non_empty_path(value: &str) -> Result<PathBuf, String> {
    non_empty_text(value).map(PathBuf::from)
}

fn non_empty_text(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("a flag value must not be empty. See decide --help.".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

/// `NAME=FILE`, split at the first `=`; both sides non-empty.
fn parse_image_arg(value: &str) -> Result<(String, PathBuf), String> {
    match value.split_once('=') {
        Some((name, file)) if !name.is_empty() && !file.is_empty() => {
            Ok((name.to_owned(), PathBuf::from(file)))
        }
        _ => Err(format!(
            "--image takes NAME=FILE with both sides non-empty, got {value:?}. See decide --help."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    /// AC: REQ-SAG-0004 (no --agent-name flag; project and agent role required).
    #[test]
    fn there_is_no_agent_name_flag_and_both_slugs_are_required() {
        let base = ["decide", "send", "--model", "jev", "--project", "p"];
        let error = parse(base.iter().chain(&["--agent-name", "x"])).expect_err("no flag");
        assert_eq!(sc_agent_types::ErrorId::id(&error), "decide.usage");
        assert!(parse(base).is_err(), "--agent-role is required");
        let parsed = parse(base.iter().chain(&["--agent-role", "r"])).expect("parses");
        assert!(matches!(parsed, Parsed::Run(_)));
    }

    #[test]
    fn images_conflict_with_batch_and_need_name_and_file() {
        let base = [
            "decide",
            "send",
            "--model",
            "jev",
            "--project",
            "p",
            "--agent-role",
            "r",
        ];
        assert!(parse(base.iter().chain(&["--batch", "--image", "a=b.png"])).is_err());
        assert!(parse(base.iter().chain(&["--image", "=b.png"])).is_err());
        assert!(parse(base.iter().chain(&["--image", "a="])).is_err());
        let Ok(Parsed::Run(cli)) = parse(base.iter().chain(&["--image", "a=x=y.png"])) else {
            panic!("an image arg parses");
        };
        let Command::Send(args) = cli.command else {
            panic!("send");
        };
        assert_eq!(
            args.images,
            vec![("a".to_owned(), PathBuf::from("x=y.png"))]
        );
    }

    /// `--config-dir` is global: accepted before or after the subcommand, on send, models and
    /// doctor.
    #[test]
    fn config_dir_is_a_global_flag() {
        for args in [
            &["decide", "--config-dir", "/c", "doctor"][..],
            &["decide", "doctor", "--config-dir", "/c"],
            &["decide", "models", "--config-dir", "/c"],
            &[
                "decide",
                "send",
                "--model",
                "jev",
                "--project",
                "p",
                "--agent-role",
                "r",
                "--config-dir",
                "/c",
            ],
        ] {
            let Ok(Parsed::Run(cli)) = parse(args) else {
                panic!("parses: {args:?}");
            };
            assert_eq!(cli.config_dir, Some(PathBuf::from("/c")), "{args:?}");
        }
        assert!(parse(["decide", "--config-dir", "", "doctor"]).is_err());
    }
}
