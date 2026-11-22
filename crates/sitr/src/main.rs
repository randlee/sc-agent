use clap::Parser;
use cli::{Cli, Command};
use serde_json::{Value, json};
use std::io::Write;
mod cli;
fn main() {
    let parsed = Cli::try_parse();
    if let Err(error) = &parsed
        && matches!(
            error.kind(),
            clap::error::ErrorKind::DisplayVersion | clap::error::ErrorKind::DisplayHelp
        )
    {
        if error.print().is_err() {
            std::process::exit(1);
        }
        return;
    }
    let value = match sc_agent::default_logger("sitr") {
        Err(error) => sc_agent::result(Err(error)),
        Ok(logger) => {
            let result = (|| -> Result<Value, sc_agent::Error> {
                let cli = parsed.map_err(|e| sc_agent::error("treesitter.usage", e))?;
                let value = match cli.command {
                    Command::Languages => sc_treesitter::languages(),
                    Command::Queries { language } => sc_treesitter::queries(&language)?,
                    Command::Query {
                        language,
                        query,
                        file,
                    } => {
                        let source = std::fs::read_to_string(&file).map_err(|e| {
                            sc_agent::error(
                                "treesitter.file_read",
                                format!("{}: {e}", file.display()),
                            )
                        })?;
                        sc_treesitter::predefined(&language, &query, &source)?
                    }
                };
                sc_agent::log(&logger, "sitr.result", json!({"result":value}))?;
                Ok(value)
            })();
            let value = sc_agent::report(&logger, result);
            if let Err(error) = logger.shutdown() {
                eprintln!("{error}");
            }
            value
        }
    };
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
