#[path = "../src/cli.rs"]
mod cli;
#[path = "../../../scripts/docli_from_clap.rs"]
pub mod docli;
use clap::CommandFactory;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    docli::write_model(
        &docli::output_path_from_args()?,
        &docli::from_clap(&cli::Cli::command()),
    )?;
    Ok(())
}
