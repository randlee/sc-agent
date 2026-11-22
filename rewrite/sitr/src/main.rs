use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
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
fn run() -> Result<Value, sc_agent::Error> {
    let cli = Cli::try_parse().map_err(|e| sc_agent::error("treesitter.usage", e))?;
    let logger = sc_agent::default_logger("sitr")?;
    let result = match cli.command {
        Command::Languages => Ok(sc_treesitter::languages()),
        Command::Queries { language } => sc_treesitter::queries(&language),
        Command::Query {
            language,
            query,
            file,
        } => std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|s| sc_treesitter::predefined(&language, &query, &s)),
    }
    .map_err(|e| sc_agent::error("treesitter.query", e));
    sc_agent::log(
        &logger,
        "sitr.result",
        match &result {
            Ok(v) => json!({"value":v}),
            Err(e) => e.detail(),
        },
    );
    let _ = logger.shutdown();
    result
}
fn main() {
    let value = sc_agent::result(run());
    let status = if value.get("ok") == Some(&Value::Bool(true)) {
        0
    } else {
        1
    };
    if writeln!(std::io::stdout().lock(), "{value}").is_err() {
        std::process::exit(1);
    }
    std::process::exit(status);
}
