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
            let value = sc_agent::after_failure(
                sc_agent::report(&logger, result),
                logger.flush(),
                "log.flush",
            );
            sc_agent::after_failure(value, logger.shutdown(), "log.shutdown")
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_logger_flush_returns_the_original_failure() -> Result<(), Box<dyn std::error::Error>>
    {
        let logger = sc_agent::default_logger("sitr-flush-test")?;
        logger.shutdown()?;
        let failed = sc_agent::after_failure(
            sc_agent::result(Ok(json!("success"))),
            logger.flush(),
            "log.flush",
        );
        let original = sc_agent::result(Err(sc_agent::error("treesitter.usage", "bad input")));
        let preserved = sc_agent::after_failure(original, logger.flush(), "log.flush");
        assert_eq!(preserved["error"]["id"], "treesitter.usage");
        let message = preserved["error"]["message"]
            .as_str()
            .ok_or("missing message")?;
        assert!(message.contains("bad input"));
        assert!(message.contains("log.flush"));
        let shutdown = sc_agent::after_failure(preserved, Err("shutdown failed"), "log.shutdown");
        assert_eq!(shutdown["error"]["id"], "treesitter.usage");
        assert!(
            shutdown["error"]["message"]
                .as_str()
                .is_some_and(|s| s.contains("shutdown failed"))
        );
        assert_eq!(failed["error"]["id"], "log.flush");
        assert_eq!(
            failed["error"]["message"],
            logger
                .flush()
                .err()
                .ok_or("closed logger flushed")?
                .to_string()
        );
        Ok(())
    }
}
