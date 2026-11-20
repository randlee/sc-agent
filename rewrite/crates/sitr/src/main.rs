use clap::{Parser, Subcommand};
use serde_json::json;
use std::process::ExitCode;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Languages,
    Queries { language: String },
}
fn main() -> ExitCode {
    let command = Args::parse().command;
    let action = match &command {
        Command::Languages => "sitr.languages",
        Command::Queries { .. } => "sitr.queries",
    };
    let log = match sc_agent::open_default_logger("sitr", "SITR_LOG_ROOT") {
        Ok(log) => log,
        Err(e) => {
            println!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = sc_agent::log(&log, action) {
        println!("{e}");
        return ExitCode::FAILURE;
    }
    let result = match command {
        Command::Languages => sc_treesitter::languages(),
        Command::Queries { language } if language == "python" => {
            json!({"ok": true, "value": {"functions": "(function_definition name: (identifier) @name)", "classes": "(class_definition name: (identifier) @name)", "imports": "[(import_statement) (import_from_statement)] @import"}})
        }
        Command::Queries { .. } => {
            json!({"ok": false, "error": {"id": "unsupported_language", "message": "Unsupported language", "action": "Choose a supported language"}})
        }
    };
    println!("{result}");
    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
