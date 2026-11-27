use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[arg(long, global = true)]
    pub config_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "Project/phase for session records")]
    pub project: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    Models,
    Doctor,
    Send {
        #[arg(long, default_value = "jev")]
        model: String,
        #[arg(long)]
        agent_role: String,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        batch: bool,
        #[arg(long, allow_hyphen_values = true)]
        workers: Option<i64>,
        #[arg(long, default_value = "120")]
        timeout_secs: f64,
    },
}
