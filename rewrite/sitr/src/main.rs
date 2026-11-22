use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};
#[derive(Parser)]
struct Cli {
    #[arg(long, global = true)]
    config_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Languages,
    Doctor,
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
    if matches!(cli.command, Command::Doctor) {
        return sc_agent::doctor("sitr", cli.config_dir.as_deref(), None);
    }
    let logger = sc_agent::default_logger("sitr")?;
    let result = (|| match cli.command {
        Command::Languages => Ok(sc_treesitter::languages()),
        Command::Queries { language } => {
            sc_treesitter::queries(&language).map_err(|e| sc_agent::error(e.id, e.message))
        }
        Command::Query {
            language,
            query,
            file,
        } => {
            let source = std::fs::read_to_string(&file).map_err(|e| {
                sc_agent::error(
                    if e.kind() == std::io::ErrorKind::NotFound {
                        "treesitter.file_not_found"
                    } else {
                        "treesitter.file_read"
                    },
                    format!("{}: {e}", file.display()),
                )
            })?;
            sc_treesitter::predefined(&language, &query, &source)
                .map_err(|e| sc_agent::error(e.id, e.message))
        }
        Command::Doctor => Ok(Value::Null),
    })();
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
