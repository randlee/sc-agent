//! Write sitr's clap command tree as docli CliModel JSON.
//!
//! Build with `--features cli`. Without that feature the example exits 1 so
//! default `cargo test --all-targets` still compiles this package.

#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
#[path = "../../../scripts/docli_from_clap.rs"]
mod docli_from_clap;

#[cfg(feature = "cli")]
#[allow(dead_code)]
#[path = "../src/cli.rs"]
mod cli;

#[cfg(feature = "cli")]
use clap::CommandFactory;

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(not(feature = "cli"))]
    {
        eprintln!("gen-cli-model: rebuild sc-treesitter with --features cli");
        ExitCode::from(1)
    }
    #[cfg(feature = "cli")]
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gen-cli-model: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(feature = "cli")]
fn run() -> Result<(), docli_from_clap::CliModelError> {
    let output = docli_from_clap::output_path_from_args()?;
    let model = docli_from_clap::from_clap(&cli::Cli::command());
    docli_from_clap::write_model(&output, &model)
}

#[cfg(all(test, feature = "cli"))]
mod tests {
    use super::*;

    #[test]
    fn sitr_model_names_the_shipped_commands() {
        let model = docli_from_clap::from_clap(&cli::Cli::command());
        assert_eq!(model.name(), "sitr");
        let names: Vec<&str> = model
            .subcommands()
            .iter()
            .map(|command| command.name())
            .collect();
        assert_eq!(names, ["languages", "queries", "query"]);
        assert!(
            model
                .options()
                .iter()
                .any(|option| option.long() == Some("--log-root"))
        );
        assert!(
            model
                .subcommands()
                .iter()
                .any(|command| command.name() == "query")
        );
    }
}
