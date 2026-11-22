use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    Languages,
    Queries {
        #[arg(long)]
        language: String,
    },
    Query {
        language: String,
        query: String,
        file: PathBuf,
    },
}
