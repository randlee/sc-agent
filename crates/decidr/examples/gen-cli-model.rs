#[path = "../src/cli.rs"]
mod cli;
#[path = "../../../scripts/docli_from_clap.rs"]
mod output;
use clap::CommandFactory;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::write_model(
        &output::output_path_from_args()?,
        &docli::clap_model::from_clap(&cli::Cli::command()),
    )?;
    Ok(())
}
