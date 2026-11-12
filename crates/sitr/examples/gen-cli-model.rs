//! Write sitr's clap command tree as docli CliModel JSON.
//!
//! `scripts/generate-cli-docs.sh` runs this example and renders the JSON with
//! the host `docli` 0.2.0 binary. The example is not a shipped binary.

#![forbid(unsafe_code)]

#[path = "../../../scripts/docli_from_clap.rs"]
mod docli_from_clap;

#[allow(dead_code)]
#[path = "../src/cli.rs"]
mod cli;

use clap::CommandFactory;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gen-cli-model: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), docli_from_clap::CliModelError> {
    let output = docli_from_clap::output_path_from_args()?;
    let model = docli_from_clap::from_clap(&cli::Cli::command());
    docli_from_clap::write_model(&output, &model)
}

#[cfg(test)]
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
