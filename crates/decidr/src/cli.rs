//! decidr argument parsing; the gen-cli-model example reads it for the CLI docs.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "decidr", version)]
pub struct Cli {
    #[arg(long, global = true)]
    pub project: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Decide the request on stdin (or a JSONL batch with --batch).
    Send {
        #[arg(long)]
        model: String,
        #[arg(long)]
        provider: Option<String>,
        /// A local runner named in config.
        #[arg(long, conflicts_with = "provider")]
        runner: Option<String>,
        #[arg(long)]
        agent_role: String,
        #[arg(long)]
        batch: bool,
        #[arg(long, allow_hyphen_values = true)]
        workers: Option<i64>,
        #[arg(long)]
        timeout_secs: Option<f64>,
    },
    /// The model table.
    Models,
    /// Settings with their sources, and where each provider key comes from.
    Doctor,
}
